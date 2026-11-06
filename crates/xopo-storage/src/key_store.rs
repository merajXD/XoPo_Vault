//! The key store: the wrapped data key and the secrets sealed with it.
//!
//! This repository moves opaque, already-sealed records between the database and
//! the security layer. It never sees a password, never derives anything and
//! never decrypts: [`xopo_security::KeyEnvelope`] and
//! [`xopo_security::SealedSecret`] validate their own bytes on the way out, so a
//! row that was edited directly in the file is reported instead of trusted.
//!
//! Storing the envelope inside the SQLCipher file means the key material is
//! protected twice: by the database key and by the password that unwraps the
//! envelope. Losing either one leaves the sealed secrets unreadable, which is
//! why the `.xdvault` backup (Phase 4) is the documented recovery route.

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use xopo_domain::Timestamp;
use xopo_security::{KeyEnvelope, SealedSecret};

use crate::error::StorageError;

/// Identifier of the single vault-wide key envelope.
pub const VAULT_KEY_ID: &str = "vault";

/// Longest accepted secret context. Contexts are `wallet:<uuid>`, so this is
/// generous while keeping a corrupt row from allocating without bound.
const MAX_CONTEXT_LEN: usize = 256;

/// Reads and writes the key store.
#[derive(Debug)]
pub struct KeyStoreRepository;

impl KeyStoreRepository {
    /// Inserts or replaces the vault key envelope.
    ///
    /// `created_at` is preserved when a row already exists, so re-wrapping the
    /// data key under a new password does not pretend the vault was created
    /// today.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] if the transaction fails.
    pub fn save_key(
        conn: &mut Connection,
        envelope: &KeyEnvelope,
        at: Timestamp,
    ) -> Result<(), StorageError> {
        let transaction = conn.transaction()?;
        save_key_in(&transaction, envelope, at)?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads the vault key envelope, if one has been stored.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] when the stored blob is not a
    /// valid envelope, which means the database was tampered with.
    pub fn load_key(conn: &Connection) -> Result<Option<KeyEnvelope>, StorageError> {
        let stored: Option<Vec<u8>> = conn
            .query_row(
                "SELECT envelope FROM key_store WHERE id = ?1",
                [VAULT_KEY_ID],
                |row| row.get(0),
            )
            .optional()?;
        stored
            .map(|bytes| {
                KeyEnvelope::from_bytes(&bytes).map_err(|error| {
                    StorageError::InvalidRecord(format!("stored key envelope: {error}"))
                })
            })
            .transpose()
    }

    /// Whether a key envelope has been stored.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] on a failed query.
    pub fn has_key(conn: &Connection) -> Result<bool, StorageError> {
        let count: i64 = conn.query_row("SELECT count(*) FROM key_store", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    /// Inserts or replaces the sealed secret for `context`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] for an empty or over-long
    /// context, and [`StorageError::Sqlite`] if the write fails.
    pub fn save_secret(
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
    pub fn load_secret(
        conn: &Connection,
        context: &str,
    ) -> Result<Option<SealedSecret>, StorageError> {
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
    pub fn delete_secret(conn: &Connection, context: &str) -> Result<bool, StorageError> {
        validate_context(context)?;
        let removed = conn.execute("DELETE FROM sealed_secrets WHERE context = ?1", [context])?;
        Ok(removed > 0)
    }

    /// Number of sealed secrets in the vault.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] on a failed query.
    pub fn secret_count(conn: &Connection) -> Result<usize, StorageError> {
        let count: i64 =
            conn.query_row("SELECT count(*) FROM sealed_secrets", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(usize::MAX))
    }
}

fn save_key_in(
    transaction: &Transaction<'_>,
    envelope: &KeyEnvelope,
    at: Timestamp,
) -> Result<(), StorageError> {
    let bytes = envelope.to_bytes();
    transaction.execute(
        "INSERT INTO key_store (id, envelope, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT(id) DO UPDATE SET envelope = excluded.envelope,
                                       updated_at = excluded.updated_at",
        params![VAULT_KEY_ID, bytes, at.as_millis()],
    )?;
    Ok(())
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
