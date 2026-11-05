//! Whole-vault persistence.
//!
//! [`VaultRepository`] writes the vault name, its creation time and every wallet
//! in one transaction, and rebuilds the aggregate through [`Vault::restore`] on
//! load. A partial vault can therefore never be observed: either the save
//! commits completely or the file keeps its previous state.

use xopo_core::Vault;
use xopo_domain::{Label, Timestamp};

use crate::error::StorageError;
use crate::wallets::save_in;
use crate::{Database, VaultMeta, WalletRepository};

const META_NAME: &str = "vault.name";
const META_CREATED_AT: &str = "vault.created_at";

/// Reads and writes the whole vault.
#[derive(Debug)]
pub struct VaultRepository;

impl VaultRepository {
    /// Writes the vault atomically, removing wallets that were deleted in memory.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] when the transaction fails; the previous
    /// state is preserved.
    pub fn save(db: &mut Database, vault: &Vault) -> Result<(), StorageError> {
        let transaction = db.connection_mut().transaction()?;

        VaultMeta::set(&transaction, META_NAME, vault.name().as_str())?;
        VaultMeta::set(
            &transaction,
            META_CREATED_AT,
            &vault.created_at().as_millis().to_string(),
        )?;

        for wallet in vault.wallets() {
            save_in(&transaction, wallet)?;
        }

        let kept: Vec<String> = vault
            .wallets()
            .iter()
            .map(|wallet| wallet.id().to_string())
            .collect();
        let mut statement = transaction.prepare("SELECT id FROM wallets")?;
        let stored: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(statement);
        for id in stored {
            if !kept.contains(&id) {
                transaction.execute("DELETE FROM wallets WHERE id = ?1", [&id])?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    /// Loads the whole vault, or reports why it cannot be loaded.
    ///
    /// A database whose vault metadata is missing is not treated as an empty
    /// vault: that would silently hide data loss.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] when the vault metadata is
    /// missing or unreadable, and propagates row-level failures from
    /// [`WalletRepository::list`] and [`Vault::restore`].
    pub fn load(db: &Database) -> Result<Vault, StorageError> {
        let name = VaultMeta::get(db.connection(), META_NAME)?
            .ok_or_else(|| StorageError::InvalidRecord("vault name is missing".into()))?;
        let created = VaultMeta::get(db.connection(), META_CREATED_AT)?
            .ok_or_else(|| StorageError::InvalidRecord("vault creation time is missing".into()))?;

        let created_at = created
            .parse::<i64>()
            .map_err(|_| StorageError::InvalidRecord(format!("vault creation time {created}")))?;
        let created_at = Timestamp::from_millis(created_at)?;
        let label = Label::parse(&name)?;
        let wallets = WalletRepository::list(db.connection())?;

        Ok(Vault::restore(label, created_at, wallets)?)
    }
}
