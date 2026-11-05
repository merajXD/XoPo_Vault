//! The vault database: open, configure, verify and migrate.
//!
//! The connection is encrypted with SQLCipher (page-level AES), so indexes, the
//! WAL and any temporary structures are encrypted as well. Opening is the only
//! place the key is used; it is never stored by this crate.

use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StorageError;
use crate::key::DatabaseKey;
use crate::migrations::{self, SCHEMA_VERSION};

/// An open, encrypted vault database.
pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The path itself can reveal the user's account name, so it stays out.
        f.write_str("Database(..)")
    }
}

impl Database {
    /// Opens an existing vault, or creates a new one, with the given key.
    ///
    /// The key is verified before anything else happens: reading the schema is
    /// the first operation that fails when the file was not encrypted with this
    /// key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::WrongKeyOrCorrupt`] when the file exists but
    /// cannot be decrypted, [`StorageError::Integrity`] when SQLite reports
    /// damage, and [`StorageError::Sqlite`] for other failures.
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open(&path)?;

        // `PRAGMA key` must be the first statement after opening. The literal is
        // built from a fixed 32-byte array, never from user text.
        conn.execute_batch(&format!("PRAGMA key = \"{}\";", key.pragma_literal()))?;

        // First read: with the wrong key this fails with "file is not a database".
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| match error {
            rusqlite::Error::SqliteFailure(_, _) => StorageError::WrongKeyOrCorrupt,
            other => StorageError::Sqlite(other),
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = FULL;
             PRAGMA busy_timeout = 5000;",
        )?;

        let database = Self { conn, path };
        database.integrity_check()?;
        Ok(database)
    }

    /// Filesystem path of the vault file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Schema version recorded in the file.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlite`] when the pragma cannot be read.
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        Ok(u32::try_from(version).unwrap_or(u32::MAX))
    }

    /// Brings the file up to [`SCHEMA_VERSION`].
    ///
    /// When the file already has data, a copy is written next to it
    /// (`<name>.pre-migration.bak`) before any change, because a failed
    /// migration on a real vault is not something the user can retry their way
    /// out of.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::SchemaTooNew`] for a file from a newer build,
    /// and propagates the migration or foreign-key failure otherwise.
    pub fn migrate(&mut self) -> Result<u32, StorageError> {
        let current = self.schema_version()?;
        if current == SCHEMA_VERSION {
            return Ok(current);
        }
        if current > SCHEMA_VERSION {
            return Err(StorageError::SchemaTooNew {
                found: current,
                supported: SCHEMA_VERSION,
            });
        }
        if current > 0 {
            self.write_pre_migration_copy()?;
        }
        migrations::apply(&mut self.conn, current)?;
        self.foreign_key_check()?;
        self.schema_version()
    }

    /// Runs SQLite's `quick_check`, which reports structural damage without
    /// scanning every index.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Integrity`] when SQLite reports a problem.
    pub fn integrity_check(&self) -> Result<(), StorageError> {
        let result: String = self
            .conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(StorageError::Integrity(result))
        }
    }

    /// Verifies that every foreign key still points at an existing row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Integrity`] listing the offending rows.
    pub fn foreign_key_check(&self) -> Result<(), StorageError> {
        let mut statement = self.conn.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        match rows.next()? {
            Some(row) => {
                let table: String = row.get(0)?;
                Err(StorageError::Integrity(format!(
                    "foreign key violation in table {table}"
                )))
            }
            None => Ok(()),
        }
    }

    /// Borrows the connection for reads.
    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Borrows the connection mutably, for write transactions.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    fn write_pre_migration_copy(&self) -> Result<(), StorageError> {
        let mut backup = self.path.clone().into_os_string();
        backup.push(".pre-migration.bak");
        std::fs::copy(&self.path, PathBuf::from(backup))?;
        Ok(())
    }
}
