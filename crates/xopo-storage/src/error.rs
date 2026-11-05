//! Storage error type.
//!
//! Every failure carries either the SQLite error or enough context to explain
//! it. Nothing is swallowed: a vault that cannot be read must stop the
//! application, not silently become an empty vault.

use thiserror::Error;

use xopo_domain::{DomainError, WalletId};

/// Errors produced by the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// The underlying SQLite call failed.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// A stored value cannot be turned back into a valid domain value. This
    /// means the database was written by a different version or was tampered
    /// with after decryption succeeded.
    #[error("invalid stored value: {0}")]
    InvalidRecord(String),

    /// A domain invariant was violated while rebuilding an aggregate.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// A vault-level invariant was violated while rebuilding the aggregate.
    #[error(transparent)]
    Vault(#[from] xopo_core::VaultError),

    /// The file exists but the supplied key does not open it. A wrong key and a
    /// damaged file are reported the same way because SQLCipher does not
    /// distinguish them, and guessing would be misleading.
    #[error("cannot open vault: wrong key or damaged file")]
    WrongKeyOrCorrupt,

    /// The file was written by a newer schema than this build understands.
    #[error("vault schema version {found} is newer than supported version {supported}")]
    SchemaTooNew {
        /// Version recorded in the file.
        found: u32,
        /// Newest version this build can open.
        supported: u32,
    },

    /// The requested wallet does not exist.
    #[error("wallet {0} was not found")]
    WalletNotFound(WalletId),

    /// SQLite reported structural damage.
    #[error("integrity check failed: {0}")]
    Integrity(String),

    /// Filesystem access failed (pre-migration copy, for example).
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
}
