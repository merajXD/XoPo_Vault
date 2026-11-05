//! Litecoin legacy addresses: BIP-44 P2PKH.
//!
//! Litecoin uses the same secp256k1 keys and the same hash160 as Bitcoin; only
//! the Base58Check version byte differs (`0x30`, so addresses start with `L`).
//! The checksum is `bs58`'s double-SHA256 Base58Check, shared with the Litecoin
//! vector in `tests/vectors.rs`.
//!
//! Bech32 Litecoin addresses (`ltc1…`, BIP-84) are not implemented in this
//! phase; the derivation itself is identical to Bitcoin's, but encoding and
//! witness handling are deferred with the rest of Litecoin's script support.

use bitcoin::hashes::Hash;
use bitcoin::CompressedPublicKey;
use xopo_domain::NetworkId;

use crate::error::WalletError;
use crate::key::MasterKey;

/// SLIP-0044 coin type for Litecoin.
pub const COIN_TYPE: u32 = 2;

/// Base58Check version byte of a Litecoin P2PKH address.
pub const P2PKH_VERSION: u8 = 0x30;

/// Encodes a key as a Litecoin P2PKH address.
#[must_use]
pub fn address_from_public_key(public_key: &CompressedPublicKey) -> String {
    let key_hash = public_key.wpubkey_hash().to_byte_array();
    let mut payload = Vec::with_capacity(21);
    payload.push(P2PKH_VERSION);
    payload.extend_from_slice(&key_hash);
    bs58::encode(payload).with_check().into_string()
}

/// Derives `m/44'/2'/account'/change/index` and encodes a Litecoin address.
///
/// # Errors
///
/// Returns a derivation error.
pub fn derive_address(
    master: &MasterKey,
    account: u32,
    change: u32,
    index: u32,
) -> Result<String, WalletError> {
    let account_key = master.account(44, COIN_TYPE, account)?;
    let key = account_key.derive(change, index)?;
    Ok(address_from_public_key(&CompressedPublicKey(
        key.public_key(),
    )))
}

/// Validates a Litecoin P2PKH address and returns its hash160.
///
/// # Errors
///
/// Returns [`WalletError::InvalidAddress`] for a bad checksum, a wrong version
/// byte or a length that is not 21 bytes of payload.
pub fn parse_address(raw: &str) -> Result<[u8; 20], WalletError> {
    let invalid = WalletError::InvalidAddress {
        network: NetworkId::Litecoin,
    };
    let decoded =
        bs58::decode(raw)
            .with_check(None)
            .into_vec()
            .map_err(|_| WalletError::InvalidAddress {
                network: NetworkId::Litecoin,
            })?;
    if decoded.len() != 21 || decoded[0] != P2PKH_VERSION {
        return Err(invalid);
    }
    let mut key_hash = [0_u8; 20];
    key_hash.copy_from_slice(&decoded[1..]);
    Ok(key_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_character_is_rejected() {
        // Flipping the last character breaks the Base58Check checksum.
        assert!(parse_address("LUWPbpM43E2p7ZSh8cyTBEkvpHmr3cB8Ez").is_ok());
        assert!(parse_address("LUWPbpM43E2p7ZSh8cyTBEkvpHmr3cB8Ey").is_err());
    }

    #[test]
    fn a_bitcoin_address_is_not_a_litecoin_address() {
        assert!(parse_address("1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA").is_err());
    }
}
