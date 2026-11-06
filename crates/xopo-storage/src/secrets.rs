//! Sealed secrets: the per-wallet encrypted payloads.
//!
//! This repository moves opaque, already-sealed records between the database and
//! the security layer. It never sees a password, never derives anything and
//! never decrypts: [`xopo_security::SealedSecret`] validates its own bytes on the
//! way out, so a row edited directly in the file is reported instead of trusted.
//!
//! The password-wrapped data key is **not** stored here. It lives in a small
//! sidecar file next to the database: a key store inside the file it unlocks
//! cannot be read without the key it holds. See `docs/SECURITY.md`.

use rusqlite::{params, Connection, OptionalExtension};

use xopo_domain::Timestamp;
use xopo_security::SealedSecret;

use crate::error::StorageError;

/// Longest accepted secret context. Contexts are `wallet:<uuid>`, so this is
/// generous while keeping a corrupt row from allocating without bound.
const MAX_CONTEXT_LEN: usize = 256;

/// Reads and writes sealed secrets.
#[derive(Debug)]
pub struct SecretRepository;

impl SecretRepository {
    /// Inserts or replaces the sealed secret for `context`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] for an empty or over-long
    /// context, and [`StorageError::Sqlite`] if the write fails.
    pub fn save(
        conn: &mut Connection,
        context: &str,
        secret: &SealedSecret,
        at: Timestamp,
    ) -> Result<(), StorageError> {
        validate_context(context)?;
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO sealed_secrets (context, payload, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(context) DO UPDATE SET payload = excluded.payload,
                                                updated_at = excluded.updated_at",
            params![context, secret.to_bytes(), at.as_millis()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads the sealed secret for `context`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] for an unreadable context or a
    /// payload that is not a sealed secret.
    pub fn load(conn: &Connection, context: &str) -> Result<Option<SealedSecret>, StorageError> {
        validate_context(context)?;
        let stored: Option<Vec<u8>> = conn
            .query_row(
                "SELECT payload FROM sealed_secrets WHERE context = ?1",
                [context],
                |row| row.get(0),
            )
            .optional()?;
        stored
            .map(|bytes| {
                SealedSecret::from_bytes(&bytes).map_err(|error| {
                    StorageError::InvalidRecord(format!("stored sealed secret: {error}"))
                })
            })
            .transpose()
    }

    /// Deletes the sealed secret for `context`, returning whether one existed.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] for an unreadable context and
    /// [`StorageError::Sqlite`] if the delete fails.
    pub fn delete(conn: &Connection, context: &str) -> Result<bool, StorageError> {
        validate_context(context)?;
        let removed = conn.execute("DELETE FROM sealed_secrets WHERE context = ?1", [context])?;
        Ok(removed > 0)
    }

    /// Number of sealed secrets in the vault.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] on a failed query.
    pub fn count(conn: &Connection) -> Result<usize, StorageError> {
        let count: i64 =
            conn.query_row("SELECT count(*) FROM sealed_secrets", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(usize::MAX))
    }
}

fn validate_context(context: &str) -> Result<(), StorageError> {
    if context.is_empty() {
        return Err(StorageError::InvalidRecord("empty secret context".into()));
    }
    if context.len() > MAX_CONTEXT_LEN {
        return Err(StorageError::InvalidRecord(format!(
            "secret context longer than {MAX_CONTEXT_LEN} bytes"
        )));
    }
    Ok(())
}
