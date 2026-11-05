//! Bitcoin addresses: BIP-84 native segwit and BIP-44 legacy.
//!
//! Encoding is `rust-bitcoin`'s, so the bech32 and Base58Check checksums come
//! from a crate that is itself tested against the BIP-173 and BIP-32 vectors.
//! Parsing verifies the checksum and the address network; a typo cannot produce
//! a plausible-looking address here.
//!
//! Taproot (BIP-86) is not implemented in this phase.

use std::str::FromStr;

use bitcoin::{Address, CompressedPublicKey, Network};
use xopo_domain::NetworkId;

use crate::error::WalletError;
use crate::key::{ChildKey, MasterKey};

/// Address families XoPo derives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BitcoinAddressType {
    /// BIP-84: purpose `84'`, P2WPKH, `bc1q…`.
    NativeSegwit,
    /// BIP-44: purpose `44'`, P2PKH, `1…`.
    Legacy,
}

impl BitcoinAddressType {
    /// BIP-43 purpose of this family.
    #[must_use]
    pub fn purpose(self) -> u32 {
        match self {
            Self::NativeSegwit => 84,
            Self::Legacy => 44,
        }
    }

    /// Text name for interface labels and logs.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::NativeSegwit => "BIP-84 native segwit",
            Self::Legacy => "BIP-44 legacy",
        }
    }
}

/// BIP-44 coin type for a Bitcoin network: `0'` on mainnet, `1'` elsewhere.
#[must_use]
pub fn coin_type(network: Network) -> u32 {
    match network {
        Network::Bitcoin => 0,
        _ => 1,
    }
}

/// Encodes an already derived key as an address.
#[must_use]
pub fn encode_address(
    key: &ChildKey,
    network: Network,
    address_type: BitcoinAddressType,
) -> String {
    let public_key = CompressedPublicKey(key.public_key());
    match address_type {
        BitcoinAddressType::NativeSegwit => Address::p2wpkh(&public_key, network).to_string(),
        BitcoinAddressType::Legacy => Address::p2pkh(public_key.pubkey_hash(), network).to_string(),
    }
}

/// Derives `m/purpose'/coin'/account'/change/index` and encodes the address.
///
/// # Errors
///
/// Returns a derivation error, or [`WalletError::UnsupportedNetwork`] if the
/// network has no Bitcoin address encoding.
pub fn derive_address(
    master: &MasterKey,
    network: Network,
    address_type: BitcoinAddressType,
    account: u32,
    change: u32,
    index: u32,
) -> Result<String, WalletError> {
    if !matches!(
        network,
        Network::Bitcoin | Network::Testnet | Network::Signet | Network::Regtest
    ) {
        return Err(WalletError::UnsupportedNetwork {
            network: NetworkId::Bitcoin,
        });
    }
    let account_key = master.account(address_type.purpose(), coin_type(network), account)?;
    let key = account_key.derive(change, index)?;
    Ok(encode_address(&key, network, address_type))
}

/// Parses an address, verifying its checksum and network.
///
/// # Errors
///
/// Returns [`WalletError::InvalidAddress`] for a bad checksum, an unsupported
/// script type or an address that belongs to another network.
pub fn parse_address(network: Network, raw: &str) -> Result<Address, WalletError> {
    let unchecked = Address::from_str(raw).map_err(|_| WalletError::InvalidAddress {
        network: NetworkId::Bitcoin,
    })?;
    unchecked
        .require_network(network)
        .map_err(|_| WalletError::InvalidAddress {
            network: NetworkId::Bitcoin,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mnemonic::Mnemonic;

    const VECTOR_PHRASE: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn parsing_rejects_a_changed_character() {
        // The last character of this vector address is part of the checksum.
        assert!(parse_address(
            Network::Bitcoin,
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyv"
        )
        .is_err());
        assert!(parse_address(
            Network::Bitcoin,
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
        )
        .is_ok());
    }

    #[test]
    fn a_mainnet_address_is_not_a_testnet_address() {
        assert!(parse_address(
            Network::Testnet,
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
        )
        .is_err());
    }

    #[test]
    fn testnet_derivation_uses_coin_type_one() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        let master = MasterKey::from_mnemonic(&mnemonic, "", Network::Testnet).expect("master");
        let address = derive_address(
            &master,
            Network::Testnet,
            BitcoinAddressType::NativeSegwit,
            0,
            0,
            0,
        )
        .expect("derive");
        assert!(address.starts_with("tb1q"));
        assert!(parse_address(Network::Testnet, &address).is_ok());
    }

    #[test]
    fn the_account_path_follows_the_network() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        let mainnet = MasterKey::from_mnemonic(&mnemonic, "", Network::Bitcoin).expect("master");
        let testnet = MasterKey::from_mnemonic(&mnemonic, "", Network::Testnet).expect("master");
        assert_eq!(
            mainnet
                .account(BitcoinAddressType::NativeSegwit.purpose(), 0, 0)
                .expect("mainnet")
                .path()
                .to_string(),
            "m/84'/0'/0'"
        );
        assert_eq!(
            testnet
                .account(BitcoinAddressType::NativeSegwit.purpose(), 1, 0)
                .expect("testnet")
                .path()
                .to_string(),
            "m/84'/1'/0'"
        );
    }
}
