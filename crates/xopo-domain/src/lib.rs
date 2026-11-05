//! # XoPo Vault domain model
//!
//! Framework-free domain types shared by every platform: networks, assets,
//! exact amounts, addresses, derivation paths, wallets and accounts.
//!
//! ## Design rules
//!
//! * Untrusted input is validated once, at construction, by a fallible
//!   constructor ([`Label::parse`], [`Address::parse`], [`Amount::from_atomic`],
//!   …). Types carry their own guarantee afterwards.
//! * Deserialization runs the same validation as construction, so a crafted or
//!   corrupted payload cannot create an invalid value.
//! * No secret material is represented here. Keys and seed phrases belong to
//!   `xopo-security` and are referenced, never embedded.
//! * No floating point, no locale formatting, no clock inside value types.
//!
//! The crate has no network, filesystem or platform dependency so it can be
//! compiled unchanged into the Windows DLL and the Android `.so`.

pub mod account;
pub mod address;
pub mod amount;
pub mod asset;
pub mod derivation;
pub mod error;
pub mod ids;
pub mod network;
pub mod time;
pub mod validation;
pub mod wallet;

pub use account::Account;
pub use address::Address;
pub use amount::{Amount, MAX_DECIMALS};
pub use asset::{Asset, AssetId, AssetKind, TokenStandard};
pub use derivation::{DerivationPath, PathSegment, HARDENED_OFFSET, MAX_DEPTH};
pub use error::DomainError;
pub use ids::{AccountId, WalletId};
pub use network::{ChainFamily, DeliveryTier, NetworkId, NetworkInfo};
pub use time::Timestamp;
pub use validation::{Label, Symbol, MAX_LABEL_CHARS};
pub use wallet::{Wallet, WalletKind};
