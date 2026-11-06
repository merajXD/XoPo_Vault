//! Forward-only migrations.
//!
//! Each migration is a pure SQL script plus a version guard. The whole upgrade
//! runs inside one transaction: a failure leaves the file at its previous
//! version instead of in a half-migrated state.
//!
//! Downgrades are not supported. Opening a file written by a newer build fails
//! with [`crate::StorageError::SchemaTooNew`] instead of guessing what the newer
//! schema means.

use rusqlite::Connection;

use crate::error::StorageError;

/// Schema version this build writes and understands.
pub const SCHEMA_VERSION: u32 = 2;

const M0001_INITIAL: &str = include_str!("../migrations/m0001_initial.sql");
const M0002_SEALED_SECRETS: &str = include_str!("../migrations/m0002_sealed_secrets.sql");

/// Applies every migration between `current` and [`SCHEMA_VERSION`].
pub(crate) fn apply(conn: &mut Connection, current: u32) -> Result<(), StorageError> {
    let transaction = conn.transaction()?;

    if current < 1 {
        transaction.execute_batch(M0001_INITIAL)?;
    }
    if current < 2 {
        transaction.execute_batch(M0002_SEALED_SECRETS)?;
    }

    // Written inside the transaction so the version can never disagree with the
    // schema it describes.
    transaction.pragma_update(None, "user_version", i64::from(SCHEMA_VERSION))?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_version_has_a_script() {
        // Guard against bumping SCHEMA_VERSION without adding the migration.
        assert_eq!(SCHEMA_VERSION, 2);
        assert!(!M0001_INITIAL.trim().is_empty());
        assert!(!M0002_SEALED_SECRETS.trim().is_empty());
    }
}
