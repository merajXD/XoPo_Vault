//! Integration tests for sealed secrets at rest.
//!
//! The point of these tests is not that SQLite can hold a blob: it is that a
//! sealed payload survives a close/reopen, still opens with the data key, is
//! bound to its wallet, and that a row edited behind the security layer's back is
//! reported instead of trusted.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xopo_domain::Timestamp;
use xopo_security::{KdfParams, KeyEnvelope, SealedSecret};
use xopo_storage::{Database, DatabaseKey, SecretRepository, StorageError};

const KEY: [u8; 32] = [0x3c; 32];
const PASSWORD: &str = "a long enough passphrase";
const CONTEXT: &str = "wallet:0192f0c4-0000-7000-8000-000000000001";
const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

struct TempVault {
    path: PathBuf,
}

impl TempVault {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "xopo-secrets-{name}-{}-{nanos}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn open(&self) -> Database {
        Database::open(&self.path, &DatabaseKey::from_bytes(KEY)).expect("open vault")
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

fn at() -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000).expect("timestamp")
}

fn params() -> KdfParams {
    KdfParams::new(19 * 1024, 2, 1).expect("policy floor")
}

fn open_migrated(vault: &TempVault) -> Database {
    let mut database = vault.open();
    database.migrate().expect("migrate");
    database
}

#[test]
fn a_fresh_vault_holds_no_secrets() {
    let vault = TempVault::new("empty");
    let database = open_migrated(&vault);
    assert_eq!(
        SecretRepository::count(database.connection()).expect("count"),
        0
    );
    assert_eq!(
        SecretRepository::load(database.connection(), CONTEXT).expect("load"),
        None
    );
}

#[test]
fn a_secret_survives_a_close_and_reopen() {
    let vault = TempVault::new("roundtrip");
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");
    {
        let mut database = open_migrated(&vault);
        SecretRepository::save(database.connection_mut(), CONTEXT, &sealed, at()).expect("save");
    }

    let database = open_migrated(&vault);
    let stored = SecretRepository::load(database.connection(), CONTEXT)
        .expect("load")
        .expect("a secret was stored");
    let opened = stored.open(&data_key, CONTEXT).expect("open");
    assert_eq!(&opened[..], PHRASE.as_bytes());
    assert!(vault.path().exists());
}

#[test]
fn secrets_are_stored_per_context_and_can_be_deleted() {
    let vault = TempVault::new("contexts");
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let first = SealedSecret::seal(&data_key, "wallet:a", PHRASE.as_bytes()).expect("seal");
    let second = SealedSecret::seal(&data_key, "wallet:b", b"other secret").expect("seal");

    let mut database = open_migrated(&vault);
    SecretRepository::save(database.connection_mut(), "wallet:a", &first, at()).expect("save a");
    SecretRepository::save(database.connection_mut(), "wallet:b", &second, at()).expect("save b");
    assert_eq!(
        SecretRepository::count(database.connection()).expect("count"),
        2
    );

    assert_eq!(
        SecretRepository::load(database.connection(), "wallet:a").expect("load"),
        Some(first.clone())
    );
    assert!(SecretRepository::delete(database.connection(), "wallet:a").expect("delete"));
    assert!(!SecretRepository::delete(database.connection(), "wallet:a").expect("delete"));
    assert_eq!(
        SecretRepository::load(database.connection(), "wallet:a").expect("load"),
        None
    );
    assert_eq!(
        SecretRepository::count(database.connection()).expect("count"),
        1
    );
}

#[test]
fn a_secret_does_not_open_under_another_data_key_or_context() {
    let vault = TempVault::new("foreign");
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    let mut database = open_migrated(&vault);
    SecretRepository::save(database.connection_mut(), CONTEXT, &sealed, at()).expect("save");

    let (_, other_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let stored = SecretRepository::load(database.connection(), CONTEXT)
        .expect("load")
        .expect("secret");
    assert!(stored.open(&other_key, CONTEXT).is_err());
    assert!(stored.open(&data_key, "wallet:other").is_err());
}

#[test]
fn a_row_edited_behind_the_security_layer_is_reported() {
    let vault = TempVault::new("tampered");
    let database = open_migrated(&vault);

    database
        .connection()
        .execute(
            "INSERT INTO sealed_secrets (context, payload, updated_at)
             VALUES ('wallet:bad', x'0011', 0)",
            [],
        )
        .expect("insert a bad secret");

    assert!(matches!(
        SecretRepository::load(database.connection(), "wallet:bad"),
        Err(StorageError::InvalidRecord(_))
    ));
    assert!(matches!(
        SecretRepository::load(database.connection(), ""),
        Err(StorageError::InvalidRecord(_))
    ));
}
