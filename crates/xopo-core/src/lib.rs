//! # XoPo Vault core
//!
//! The application layer above [`xopo_domain`]. It coordinates domain
//! aggregates and will later own persistence, security and synchronization
//! orchestration. Platform interfaces (Qt/C++ and Kotlin) talk to this crate,
//! never to storage directly.
//!
//! ## Current scope
//!
//! The [`Vault`] aggregate and its invariants, plus [`provision`], which turns a
//! key source into a wallet with real derived addresses. Persistence, including
//! encryption, lives in `xopo-storage`; this crate performs no I/O and stores no
//! key material — [`provision::create_wallet`] hands the [`xopo_wallet::KeySource`]
//! back to the caller instead of embedding it in the vault.
//!
//! It is still *not* a usable wallet: there is no signing, no balance retrieval
//! and no network access yet. Those arrive with the security and chain-adapter
//! phases, and only then may the interface present them as available.

pub mod provision;
pub mod vault;

pub use provision::{create_wallet, AccountRequest, CreatedWallet};
pub use vault::{Vault, VaultError};

/// The domain model, re-exported so platform bindings need one dependency.
pub use xopo_domain as domain;

/// Version of the core library, reported to the interface for diagnostics.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
