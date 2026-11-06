//! # XoPo Vault engine
//!
//! The one crate allowed to put the three layers together: `xopo-core` (vault
//! rules), `xopo-storage` (encrypted SQLite) and `xopo-security` (KDF, envelope,
//! sealed secrets). Interfaces talk to this crate instead of wiring those layers
//! themselves, so the ordering rules — derive first, seal second, write last —
//! live in one place and are tested there.
//!
//! ## The vault on disk
//!
//! ```text
//! demo.xopo       SQLCipher file, keyed with the vault's random data key
//! demo.xopo.key   90-byte envelope: the data key, wrapped with the password
//! ```
//!
//! The envelope sits *beside* the database, not inside it: the data key it wraps
//! is what opens the database, so an envelope stored in a table could never be
//! read. See `docs/SECURITY.md`.
//!
//! ## What a call can and cannot do
//!
//! * Nothing is written before every fallible step has succeeded; a failed
//!   `add_wallet` reloads the vault from disk so memory never disagrees with the
//!   file.
//! * A wrong password and a damaged envelope are reported the same way.
//! * The data key never leaves this crate except as bytes for the database open
//!   call, and the phrase only comes back through [`VaultHandle::reveal_phrase`].
//!
//! There is no signing, no balance retrieval and no network access here.

pub mod error;
pub mod vault;

pub use error::EngineError;
pub use vault::{
    create_vault, create_vault_with_params, open_vault, AddedWallet, VaultHandle, VaultPaths,
};

/// Longest accepted vault name, matching the domain's label limit.
pub use xopo_domain::Label;

/// Smallest password the engine accepts.
///
/// This is a development threshold, not a security claim: Argon2id makes a short
/// password expensive to guess, not impossible. The interface should enforce
/// more (length, a word list) before this crate is used with real funds.
pub const MIN_PASSWORD_LEN: usize = 8;
