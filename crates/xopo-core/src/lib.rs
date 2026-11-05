//! # XoPo Vault core
//!
//! The application layer above [`xopo_domain`]. It coordinates domain
//! aggregates and will later own persistence, security and synchronization
//! orchestration. Platform interfaces (Qt/C++ and Kotlin) talk to this crate,
//! never to storage directly.
//!
//! ## Current scope
//!
//! The [`Vault`] aggregate and its invariants. Persistence, including
//! encryption, lives in `xopo-storage`; this crate performs no I/O and holds no
//! key material. It is still *not* a usable wallet: there is no key derivation,
//! no signing, no balance retrieval and no network access yet. Those arrive with
//! the wallet, security and chain-adapter phases, and only then may the
//! interface present them as available.

pub mod vault;

pub use vault::{Vault, VaultError};

/// The domain model, re-exported so platform bindings need one dependency.
pub use xopo_domain as domain;

/// Version of the core library, reported to the interface for diagnostics.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
