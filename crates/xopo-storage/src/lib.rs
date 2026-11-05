//! # XoPo Vault storage
//!
//! Encrypted SQLite persistence for the vault.
//!
//! * The file is encrypted with SQLCipher, so labels, addresses, indexes and the
//!   write-ahead log are unreadable without the key.
//! * The key comes from the security layer; this crate never derives or stores
//!   it, and [`DatabaseKey`]'s `Debug` output is redacted.
//! * Money is stored as atomic decimal text because `u128` does not fit SQLite's
//!   signed 64-bit integer.
//! * Every write happens inside a transaction, and every row read back is
//!   rebuilt through the domain constructors, so stored values are validated
//!   exactly like untrusted input.
//!
//! Nothing here creates, signs or derives keys: this is a persistence layer, not
//! a wallet.

mod db;
mod error;
mod key;
mod migrations;
mod settings;
mod wallets;

pub use db::Database;
pub use error::StorageError;
pub use key::DatabaseKey;
pub use migrations::SCHEMA_VERSION;
pub use settings::VaultMeta;
pub use wallets::WalletRepository;
