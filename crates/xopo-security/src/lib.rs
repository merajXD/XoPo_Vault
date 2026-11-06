//! # XoPo Vault security
//!
//! Password-based key management for the vault.
//!
//! The hierarchy is the one documented in `docs/SECURITY.md`:
//!
//! ```text
//! password ──Argon2id(salt, params)──► KEK ──AEAD──► DEK ──AEAD──► secrets
//! ```
//!
//! * [`KeyEnvelope`] stores a random 32-byte data key (DEK) wrapped by a
//!   password-derived key (KEK). The KDF parameters and salt travel with the
//!   envelope, so changing the password re-wraps 90 bytes instead of rewriting
//!   the database.
//! * [`SealedSecret`] encrypts an individual secret (a seed phrase, a private
//!   key) with the DEK, bound to a context string so a blob cannot be moved to
//!   another wallet and still open.
//! * [`SecretKey`] is zeroized on drop, is not `Clone`, and redacts itself from
//!   `Debug` output; wrong passwords and tampered bytes are the same error, so
//!   the failure does not say which of the two happened.
//!
//! What this crate does **not** do: it never stores, logs or serializes a
//! password, and it does not talk to the operating system's keystore. DPAPI and
//! the Android Keystore live behind a platform trait in a later step, and the
//! application lock (attempt throttling, auto-lock) is not implemented yet.
//! Rust cannot promise that the operating system kept no copy of a key, so the
//! zeroization here is defence in depth, not a guarantee.

mod aead;
mod envelope;
mod error;
mod kdf;
mod key;
mod rng;
mod secret;

pub use envelope::{KeyEnvelope, ENVELOPE_LEN, ENVELOPE_VERSION};
pub use error::SecurityError;
pub use kdf::KdfParams;
pub use key::SecretKey;
pub use secret::{SealedSecret, MAX_SECRET_LEN};
