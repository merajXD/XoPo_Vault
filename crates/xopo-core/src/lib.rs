//! # XoPo Vault core
//!
//! The application layer above [`xopo_domain`]. It coordinates domain
//! aggregates and will later own persistence, security and synchronization
//! orchestration. Platform interfaces (Qt/C++ and Kotlin) talk to this crate,
//! never to storage directly.
//!
//! ## Current scope
//!
//! The in-memory [`Vault`] aggregate and its invariants. It is deliberately
//! *not* a complete vault: there is no persistence, encryption, key handling,
//! balance retrieval or network access yet. Those arrive with the storage,
//! security, wallet and chain-adapter phases, and only then may the interface
//! present them as available.

pub mod vault;

pub use vault::{Vault, VaultError};

/// The domain model, re-exported so platform bindings need one dependency.
pub use xopo_domain as domain;

/// Version of the core library, reported to the interface for diagnostics.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
