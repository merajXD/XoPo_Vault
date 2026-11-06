//! Engine errors.
//!
//! Every variant says what the user can act on. No variant carries a password, a
//! key, a phrase or a path from the user's home directory.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by vault operations.
#[derive(Debug, Error)]
pub enum EngineError {
    /// No database at the chosen path.
    #[error("no vault found at {0}")]
    VaultNotFound(PathBuf),

    /// A vault file or its key envelope already exists at the chosen path.
    #[error("a vault already exists at {0}")]
    AlreadyExists(PathBuf),

    /// The database is there but the envelope is not, so the password has
    /// nothing to unlock. Reported instead of guessing the key.
    #[error("vault key file is missing: {0}")]
    MissingKeyFile(PathBuf),

    /// The password did not unwrap the data key, or the envelope was altered.
    /// The two are deliberately indistinguishable.
    #[error("wrong password, or the vault key file was damaged")]
    WrongPassword,

    /// The password is shorter than [`crate::MIN_PASSWORD_LEN`].
    #[error("password must be at least {minimum} characters")]
    WeakPassword {
        /// Smallest accepted length.
        minimum: usize,
    },

    /// The new password was the same as the current one.
    #[error("the new password is the same as the current password")]
    PasswordUnchanged,

    /// A wallet has no sealed phrase: either it is watch-only or its secret was
    /// removed. Revealing is refused rather than returning an empty phrase.
    #[error("wallet {0} has no sealed recovery phrase")]
    NoRecoveryPhrase(xopo_domain::WalletId),

    /// A sealed phrase decrypted but is not valid UTF-8, which means the record
    /// was damaged in a way the AEAD tag did not catch (it cannot happen without
    /// a key mismatch, but guessing is not an option).
    #[error("the sealed phrase for wallet {0} is unreadable")]
    CorruptPhrase(xopo_domain::WalletId),

    /// The vault name, wallet label or network list was rejected.
    #[error(transparent)]
    Domain(#[from] xopo_domain::DomainError),

    /// Vault-level invariant or provisioning failure.
    #[error(transparent)]
    Core(#[from] xopo_core::VaultError),

    /// Key derivation, sealing or opening failed.
    #[error(transparent)]
    Security(#[from] xopo_security::SecurityError),

    /// Database, schema or integrity failure.
    #[error(transparent)]
    Storage(#[from] xopo_storage::StorageError),

    /// Phrase parsing or derivation failed.
    #[error(transparent)]
    Wallet(#[from] xopo_wallet::WalletError),

    /// Reading or writing the vault files failed.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
}
