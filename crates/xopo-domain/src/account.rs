//! Accounts: one network identity inside a wallet.
//!
//! An account binds a wallet to a network, a validated address, an optional
//! derivation path and a user label. It holds no secret material: keys and
//! seed phrases live in the security layer and are referenced by the wallet,
//! never embedded in the account record.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::address::Address;
use crate::derivation::DerivationPath;
use crate::error::DomainError;
use crate::ids::{AccountId, WalletId};
use crate::network::NetworkId;
use crate::time::Timestamp;
use crate::validation::Label;

/// A single account on a single network.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: AccountId,
    wallet_id: WalletId,
    network: NetworkId,
    label: Label,
    address: Address,
    derivation: Option<DerivationPath>,
    created_at: Timestamp,
}

impl Account {
    /// Creates a new account with a fresh identifier.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::NetworkMismatch`] when the address belongs to a
    /// different network than the account.
    pub fn new(
        wallet_id: WalletId,
        network: NetworkId,
        label: Label,
        address: Address,
        derivation: Option<DerivationPath>,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        Self::restore(
            AccountId::new(),
            wallet_id,
            network,
            label,
            address,
            derivation,
            created_at,
        )
    }

    /// Rebuilds an account with an existing identifier, for example from
    /// storage or a validated wire payload.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::NetworkMismatch`] when the address belongs to a
    /// different network than the account.
    pub fn restore(
        id: AccountId,
        wallet_id: WalletId,
        network: NetworkId,
        label: Label,
        address: Address,
        derivation: Option<DerivationPath>,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        if address.network() != network {
            return Err(DomainError::NetworkMismatch {
                expected: network,
                actual: address.network(),
            });
        }
        Ok(Self {
            id,
            wallet_id,
            network,
            label,
            address,
            derivation,
            created_at,
        })
    }

    /// Stable account identifier.
    #[must_use]
    pub fn id(&self) -> AccountId {
        self.id
    }

    /// Owning wallet.
    #[must_use]
    pub fn wallet_id(&self) -> WalletId {
        self.wallet_id
    }

    /// Network the account lives on.
    #[must_use]
    pub fn network(&self) -> NetworkId {
        self.network
    }

    /// User-visible account label.
    #[must_use]
    pub fn label(&self) -> &Label {
        &self.label
    }

    /// Validated address.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Derivation path, when the account's keys are derived rather than
    /// imported.
    #[must_use]
    pub fn derivation(&self) -> Option<&DerivationPath> {
        self.derivation.as_ref()
    }

    /// Creation time.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Renames the account.
    #[must_use]
    pub fn with_label(self, label: Label) -> Self {
        Self { label, ..self }
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.network)
    }
}

impl<'de> Deserialize<'de> for Account {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawAccount {
            id: AccountId,
            wallet_id: WalletId,
            network: NetworkId,
            label: Label,
            address: Address,
            derivation: Option<DerivationPath>,
            created_at: Timestamp,
        }

        let raw = RawAccount::deserialize(deserializer)?;
        Self::restore(
            raw.id,
            raw.wallet_id,
            raw.network,
            raw.label,
            raw.address,
            raw.derivation,
            raw.created_at,
        )
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

    #[test]
    fn builds_an_account_with_a_derivation_path() {
        let wallet_id = WalletId::new();
        let address = Address::parse(
            NetworkId::Bitcoin,
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
        )
        .expect("address");
        let derivation = DerivationPath::parse("m/84'/0'/0'/0/0").expect("path");
        let account = Account::new(
            wallet_id,
            NetworkId::Bitcoin,
            label("Savings"),
            address.clone(),
            Some(derivation.clone()),
            timestamp(),
        )
        .expect("account");

        assert_eq!(account.wallet_id(), wallet_id);
        assert_eq!(account.network(), NetworkId::Bitcoin);
        assert_eq!(account.address(), &address);
        assert_eq!(account.derivation(), Some(&derivation));
    }

    #[test]
    fn rejects_an_address_from_another_network() {
        let address = Address::parse(
            NetworkId::Ethereum,
            "0x52908400098527886E0F7030069857D2E4169EE7",
        )
        .expect("address");
        let error = Account::new(
            WalletId::new(),
            NetworkId::Bitcoin,
            label("Wrong chain"),
            address,
            None,
            timestamp(),
        )
        .expect_err("network mismatch must be rejected");
        assert_eq!(
            error,
            DomainError::NetworkMismatch {
                expected: NetworkId::Bitcoin,
                actual: NetworkId::Ethereum
            }
        );
    }

    #[test]
    fn json_round_trip_preserves_identity_and_revalidates() {
        let account = Account::new(
            WalletId::new(),
            NetworkId::Ton,
            label("Spending"),
            Address::parse(
                NetworkId::Ton,
                "EQDtFpEwcFAEcRe5mLVh2N6C0x-_hJEM7W61_JLnSF74p4q2",
            )
            .expect("address"),
            None,
            timestamp(),
        )
        .expect("account");

        let json = serde_json::to_string(&account).expect("serialize account");
        let parsed: Account = serde_json::from_str(&json).expect("deserialize account");
        assert_eq!(parsed, account);
    }

    #[test]
    fn json_with_mismatched_network_is_rejected() {
        let json = r#"{
            "id": "6f1f5f1a-3a3f-4a2c-8f43-1f8f0b2b9d11",
            "walletId": "0f0f1a3e-75b2-4e59-9f47-3c7a4b1d2c99",
            "network": "bitcoin",
            "label": "Wrong",
            "address": {"network": "ethereum", "value": "0x52908400098527886E0F7030069857D2E4169EE7"},
            "derivation": null,
            "createdAt": 1700000000000
        }"#;
        assert!(serde_json::from_str::<Account>(json).is_err());
    }

    #[test]
    fn label_can_be_replaced_without_changing_identity() {
        let account = Account::new(
            WalletId::new(),
            NetworkId::Litecoin,
            label("Old"),
            Address::parse(
                NetworkId::Litecoin,
                "ltc1qg88k4u2y6kfcg9yh8n5x0z3m6j7t8h2q4w9z6c",
            )
            .expect("address"),
            None,
            timestamp(),
        )
        .expect("account");
        let renamed = account.clone().with_label(label("New"));
        assert_eq!(renamed.id(), account.id());
        assert_eq!(renamed.label().as_str(), "New");
    }
}
