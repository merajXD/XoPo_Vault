//! BIP-32 master, account and child keys.
//!
//! Derivation itself is `rust-bitcoin`'s `bip32` module, which is checked
//! against the BIP-32 test vectors upstream. This module adds:
//!
//! * Paths expressed with the already validated
//!   [`xopo_domain::DerivationPath`], so an index can never exceed 2^31 here.
//! * Redacted `Debug` for every key type.
//! * SLIP-132 re-tagging (`zprv`/`zpub`), which is a four-byte version change,
//!   not cryptography.
//! * A [`Seed`] that wipes itself on drop.
//!
//! ## What is not protected yet
//!
//! The extended keys are held by `bitcoin::bip32::Xpriv`, which does **not**
//! zeroize its memory on drop. Wiping key memory, wrapping it in a platform
//! keystore and locking it behind a password is Phase 4 (`xopo-security`). Until
//! then this crate must not be used to hold real funds.

use std::fmt;
use std::fmt::Write as _;

use bitcoin::bip32::{ChildNumber, Xpriv, Xpub};
use bitcoin::secp256k1::{PublicKey, Secp256k1};
use bitcoin::Network;
use xopo_domain::{DerivationPath, PathSegment};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::WalletError;
use crate::mnemonic::Mnemonic;

/// Version bytes of a mainnet `xprv`, used to detect foreign keys.
const MAINNET_XPRV_VERSION: [u8; 4] = [0x04, 0x88, 0xAD, 0xE4];
/// Version bytes of a mainnet `xpub`.
const MAINNET_XPUB_VERSION: [u8; 4] = [0x04, 0x88, 0xB2, 0x1E];

/// A 64-byte BIP-39 seed, wiped on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Seed([u8; 64]);

impl Seed {
    pub(crate) fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Seed(64 bytes, redacted)")
    }
}

/// The root of a BIP-32 tree.
pub struct MasterKey {
    xpriv: Xpriv,
    network: Network,
}

impl MasterKey {
    /// Builds the root key from a seed.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::MasterKey`] if the seed is unusable.
    pub fn from_seed(seed: &Seed, network: Network) -> Result<Self, WalletError> {
        let xpriv =
            Xpriv::new_master(network, seed.as_bytes()).map_err(|_| WalletError::MasterKey)?;
        Ok(Self { xpriv, network })
    }

    /// Builds the root key from a mnemonic and an optional BIP-39 passphrase.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::MasterKey`] if the derived seed is unusable.
    pub fn from_mnemonic(
        mnemonic: &Mnemonic,
        passphrase: &str,
        network: Network,
    ) -> Result<Self, WalletError> {
        let seed = mnemonic.seed(passphrase);
        Self::from_seed(&seed, network)
    }

    /// Network this tree was built for; it only affects serialisation prefixes.
    #[must_use]
    pub fn network(&self) -> Network {
        self.network
    }

    /// Master fingerprint as eight hex characters (public, safe to log).
    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.xpriv.fingerprint(&Secp256k1::new()).to_string()
    }

    /// Root key in `xprv` form. This is a secret.
    #[must_use]
    pub fn xprv(&self) -> String {
        self.xpriv.to_string()
    }

    /// Root public key in `xpub` form.
    #[must_use]
    pub fn xpub(&self) -> String {
        Xpub::from_priv(&Secp256k1::new(), &self.xpriv).to_string()
    }

    /// Derives an arbitrary validated path.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::DerivationFailed`] if BIP-32 rejects the child.
    pub fn derive(&self, path: &DerivationPath) -> Result<ChildKey, WalletError> {
        let xpriv = self.derive_xpriv(path)?;
        Ok(ChildKey {
            xpriv,
            path: path.clone(),
        })
    }

    /// Derives an account key `m/purpose'/coin'/account'`.
    ///
    /// # Errors
    ///
    /// Returns a path error for out-of-range components and
    /// [`WalletError::DerivationFailed`] if BIP-32 rejects the child.
    pub fn account(
        &self,
        purpose: u32,
        coin_type: u32,
        account: u32,
    ) -> Result<AccountKey, WalletError> {
        let path = DerivationPath::for_account(purpose, coin_type, account)?;
        let xpriv = self.derive_xpriv(&path)?;
        Ok(AccountKey {
            xpriv,
            path,
            purpose,
        })
    }

    fn derive_xpriv(&self, path: &DerivationPath) -> Result<Xpriv, WalletError> {
        let numbers = child_numbers(path)?;
        self.xpriv
            .derive_priv(&Secp256k1::new(), &numbers)
            .map_err(|_| WalletError::DerivationFailed)
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "MasterKey({}, fingerprint {}, redacted)",
            self.network,
            self.fingerprint()
        )
    }
}

/// An account-level key, `m/purpose'/coin'/account'`.
pub struct AccountKey {
    xpriv: Xpriv,
    path: DerivationPath,
    purpose: u32,
}

impl AccountKey {
    /// Path of this account.
    #[must_use]
    pub fn path(&self) -> &DerivationPath {
        &self.path
    }

    /// BIP-43 purpose that produced this account.
    #[must_use]
    pub fn purpose(&self) -> u32 {
        self.purpose
    }

    /// Derives `change/index` below the account.
    ///
    /// # Errors
    ///
    /// Returns a path error for out-of-range components and
    /// [`WalletError::DerivationFailed`] if BIP-32 rejects the child.
    pub fn derive(&self, change: u32, index: u32) -> Result<ChildKey, WalletError> {
        let path = self.child_path(change, index)?;
        let numbers = [
            ChildNumber::from_normal_idx(change)
                .map_err(|_| WalletError::InvalidChildIndex { index: change })?,
            ChildNumber::from_normal_idx(index)
                .map_err(|_| WalletError::InvalidChildIndex { index })?,
        ];
        let xpriv = self
            .xpriv
            .derive_priv(&Secp256k1::new(), &numbers)
            .map_err(|_| WalletError::DerivationFailed)?;
        Ok(ChildKey { xpriv, path })
    }

    /// Account key in `xprv` form. This is a secret.
    #[must_use]
    pub fn xprv(&self) -> String {
        self.xpriv.to_string()
    }

    /// Account key in `xpub` form.
    #[must_use]
    pub fn xpub(&self) -> String {
        Xpub::from_priv(&Secp256k1::new(), &self.xpriv).to_string()
    }

    /// SLIP-132 prefix that matches this account's purpose.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnknownPurpose`] for a purpose without a
    /// registered prefix.
    pub fn slip132(&self) -> Result<Slip132, WalletError> {
        Slip132::for_purpose(self.purpose).ok_or(WalletError::UnknownPurpose {
            purpose: self.purpose,
        })
    }

    /// Account key with its SLIP-132 private prefix (`zprv` for BIP-84).
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnknownPurpose`] or
    /// [`WalletError::UnexpectedExtendedKeyVersion`] for a non-mainnet key.
    pub fn slip132_xprv(&self) -> Result<String, WalletError> {
        retag(&self.xprv(), self.slip132()?.private_version())
    }

    /// Account key with its SLIP-132 public prefix (`zpub` for BIP-84).
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnknownPurpose`] or
    /// [`WalletError::UnexpectedExtendedKeyVersion`] for a non-mainnet key.
    pub fn slip132_xpub(&self) -> Result<String, WalletError> {
        retag(&self.xpub(), self.slip132()?.public_version())
    }

    fn child_path(&self, change: u32, index: u32) -> Result<DerivationPath, WalletError> {
        let mut segments: Vec<PathSegment> = self.path.segments().to_vec();
        segments.push(PathSegment::new(change, false)?);
        segments.push(PathSegment::new(index, false)?);
        Ok(DerivationPath::from_segments(segments)?)
    }
}

impl fmt::Debug for AccountKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "AccountKey({}, redacted)", self.path)
    }
}

/// A derived child key with its full path.
pub struct ChildKey {
    xpriv: Xpriv,
    path: DerivationPath,
}

impl ChildKey {
    /// Full path of this key.
    #[must_use]
    pub fn path(&self) -> &DerivationPath {
        &self.path
    }

    /// Public key on the secp256k1 curve.
    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_secret_key(&Secp256k1::new(), &self.xpriv.private_key)
    }

    /// Compressed public key as 33 hex characters, the form test vectors use.
    #[must_use]
    pub fn public_key_hex(&self) -> String {
        let bytes = self.public_key().serialize();
        let mut rendered = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(rendered, "{byte:02x}");
        }
        rendered
    }

    /// Child key in `xprv` form. This is a secret.
    #[must_use]
    pub fn xprv(&self) -> String {
        self.xpriv.to_string()
    }

    /// Child public key in `xpub` form.
    #[must_use]
    pub fn xpub(&self) -> String {
        Xpub::from_priv(&Secp256k1::new(), &self.xpriv).to_string()
    }
}

impl fmt::Debug for ChildKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ChildKey({}, redacted)", self.path)
    }
}

/// Registered SLIP-132 extended-key prefixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slip132 {
    /// BIP-44 legacy: `xprv`/`xpub`.
    Bip44,
    /// BIP-49 nested segwit: `yprv`/`ypub`.
    Bip49,
    /// BIP-84 native segwit: `zprv`/`zpub`.
    Bip84,
}

impl Slip132 {
    /// Prefix registered for a BIP-43 purpose.
    #[must_use]
    pub fn for_purpose(purpose: u32) -> Option<Self> {
        match purpose {
            44 => Some(Self::Bip44),
            49 => Some(Self::Bip49),
            84 => Some(Self::Bip84),
            _ => None,
        }
    }

    /// BIP-43 purpose of this prefix.
    #[must_use]
    pub fn purpose(self) -> u32 {
        match self {
            Self::Bip44 => 44,
            Self::Bip49 => 49,
            Self::Bip84 => 84,
        }
    }

    /// Four private-key version bytes.
    #[must_use]
    pub fn private_version(self) -> [u8; 4] {
        match self {
            Self::Bip44 => MAINNET_XPRV_VERSION,
            Self::Bip49 => [0x04, 0x9D, 0x78, 0x78],
            Self::Bip84 => [0x04, 0xB2, 0x43, 0x0C],
        }
    }

    /// Four public-key version bytes.
    #[must_use]
    pub fn public_version(self) -> [u8; 4] {
        match self {
            Self::Bip44 => MAINNET_XPUB_VERSION,
            Self::Bip49 => [0x04, 0x9D, 0x7C, 0xB2],
            Self::Bip84 => [0x04, 0xB2, 0x47, 0x46],
        }
    }
}

/// Replaces the version bytes of a mainnet extended key and fixes the checksum.
///
/// Only the prefix changes; the key data and depth stay identical, which is why
/// this is safe to do without touching key material.
fn retag(extended: &str, version: [u8; 4]) -> Result<String, WalletError> {
    let mut payload = bs58::decode(extended)
        .with_check(None)
        .into_vec()
        .map_err(|_| WalletError::MalformedExtendedKey {
            kind: "extended key",
        })?;
    if payload.len() != 78 {
        return Err(WalletError::MalformedExtendedKey {
            kind: "extended key",
        });
    }
    let current = &payload[..4];
    if current != MAINNET_XPRV_VERSION && current != MAINNET_XPUB_VERSION {
        let mut rendered = String::new();
        for byte in current {
            let _ = write!(rendered, "{byte:02x}");
        }
        return Err(WalletError::UnexpectedExtendedKeyVersion { version: rendered });
    }
    payload[..4].copy_from_slice(&version);
    Ok(bs58::encode(payload).with_check().into_string())
}

fn child_numbers(path: &DerivationPath) -> Result<Vec<ChildNumber>, WalletError> {
    path.segments()
        .iter()
        .map(|segment| {
            let result = if segment.is_hardened() {
                ChildNumber::from_hardened_idx(segment.index())
            } else {
                ChildNumber::from_normal_idx(segment.index())
            };
            result.map_err(|_| WalletError::InvalidChildIndex {
                index: segment.index(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTOR_PHRASE: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn official_bip39_vector_master_key_matches() {
        // Seed of vector #1 with passphrase "TREZOR", from Trezor vectors.json.
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        let master = MasterKey::from_mnemonic(&mnemonic, "", Network::Bitcoin).expect("master");
        assert_eq!(
            master.xprv(),
            "xprv9s21ZrQH143K3GJpoapnV8SFfukcVBSfeCficPSGfubmSFDxo1kuHnLisriDvSnRRuL2Qrg\
5ggqHKNVpxR86QEC8w35uxmGoggxtQTPvfUu"
        );
    }

    #[test]
    fn paths_lower_below_the_account_are_rejected_when_out_of_range() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        let master = MasterKey::from_mnemonic(&mnemonic, "", Network::Bitcoin).expect("master");
        let account = master.account(84, 0, 0).expect("account");
        assert!(account.derive(xopo_domain::HARDENED_OFFSET, 0).is_err());
        assert!(account.derive(0, xopo_domain::HARDENED_OFFSET).is_err());
    }

    #[test]
    fn debug_output_never_contains_key_material() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        let master = MasterKey::from_mnemonic(&mnemonic, "", Network::Bitcoin).expect("master");
        let debug = format!("{master:?}");
        assert!(debug.contains("redacted"));
        assert!(!debug.contains("xprv"));
    }
}
