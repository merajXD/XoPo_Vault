//! Network-scoped addresses.
//!
//! Validation here is *structural*: character set, prefix and length per
//! network. It deliberately does not check Base58Check or Bech32 checksums,
//! which require the chain-specific decoder owned by the relevant adapter.
//!
//! An address that passes this module is therefore well-formed, not proven to
//! exist on-chain or to be owned by anyone.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;
use crate::network::NetworkId;

/// Shortest address accepted for any supported network.
///
/// The shortest real address among the supported chains is well above this
/// bound; it exists to reject obvious truncation.
pub const MIN_ADDRESS_LEN: usize = 20;

/// Longest address accepted for any supported network.
pub const MAX_ADDRESS_LEN: usize = 128;

const BASE58_CHARSET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BECH32_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// An address that has been validated for exactly one network.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    network: NetworkId,
    value: String,
}

impl Address {
    /// Validates an address for a network.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyAddress`], [`DomainError::AddressWhitespace`],
    /// [`DomainError::AddressNonPrintable`], [`DomainError::AddressLength`] or
    /// [`DomainError::AddressFormat`].
    pub fn parse(network: NetworkId, raw: &str) -> Result<Self, DomainError> {
        if raw.is_empty() {
            return Err(DomainError::EmptyAddress);
        }
        if raw.chars().any(char::is_whitespace) {
            return Err(DomainError::AddressWhitespace);
        }
        if !raw.chars().all(|character| character.is_ascii_graphic()) {
            return Err(DomainError::AddressNonPrintable);
        }
        let length = raw.len();
        if !(MIN_ADDRESS_LEN..=MAX_ADDRESS_LEN).contains(&length) {
            return Err(DomainError::AddressLength {
                min: MIN_ADDRESS_LEN,
                max: MAX_ADDRESS_LEN,
                actual: length,
            });
        }
        if !matches_network(network, raw) {
            return Err(DomainError::AddressFormat { network });
        }
        Ok(Self {
            network,
            value: raw.to_owned(),
        })
    }

    /// Network this address belongs to.
    #[must_use]
    pub fn network(&self) -> NetworkId {
        self.network
    }

    /// Address text exactly as supplied by the user or the network.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawAddress {
            network: NetworkId,
            value: String,
        }

        let raw = RawAddress::deserialize(deserializer)?;
        Self::parse(raw.network, &raw.value).map_err(serde::de::Error::custom)
    }
}

fn is_base58(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| BASE58_CHARSET.contains(&byte))
}

fn is_bech32_lowercase(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| BECH32_CHARSET.contains(&byte))
}

fn is_hex(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_base64url(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn is_legacy_base58_address(value: &str, prefixes: &[u8]) -> bool {
    (26..=35).contains(&value.len())
        && is_base58(value)
        && prefixes
            .iter()
            .any(|prefix| value.starts_with(char::from(*prefix)))
}

fn matches_network(network: NetworkId, value: &str) -> bool {
    match network {
        NetworkId::Bitcoin => {
            if let Some(rest) = value.strip_prefix("bc1") {
                is_bech32_lowercase(rest) && (14..=74).contains(&value.len())
            } else {
                is_legacy_base58_address(value, b"13")
            }
        }
        NetworkId::Litecoin => {
            if let Some(rest) = value.strip_prefix("ltc1") {
                is_bech32_lowercase(rest) && (14..=74).contains(&value.len())
            } else {
                is_legacy_base58_address(value, b"LM3")
            }
        }
        NetworkId::BitcoinCash => {
            if let Some(rest) = value.strip_prefix("bitcoincash:") {
                is_bech32_lowercase(rest) && (42..=62).contains(&value.len())
            } else {
                is_legacy_base58_address(value, b"13")
            }
        }
        NetworkId::Ethereum | NetworkId::BnbChain | NetworkId::Polygon => {
            value.len() == 42 && value.starts_with("0x") && is_hex(&value["0x".len()..])
        }
        NetworkId::Ton => {
            value.len() == 48
                && is_base64url(value)
                && (value.starts_with("EQ") || value.starts_with("UQ"))
        }
        NetworkId::Solana => (32..=44).contains(&value.len()) && is_base58(value),
        NetworkId::Xrp => is_legacy_base58_address(value, b"r"),
        NetworkId::Dogecoin => is_legacy_base58_address(value, b"D"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(network: NetworkId) -> &'static str {
        match network {
            NetworkId::Bitcoin => "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            NetworkId::Litecoin => "ltc1qg88k4u2y6kfcg9yh8n5x0z3m6j7t8h2q4w9z6c",
            NetworkId::BitcoinCash => "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a",
            NetworkId::Ethereum => "0x52908400098527886E0F7030069857D2E4169EE7",
            NetworkId::BnbChain => "0x52908400098527886E0F7030069857D2E4169EE7",
            NetworkId::Polygon => "0x52908400098527886E0F7030069857D2E4169EE7",
            NetworkId::Ton => "EQDtFpEwcFAEcRe5mLVh2N6C0x-_hJEM7W61_JLnSF74p4q2",
            NetworkId::Solana => "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            NetworkId::Xrp => "rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh",
            NetworkId::Dogecoin => "DBXu2kgc3xtvCUWFcxFE3r9hEYgmuaaCyD",
        }
    }

    #[test]
    fn accepts_structurally_valid_addresses() {
        for network in NetworkId::ALL {
            let address = Address::parse(network, valid(network))
                .unwrap_or_else(|error| panic!("{network} address rejected: {error}"));
            assert_eq!(address.network(), network);
            assert_eq!(address.as_str(), valid(network));
        }
    }

    #[test]
    fn rejects_an_address_of_another_network() {
        let ethereum = valid(NetworkId::Ethereum);
        assert_eq!(
            Address::parse(NetworkId::Bitcoin, ethereum),
            Err(DomainError::AddressFormat {
                network: NetworkId::Bitcoin
            })
        );
    }

    #[test]
    fn rejects_malformed_characters_and_lengths() {
        assert_eq!(
            Address::parse(NetworkId::Bitcoin, ""),
            Err(DomainError::EmptyAddress)
        );
        assert_eq!(
            Address::parse(
                NetworkId::Bitcoin,
                "bc1q ar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
            ),
            Err(DomainError::AddressWhitespace)
        );
        assert_eq!(
            Address::parse(
                NetworkId::Bitcoin,
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdü"
            ),
            Err(DomainError::AddressNonPrintable)
        );
        assert!(matches!(
            Address::parse(NetworkId::Bitcoin, "bc1"),
            Err(DomainError::AddressLength { .. })
        ));
    }

    #[test]
    fn rejects_wrong_prefixes_and_character_sets() {
        // Valid length and charset, but not a Bitcoin prefix.
        assert_eq!(
            Address::parse(
                NetworkId::Bitcoin,
                "Ltc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
            ),
            Err(DomainError::AddressFormat {
                network: NetworkId::Bitcoin
            })
        );
        // Base58 excludes the digit 0.
        assert_eq!(
            Address::parse(NetworkId::Xrp, "r0b9CJAWyB4rj91VRWn96DkukG4bwdtyTh"),
            Err(DomainError::AddressFormat {
                network: NetworkId::Xrp
            })
        );
        // Ethereum address with a non-hex character.
        assert_eq!(
            Address::parse(
                NetworkId::Ethereum,
                "0x52908400098527886E0F7030069857D2E4169EEZ"
            ),
            Err(DomainError::AddressFormat {
                network: NetworkId::Ethereum
            })
        );
        // TON addresses use a 48-character base64url body.
        assert!(Address::parse(
            NetworkId::Ton,
            "0QDtFpEwcFAEcRe5mLVh2N6C0x-_hJEM7W61_JLnSF74p4q2"
        )
        .is_err());
    }

    #[test]
    fn address_round_trips_through_json_and_is_revalidated() {
        let address =
            Address::parse(NetworkId::Ethereum, valid(NetworkId::Ethereum)).expect("valid address");
        let json = serde_json::to_string(&address).expect("serialize address");
        let parsed: Address = serde_json::from_str(&json).expect("deserialize address");
        assert_eq!(parsed, address);

        assert!(
            serde_json::from_str::<Address>(r#"{"network":"ethereum","value":"0x123"}"#).is_err()
        );
    }
}
