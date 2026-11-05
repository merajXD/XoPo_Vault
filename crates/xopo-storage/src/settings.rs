//! Vault-level metadata (`vault_meta`).
//!
//! Small string key/value pairs such as the vault name or the record of when it
//! was created. They live inside the encrypted file, so they are as protected as
//! wallet labels. No secret is ever stored here.

use rusqlite::{Connection, OptionalExtension};

use crate::error::StorageError;

/// Typed access to the `vault_meta` table.
#[derive(Debug)]
pub struct VaultMeta;

impl VaultMeta {
    /// Stores a value, replacing any previous value for the key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] for an empty key and
    /// [`StorageError::Sqlite`] when the write fails.
    pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), StorageError> {
        if key.is_empty() {
            return Err(StorageError::InvalidRecord("empty meta key".into()));
        }
        conn.execute(
            "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Reads a value, or `None` when the key was never written.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidRecord`] for an empty key and
    /// [`StorageError::Sqlite`] when the query fails.
    pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, StorageError> {
        if key.is_empty() {
            return Err(StorageError::InvalidRecord("empty meta key".into()));
        }
        Ok(conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Removes a key. Returns `true` when a row was deleted.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] when the delete fails.
    pub fn remove(conn: &Connection, key: &str) -> Result<bool, StorageError> {
        let removed = conn.execute("DELETE FROM vault_meta WHERE key = ?1", [key])?;
        Ok(removed > 0)
    }
}
