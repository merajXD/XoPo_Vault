//! The in-memory vault aggregate.
//!
//! [`Vault`] owns the user's wallets and enforces cross-wallet rules that a
//! single [`Wallet`] cannot see. It stays storage-free: `xopo-storage` provides
//! `VaultRepository`, which persists exactly these aggregates. Revision tracking
//! for Local Sync is a later phase and is not simulated here.

use std::collections::HashSet;

use thiserror::Error;

use xopo_domain::{
    Account, AccountId, Address, DerivationPath, DomainError, Label, NetworkId, Timestamp, Wallet,
    WalletId, WalletKind,
};

/// Errors raised by vault-level operations.
#[derive(Debug, Error)]
pub enum VaultError {
    /// A domain value or invariant was violated.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// No wallet with this identifier exists in the vault.
    #[error("wallet {wallet_id} was not found")]
    WalletNotFound {
        /// The identifier that was requested.
        wallet_id: WalletId,
    },
    /// No account with this identifier exists in the vault.
    #[error("account {account_id} was not found")]
    AccountNotFound {
        /// The identifier that was requested.
        account_id: AccountId,
    },
    /// Two wallets claim the same identifier. This cannot happen through the
    /// API; it means stored or synced data is inconsistent.
    #[error("wallet {wallet_id} appears twice in the vault")]
    DuplicateWalletId {
        /// The duplicated identifier.
        wallet_id: WalletId,
    },
    /// Two accounts claim the same identifier, so one of them would be
    /// unreachable.
    #[error("account {account_id} appears twice in the vault")]
    DuplicateAccountId {
        /// The duplicated identifier.
        account_id: AccountId,
    },
}

/// A user's set of wallets.
///
/// The vault is created once per unlocked session. Persisting it is the job of
/// `xopo-storage::VaultRepository`; this type performs no I/O and holds no
/// secret material.
#[derive(Clone, Debug)]
pub struct Vault {
    name: Label,
    created_at: Timestamp,
    wallets: Vec<Wallet>,
}

impl Vault {
    /// Creates an empty vault.
    #[must_use]
    pub fn new(name: Label, created_at: Timestamp) -> Self {
        Self {
            name,
            created_at,
            wallets: Vec::new(),
        }
    }

    /// Rebuilds a vault from stored wallets, re-checking the invariants that
    /// only the whole set can violate.
    ///
    /// # Errors
    ///
    /// Returns [`VaultError::DuplicateWalletId`] or
    /// [`VaultError::DuplicateAccountId`] when the same identity appears twice.
    #[must_use = "a vault must be checked before it is used"]
    pub fn restore(
        name: Label,
        created_at: Timestamp,
        wallets: Vec<Wallet>,
    ) -> Result<Self, VaultError> {
        let mut wallet_ids: HashSet<WalletId> = HashSet::with_capacity(wallets.len());
        let mut account_ids: HashSet<AccountId> = HashSet::new();
        for wallet in &wallets {
            if !wallet_ids.insert(wallet.id()) {
                return Err(VaultError::DuplicateWalletId {
                    wallet_id: wallet.id(),
                });
            }
            for account in wallet.accounts() {
                if !account_ids.insert(account.id()) {
                    return Err(VaultError::DuplicateAccountId {
                        account_id: account.id(),
                    });
                }
            }
        }
        Ok(Self {
            name,
            created_at,
            wallets,
        })
    }

    /// Vault display name.
    #[must_use]
    pub fn name(&self) -> &Label {
        &self.name
    }

    /// Creation time of this vault instance.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// All wallets, in insertion order.
    #[must_use]
    pub fn wallets(&self) -> &[Wallet] {
        &self.wallets
    }

    /// Adds an empty wallet and returns its identifier.
    pub fn create_wallet(
        &mut self,
        kind: WalletKind,
        label: Label,
        created_at: Timestamp,
    ) -> WalletId {
        let wallet = Wallet::new(kind, label, created_at);
        let id = wallet.id();
        self.wallets.push(wallet);
        id
    }

    /// Looks up a wallet.
    #[must_use]
    pub fn wallet(&self, wallet_id: WalletId) -> Option<&Wallet> {
        self.wallets.iter().find(|wallet| wallet.id() == wallet_id)
    }

    /// Adds an account to an existing wallet.
    ///
    /// # Errors
    ///
    /// Returns [`VaultError::WalletNotFound`] for an unknown wallet, otherwise
    /// the domain errors from [`Wallet::add_account`].
    pub fn add_account(
        &mut self,
        wallet_id: WalletId,
        network: NetworkId,
        label: Label,
        address: Address,
        derivation: Option<DerivationPath>,
        created_at: Timestamp,
    ) -> Result<AccountId, VaultError> {
        let wallet = self
            .wallets
            .iter_mut()
            .find(|wallet| wallet.id() == wallet_id)
            .ok_or(VaultError::WalletNotFound { wallet_id })?;
        Ok(wallet.add_account(network, label, address, derivation, created_at)?)
    }

    /// Finds an account and the wallet that owns it.
    #[must_use]
    pub fn account(&self, account_id: AccountId) -> Option<(&Wallet, &Account)> {
        self.wallets
            .iter()
            .find_map(|wallet| wallet.account(account_id).map(|account| (wallet, account)))
    }

    /// Total number of accounts across all wallets.
    #[must_use]
    pub fn account_count(&self) -> usize {
        self.wallets
            .iter()
            .map(xopo_domain::Wallet::account_count)
            .sum()
    }

    /// Whether any wallet is watch-only.
    #[must_use]
    pub fn has_watch_only_wallet(&self) -> bool {
        self.wallets
            .iter()
            .any(|wallet| wallet.kind() == WalletKind::WatchOnly)
    }
}
