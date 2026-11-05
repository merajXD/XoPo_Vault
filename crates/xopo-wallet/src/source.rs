//! Where a wallet's keys come from.
//!
//! [`KeySource`] is the single place that answers "can this wallet sign?".
//! The answer is a type, not a boolean: [`WatchOnlyAccount`] has no method that
//! returns a seed, a key or a path to one, so a watch-only wallet cannot reach a
//! signing path by flipping a flag in a database row or a UI form.

use xopo_domain::{Address, DerivationPath};

use crate::mnemonic::Mnemonic;

/// How a wallet's addresses are produced.
#[derive(Debug)]
pub enum KeySource {
    /// A phrase this application generated.
    Created(Mnemonic),
    /// A phrase the user typed in.
    Imported(Mnemonic),
    /// Addresses only; there is no key here to lose.
    WatchOnly(WatchOnlyAccount),
}

impl KeySource {
    /// Whether this wallet can never sign.
    #[must_use]
    pub fn is_watch_only(&self) -> bool {
        matches!(self, Self::WatchOnly(_))
    }

    /// The phrase behind a signing wallet.
    #[must_use]
    pub fn mnemonic(&self) -> Option<&Mnemonic> {
        match self {
            Self::Created(mnemonic) | Self::Imported(mnemonic) => Some(mnemonic),
            Self::WatchOnly(_) => None,
        }
    }
}

/// An address that is watched without any key.
///
/// Every method here returns public data. There is deliberately no conversion
/// from `WatchOnlyAccount` to anything that can derive or sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchOnlyAccount {
    address: Address,
    path: Option<DerivationPath>,
}

impl WatchOnlyAccount {
    /// Watches an address with no knowledge of its derivation path.
    #[must_use]
    pub fn new(address: Address) -> Self {
        Self {
            address,
            path: None,
        }
    }

    /// Watches an address while remembering where it came from.
    ///
    /// The path is metadata for display and gap-limit tracking; it grants no
    /// ability to derive anything.
    #[must_use]
    pub fn with_path(address: Address, path: DerivationPath) -> Self {
        Self {
            address,
            path: Some(path),
        }
    }

    /// The watched address.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Derivation path, when the wallet recorded one.
    #[must_use]
    pub fn path(&self) -> Option<&DerivationPath> {
        self.path.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xopo_domain::NetworkId;

    #[test]
    fn a_watch_only_source_never_hands_out_a_mnemonic() {
        let address = Address::parse(
            NetworkId::Bitcoin,
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu",
        )
        .expect("valid address");
        let source = KeySource::WatchOnly(WatchOnlyAccount::new(address.clone()));
        assert!(source.is_watch_only());
        assert!(source.mnemonic().is_none());
        assert_eq!(source_address(&source), &address);
    }

    fn source_address(source: &KeySource) -> &Address {
        match source {
            KeySource::WatchOnly(account) => account.address(),
            KeySource::Created(_) | KeySource::Imported(_) => panic!("not watch-only"),
        }
    }
}
