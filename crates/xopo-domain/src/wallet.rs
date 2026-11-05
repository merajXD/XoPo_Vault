//! Wallets: a labelled collection of accounts.
//!
//! A wallet is the unit a user recognises ("Main", "Trading") and the unit that
//! will be backed by one key source in the security layer. This module owns the
//! composition rules only; it never stores or derives key material.
//!
//! Watch-only wallets are a first-class kind: they hold addresses that the user
//! can observe but cannot spend from, and the UI must label them as such. The
//! core does not pretend a watch-only wallet can sign.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::account::Account;
use crate::address::Address;
use crate::derivation::DerivationPath;
use crate::error::DomainError;
use crate::ids::{AccountId, WalletId};
use crate::network::NetworkId;
use crate::time::Timestamp;
use crate::validation::Label;

/// What a wallet can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WalletKind {
    /// A wallet whose keys the application manages and can sign with.
    Hierarchical,
    /// A wallet of observed addresses only. It can never sign.
    WatchOnly,
}

impl WalletKind {
    /// Human-readable name.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            WalletKind::Hierarchical => "Standard",
            WalletKind::WatchOnly => "Watch-only",
        }
    }

    /// Whether a wallet of this kind can sign transactions.
    #[must_use]
    pub fn can_sign(self) -> bool {
        matches!(self, WalletKind::Hierarchical)
    }
}

/// A wallet and its accounts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wallet {
    id: WalletId,
    label: Label,
    kind: WalletKind,
    created_at: Timestamp,
    accounts: Vec<Account>,
}

impl Wallet {
    /// Creates an empty wallet with a fresh identifier.
    #[must_use]
    pub fn new(kind: WalletKind, label: Label, created_at: Timestamp) -> Self {
        Self {
            id: WalletId::new(),
            label,
            kind,
            created_at,
            accounts: Vec::new(),
        }
    }

    /// Rebuilds a wallet with existing identifiers and accounts.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::AccountWalletMismatch`] if an account records a
    /// different owner, and [`DomainError::DuplicateAccountAddress`] if two
    /// accounts share an address on the same network.
    pub fn restore(
        id: WalletId,
        label: Label,
        kind: WalletKind,
        created_at: Timestamp,
        accounts: Vec<Account>,
    ) -> Result<Self, DomainError> {
        for (index, account) in accounts.iter().enumerate() {
            if account.wallet_id() != id {
                return Err(DomainError::AccountWalletMismatch {
                    account: account.id(),
                    wallet: id,
                    owner: account.wallet_id(),
                });
            }
            let duplicate = accounts[..index].iter().any(|other| {
                other.network() == account.network() && other.address() == account.address()
            });
            if duplicate {
                return Err(DomainError::DuplicateAccountAddress);
            }
        }
        Ok(Self {
            id,
            label,
            kind,
            created_at,
            accounts,
        })
    }

    /// Stable wallet identifier.
    #[must_use]
    pub fn id(&self) -> WalletId {
        self.id
    }

    /// User-visible wallet label.
    #[must_use]
    pub fn label(&self) -> &Label {
        &self.label
    }

    /// Wallet kind.
    #[must_use]
    pub fn kind(&self) -> WalletKind {
        self.kind
    }

    /// Creation time.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Accounts, in insertion order.
    #[must_use]
    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// Number of accounts.
    #[must_use]
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }

    /// Adds an account to this wallet.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DuplicateAccountAddress`] when the wallet already
    /// contains this address on this network, and the account validation errors
    /// from [`Account::new`].
    pub fn add_account(
        &mut self,
        network: NetworkId,
        label: Label,
        address: Address,
        derivation: Option<DerivationPath>,
        created_at: Timestamp,
    ) -> Result<AccountId, DomainError> {
        if self
            .accounts
            .iter()
            .any(|account| account.network() == network && account.address() == &address)
        {
            return Err(DomainError::DuplicateAccountAddress);
        }
        let account = Account::new(self.id, network, label, address, derivation, created_at)?;
        let id = account.id();
        self.accounts.push(account);
        Ok(id)
    }

    /// Finds an account by identifier.
    #[must_use]
    pub fn account(&self, id: AccountId) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id() == id)
    }

    /// Finds an account by network and address.
    #[must_use]
    pub fn account_by_address(&self, network: NetworkId, address: &Address) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.network() == network && account.address() == address)
    }

    /// Accounts on one network.
    pub fn accounts_on(&self, network: NetworkId) -> impl Iterator<Item = &Account> {
        self.accounts
            .iter()
            .filter(move |account| account.network() == network)
    }

    /// Networks that this wallet has at least one account on.
    #[must_use]
    pub fn networks(&self) -> Vec<NetworkId> {
        let mut networks: Vec<NetworkId> = Vec::new();
        for account in &self.accounts {
            if !networks.contains(&account.network()) {
                networks.push(account.network());
            }
        }
        networks
    }
}

impl fmt::Display for Wallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.kind.display_name())
    }
}

impl<'de> Deserialize<'de> for Wallet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawWallet {
            id: WalletId,
            label: Label,
            kind: WalletKind,
            created_at: Timestamp,
            accounts: Vec<Account>,
        }

        let raw = RawWallet::deserialize(deserializer)?;
        Self::restore(raw.id, raw.label, raw.kind, raw.created_at, raw.accounts)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(text: &str) -> Label {
        Label::parse(text).expect("valid label")
    }

    fn timestamp() -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000).expect("valid timestamp")
    }

    fn address(network: NetworkId) -> Address {
        let raw = match network {
            NetworkId::Bitcoin => "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            NetworkId::Ethereum => "0x52908400098527886E0F7030069857D2E4169EE7",
            NetworkId::Ton => "EQDtFpEwcFAEcRe5mLVh2N6C0x-_hJEM7W61_JLnSF74p4q2",
            NetworkId::Litecoin => "ltc1qg88k4u2y6kfcg9yh8n5x0z3m6j7t8h2q4w9z6c",
            other => panic!("test helper has no address for {other}"),
        };
        Address::parse(network, raw).expect("valid address")
    }

    #[test]
    fn new_wallet_starts_empty() {
        let wallet = Wallet::new(WalletKind::Hierarchical, label("Main"), timestamp());
        assert_eq!(wallet.account_count(), 0);
        assert!(wallet.accounts().is_empty());
        assert!(wallet.networks().is_empty());
        assert!(wallet.kind().can_sign());
    }

    #[test]
    fn accounts_can_span_networks() {
        let mut wallet = Wallet::new(WalletKind::Hierarchical, label("Main"), timestamp());
        wallet
            .add_account(
                NetworkId::Bitcoin,
                label("BTC"),
                address(NetworkId::Bitcoin),
                None,
                timestamp(),
            )
            .expect("btc account");
        wallet
            .add_account(
                NetworkId::Ethereum,
                label("ETH"),
                address(NetworkId::Ethereum),
                None,
                timestamp(),
            )
            .expect("eth account");

        assert_eq!(wallet.account_count(), 2);
        assert_eq!(
            wallet.networks(),
            vec![NetworkId::Bitcoin, NetworkId::Ethereum]
        );
        assert_eq!(wallet.accounts_on(NetworkId::Bitcoin).count(), 1);
    }

    #[test]
    fn duplicate_address_is_rejected_per_network() {
        let mut wallet = Wallet::new(WalletKind::Hierarchical, label("Main"), timestamp());
        wallet
            .add_account(
                NetworkId::Bitcoin,
                label("BTC"),
                address(NetworkId::Bitcoin),
                None,
                timestamp(),
            )
            .expect("first account");
        assert_eq!(
            wallet.add_account(
                NetworkId::Bitcoin,
                label("BTC again"),
                address(NetworkId::Bitcoin),
                None,
                timestamp()
            ),
            Err(DomainError::DuplicateAccountAddress)
        );
    }

    #[test]
    fn accounts_are_found_by_id_and_address() {
        let mut wallet = Wallet::new(WalletKind::WatchOnly, label("Observe"), timestamp());
        let btc = address(NetworkId::Bitcoin);
        let id = wallet
            .add_account(
                NetworkId::Bitcoin,
                label("Cold"),
                btc.clone(),
                None,
                timestamp(),
            )
            .expect("account");

        assert_eq!(wallet.account(id).map(Account::id), Some(id));
        assert_eq!(
            wallet
                .account_by_address(NetworkId::Bitcoin, &btc)
                .map(Account::id),
            Some(id)
        );
        assert!(wallet
            .account_by_address(NetworkId::Ethereum, &address(NetworkId::Ethereum))
            .is_none());
        assert!(!wallet.kind().can_sign());
    }

    #[test]
    fn watch_only_wallet_never_reports_signing_ability() {
        assert!(!WalletKind::WatchOnly.can_sign());
        assert_eq!(WalletKind::WatchOnly.display_name(), "Watch-only");
    }

    #[test]
    fn json_round_trip_preserves_accounts() {
        let mut wallet = Wallet::new(WalletKind::Hierarchical, label("Main"), timestamp());
        wallet
            .add_account(
                NetworkId::Ton,
                label("TON"),
                address(NetworkId::Ton),
                Some(DerivationPath::parse("m/44'/607'/0'/0/0").expect("path")),
                timestamp(),
            )
            .expect("account");

        let json = serde_json::to_string(&wallet).expect("serialize wallet");
        let parsed: Wallet = serde_json::from_str(&json).expect("deserialize wallet");
        assert_eq!(parsed, wallet);
    }

    #[test]
    fn restore_rejects_foreign_accounts_and_duplicates() {
        let wallet_id = WalletId::new();
        let other_wallet = WalletId::new();
        let account = Account::new(
            other_wallet,
            NetworkId::Bitcoin,
            label("Foreign"),
            address(NetworkId::Bitcoin),
            None,
            timestamp(),
        )
        .expect("account");
        assert!(matches!(
            Wallet::restore(
                wallet_id,
                label("Main"),
                WalletKind::Hierarchical,
                timestamp(),
                vec![account]
            ),
            Err(DomainError::AccountWalletMismatch { .. })
        ));

        let first = Account::new(
            wallet_id,
            NetworkId::Bitcoin,
            label("One"),
            address(NetworkId::Bitcoin),
            None,
            timestamp(),
        )
        .expect("account");
        let second = Account::new(
            wallet_id,
            NetworkId::Bitcoin,
            label("Two"),
            address(NetworkId::Bitcoin),
            None,
            timestamp(),
        )
        .expect("account");
        assert_eq!(
            Wallet::restore(
                wallet_id,
                label("Main"),
                WalletKind::Hierarchical,
                timestamp(),
                vec![first, second]
            ),
            Err(DomainError::DuplicateAccountAddress)
        );
    }
}
