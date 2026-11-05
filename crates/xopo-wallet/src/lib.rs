//! # XoPo Vault key layer
//!
//! Mnemonics, hierarchical keys and address encoding. This crate is the only
//! place in the workspace where secret material exists.
//!
//! ## Design rules
//!
//! * No cryptography is implemented here. BIP-39 and BIP-32 come from the
//!   `rust-bitcoin` crates, secp256k1 from `libsecp256k1`, Keccak-256 from
//!   `RustCrypto`. This crate composes them and checks the result against
//!   published test vectors.
//! * Secret types never implement `Debug` in a form that prints key material,
//!   `Display`, or `Serialize`. A mnemonic cannot be written to JSON by
//!   accident.
//! * A watch-only wallet is a *different type* with no key-producing method, so
//!   it cannot reach a signing path by flipping a flag.
//! * Addresses are encoded by the chain module that owns them and are checked
//!   against published vectors, not against this crate's own output.
//!
//! ## What this crate does not do
//!
//! It does not sign, broadcast, store, encrypt or display anything. Key
//! material is not persisted here; that is `xopo-security` and `xopo-storage`
//! work in later phases.

#![warn(missing_docs)]

pub mod adapter;
pub mod key;
pub mod mnemonic;
pub mod source;

#[cfg(feature = "bitcoin")]
pub mod bitcoin;
#[cfg(feature = "ethereum")]
pub mod ethereum;
#[cfg(feature = "litecoin")]
pub mod litecoin;

mod error;

pub use error::WalletError;
pub use key::{AccountKey, ChildKey, MasterKey, Seed, Slip132};
pub use mnemonic::{Mnemonic, WordCount};
pub use source::{KeySource, WatchOnlyAccount};
