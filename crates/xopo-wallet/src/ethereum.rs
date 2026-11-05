//! Ethereum addresses: BIP-44 derivation and EIP-55 checksums.
//!
//! An Ethereum address is the last 20 bytes of `Keccak-256` over the
//! uncompressed public key, without its `0x04` prefix. Keccak-256 comes from
//! `RustCrypto`'s `sha3`, not from this crate.
//!
//! Case matters: a mixed-case address carries an EIP-55 checksum over its own
//! hex text. All-lowercase and all-uppercase forms are accepted because the
//! checksum is optional on-chain, and both are common in the wild; mixed case
//! must verify, so a mistyped checksummed address is rejected.

use std::fmt::Write as _;

use bitcoin::secp256k1::PublicKey;
use sha3::{Digest, Keccak256};
use xopo_domain::NetworkId;

use crate::error::WalletError;
use crate::key::MasterKey;

/// SLIP-0044 coin type for EVM chains.
pub const COIN_TYPE: u32 = 60;

/// Derives `m/44'/60'/account'/0/index` and returns an EIP-55 address.
///
/// # Errors
///
/// Returns a derivation error.
pub fn derive_address(master: &MasterKey, account: u32, index: u32) -> Result<String, WalletError> {
    let account_key = master.account(44, COIN_TYPE, account)?;
    let key = account_key.derive(0, index)?;
    Ok(checksum_encode(&address_from_public_key(&key.public_key())))
}

/// The 20 address bytes of a secp256k1 public key.
#[must_use]
pub fn address_from_public_key(public_key: &PublicKey) -> [u8; 20] {
    let uncompressed = public_key.serialize_uncompressed();
    let digest = Keccak256::digest(&uncompressed[1..]);
    let mut address = [0_u8; 20];
    address.copy_from_slice(&digest[12..]);
    address
}

/// Renders an address with its EIP-55 checksum.
#[must_use]
pub fn checksum_encode(address: &[u8; 20]) -> String {
    let lower = hex_lower(address);
    let digest = Keccak256::digest(lower.as_bytes());
    let mut rendered = String::with_capacity(42);
    rendered.push_str("0x");
    for (index, character) in lower.chars().enumerate() {
        if !character.is_ascii_alphabetic() {
            rendered.push(character);
            continue;
        }
        let nibble = if index % 2 == 0 {
            digest[index / 2] >> 4
        } else {
            digest[index / 2] & 0x0f
        };
        if nibble >= 8 {
            rendered.push(character.to_ascii_uppercase());
        } else {
            rendered.push(character);
        }
    }
    rendered
}

/// Validates an address and returns its bytes.
///
/// # Errors
///
/// Returns [`WalletError::InvalidAddress`] when the prefix, length, characters
/// or EIP-55 checksum are wrong.
pub fn parse_address(raw: &str) -> Result<[u8; 20], WalletError> {
    let body = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .ok_or(WalletError::InvalidAddress {
            network: NetworkId::Ethereum,
        })?;
    if body.len() != 40 || !body.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(WalletError::InvalidAddress {
            network: NetworkId::Ethereum,
        });
    }
    let mut address = [0_u8; 20];
    for (index, pair) in body.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| WalletError::InvalidAddress {
            network: NetworkId::Ethereum,
        })?;
        address[index] = u8::from_str_radix(text, 16).map_err(|_| WalletError::InvalidAddress {
            network: NetworkId::Ethereum,
        })?;
    }

    let mixed_case = body.bytes().any(|byte| byte.is_ascii_lowercase())
        && body.bytes().any(|byte| byte.is_ascii_uppercase());
    if mixed_case && checksum_encode(&address) != format!("0x{body}") {
        return Err(WalletError::InvalidAddress {
            network: NetworkId::Ethereum,
        });
    }
    Ok(address)
}

fn hex_lower(address: &[u8; 20]) -> String {
    let mut rendered = String::with_capacity(40);
    for byte in address {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vectors from EIP-55 itself.
    const EIP55_VECTORS: [&str; 4] = [
        "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
        "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
        "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
        "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
    ];

    #[test]
    fn eip55_vectors_round_trip() {
        for vector in EIP55_VECTORS {
            let bytes = parse_address(vector).expect("valid checksum");
            assert_eq!(checksum_encode(&bytes), vector);
        }
    }

    #[test]
    fn lowercase_and_uppercase_forms_are_accepted() {
        let lower = EIP55_VECTORS[0].to_ascii_lowercase();
        let upper = format!("0x{}", EIP55_VECTORS[0][2..].to_ascii_uppercase());
        assert!(parse_address(&lower).is_ok());
        assert!(parse_address(&upper).is_ok());
    }

    #[test]
    fn a_broken_checksum_is_rejected() {
        let broken = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD";
        assert!(parse_address(broken).is_err());
    }

    #[test]
    fn malformed_addresses_are_rejected() {
        assert!(parse_address("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed").is_err());
        assert!(parse_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAe").is_err());
        assert!(parse_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAez").is_err());
    }
}
