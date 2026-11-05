//! Wallet repository.
//!
//! A wallet is stored as one row plus one row per account, and is always
//! rebuilt through [`Wallet::restore`], so every invariant the domain enforces
//! is re-checked when data comes back out of the database. Storage never trusts
//! its own rows.

use rusqlite::{params, Connection, OptionalExtension};

use xopo_domain::{
    Account, Address, DerivationPath, Label, NetworkId, Timestamp, Wallet, WalletId, WalletKind,
};

use crate::error::StorageError;

/// Reads and writes wallets.
#[derive(Debug)]
pub struct WalletRepository;

impl WalletRepository {
    /// Inserts or updates a wallet and its accounts in one transaction.
    ///
    /// Accounts that are no longer part of the wallet are removed; accounts that
    /// remain are updated in place, so their identifiers stay stable.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] if the transaction fails. The wallet row
    /// and its accounts are never left half-written.
    pub fn save(conn: &mut Connection, wallet: &Wallet) -> Result<(), StorageError> {
        let transaction = conn.transaction()?;
        let wallet_id = wallet.id().to_string();

        transaction.execute(
            "INSERT INTO wallets (id, label, kind, key_ref, created_at)
             VALUES (?1, ?2, ?3, NULL, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 label = excluded.label,
                 kind = excluded.kind,
                 created_at = excluded.created_at",
            params![
                wallet_id,
                wallet.label().as_str(),
                kind_text(wallet.kind()),
                wallet.created_at().as_millis()
            ],
        )?;

        for account in wallet.accounts() {
            transaction.execute(
                "INSERT INTO accounts
                     (id, wallet_id, network_id, label, address, derivation, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                     wallet_id = excluded.wallet_id,
                     network_id = excluded.network_id,
                     label = excluded.label,
                     address = excluded.address,
                     derivation = excluded.derivation,
                     created_at = excluded.created_at",
                params![
                    account.id().to_string(),
                    wallet_id,
                    account.network().id(),
                    account.label().as_str(),
                    account.address().as_str(),
                    account.derivation().map(ToString::to_string),
                    account.created_at().as_millis()
                ],
            )?;
        }

        // Remove accounts that were deleted from the aggregate.
        let kept: Vec<String> = wallet
            .accounts()
            .iter()
            .map(|account| account.id().to_string())
            .collect();
        let mut statement = transaction.prepare("SELECT id FROM accounts WHERE wallet_id = ?1")?;
        let stored: Vec<String> = statement
            .query_map([&wallet_id], |row| row.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(statement);

        for id in stored {
            if !kept.contains(&id) {
                transaction.execute("DELETE FROM accounts WHERE id = ?1", [&id])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    /// Loads one wallet with all of its accounts, in insertion order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::WalletNotFound`] when the identifier is unknown,
    /// and [`StorageError::InvalidRecord`] when a row cannot be turned back into
    /// a valid wallet.
    pub fn load(conn: &Connection, id: WalletId) -> Result<Wallet, StorageError> {
        let wallet_id = id.to_string();
        let stored = conn
            .query_row(
                "SELECT label, kind, created_at FROM wallets WHERE id = ?1",
                [&wallet_id],
                |row| {
                    Ok(StoredWallet {
                        label: row.get(0)?,
                        kind: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or(StorageError::WalletNotFound(id))?;

        let accounts = load_accounts(conn, &wallet_id)?;
        let label = Label::parse(&stored.label)?;
        let created_at = Timestamp::from_millis(stored.created_at)?;
        Ok(Wallet::restore(
            id,
            label,
            parse_kind(&stored.kind)?,
            created_at,
            accounts,
        )?)
    }

    /// Loads every wallet, oldest first.
    ///
    /// # Errors
    ///
    /// Propagates row-level failures from [`WalletRepository::load`].
    pub fn list(conn: &Connection) -> Result<Vec<Wallet>, StorageError> {
        let mut statement = conn.prepare("SELECT id FROM wallets ORDER BY created_at, rowid")?;
        let ids: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(statement);

        ids.iter()
            .map(|id| {
                let parsed: WalletId = id
                    .parse()
                    .map_err(|_| StorageError::InvalidRecord(format!("wallet id {id}")))?;
                Self::load(conn, parsed)
            })
            .collect()
    }

    /// Number of stored wallets.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] when the query fails.
    pub fn count(conn: &Connection) -> Result<u64, StorageError> {
        let count: i64 = conn.query_row("SELECT count(*) FROM wallets", [], |row| row.get(0))?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Deletes a wallet and, by cascade, its accounts.
    ///
    /// Returns `true` when a row was removed. Balances and transactions that
    /// reference those accounts are removed by the same cascade.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] when the delete fails.
    pub fn delete(conn: &Connection, id: WalletId) -> Result<bool, StorageError> {
        let removed = conn.execute("DELETE FROM wallets WHERE id = ?1", [id.to_string()])?;
        Ok(removed > 0)
    }
}

struct StoredWallet {
    label: String,
    kind: String,
    created_at: i64,
}

fn load_accounts(conn: &Connection, wallet_id: &str) -> Result<Vec<Account>, StorageError> {
    let mut statement = conn.prepare(
        "SELECT id, network_id, label, address, derivation, created_at
         FROM accounts WHERE wallet_id = ?1 ORDER BY rowid",
    )?;
    let rows = statement
        .query_map([wallet_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let wallet: WalletId = wallet_id
        .parse()
        .map_err(|_| StorageError::InvalidRecord(format!("wallet id {wallet_id}")))?;

    let mut accounts = Vec::with_capacity(rows.len());
    for (id, network, label, address, derivation, created_at) in rows {
        accounts.push(rebuild_account(
            &id,
            wallet,
            &network,
            &label,
            &address,
            derivation.as_deref(),
            created_at,
        )?);
    }
    Ok(accounts)
}

fn rebuild_account(
    id: &str,
    wallet: WalletId,
    network: &str,
    label: &str,
    address: &str,
    derivation: Option<&str>,
    created_at: i64,
) -> Result<Account, StorageError> {
    let network = NetworkId::from_id(network)
        .ok_or_else(|| StorageError::InvalidRecord(format!("network {network}")))?;
    let account_id = id
        .parse()
        .map_err(|_| StorageError::InvalidRecord(format!("account id {id}")))?;
    let derivation = derivation
        .map(DerivationPath::parse)
        .transpose()
        .map_err(|error| StorageError::InvalidRecord(format!("derivation path: {error}")))?;
    Ok(Account::restore(
        account_id,
        wallet,
        network,
        Label::parse(label)?,
        Address::parse(network, address)?,
        derivation,
        Timestamp::from_millis(created_at)?,
    )?)
}

fn kind_text(kind: WalletKind) -> &'static str {
    match kind {
        WalletKind::Hierarchical => "hierarchical",
        WalletKind::WatchOnly => "watch-only",
    }
}

fn parse_kind(text: &str) -> Result<WalletKind, StorageError> {
    match text {
        "hierarchical" => Ok(WalletKind::Hierarchical),
        "watch-only" => Ok(WalletKind::WatchOnly),
        other => Err(StorageError::InvalidRecord(format!("wallet kind {other}"))),
    }
}
