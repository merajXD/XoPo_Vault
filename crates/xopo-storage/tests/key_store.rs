//! Integration tests for the key store.
//!
//! The point of these tests is not that SQLite can hold a blob: it is that the
//! wrapped key and the sealed secrets survive a close/reopen, still open with the
//! password and the data key respectively, and that a row edited behind the
//! security layer's back is reported instead of trusted.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xopo_domain::Timestamp;
use xopo_security::{KdfParams, KeyEnvelope, SealedSecret};
use xopo_storage::{Database, DatabaseKey, KeyStoreRepository, StorageError};

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
            "xopo-keystore-{name}-{}-{nanos}.db",
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
fn a_fresh_vault_has_no_key() {
    let vault = TempVault::new("empty");
    let database = open_migrated(&vault);
    assert!(!KeyStoreRepository::has_key(database.connection()).expect("query"));
    assert_eq!(
        KeyStoreRepository::load_key(database.connection()).expect("load"),
        None
    );
    assert_eq!(
        KeyStoreRepository::secret_count(database.connection()).expect("count"),
        0
    );
}

#[test]
fn the_wrapped_key_survives_a_close_and_reopen() {
    let vault = TempVault::new("roundtrip");
    let (envelope, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    {
        let mut database = open_migrated(&vault);
        KeyStoreRepository::save_key(database.connection_mut(), &envelope, at()).expect("save");
        assert!(KeyStoreRepository::has_key(database.connection()).expect("query"));
    }

    let database = open_migrated(&vault);
    let stored = KeyStoreRepository::load_key(database.connection())
        .expect("load")
        .expect("a key was stored");
    assert_eq!(stored.to_bytes(), envelope.to_bytes());
    assert_eq!(stored.unseal(PASSWORD).expect("unseal"), data_key);
}

#[test]
fn re_wrapping_replaces_the_envelope_and_keeps_the_secrets() {
    let vault = TempVault::new("rewrap");
    let (envelope, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    let mut database = open_migrated(&vault);
    KeyStoreRepository::save_key(database.connection_mut(), &envelope, at()).expect("save");
    KeyStoreRepository::save_secret(database.connection_mut(), CONTEXT, &sealed, at())
        .expect("save secret");

    let new_password = "the replacement passphrase";
    let rewrapped = KeyEnvelope::wrap(&data_key, new_password, params()).expect("rewrap");
    KeyStoreRepository::save_key(database.connection_mut(), &rewrapped, at()).expect("replace");

    let stored = KeyStoreRepository::load_key(database.connection())
        .expect("load")
        .expect("key");
    assert_eq!(stored.to_bytes(), rewrapped.to_bytes());
    assert!(stored.unseal(PASSWORD).is_err());

    let recovered = stored.unseal(new_password).expect("new password");
    assert_eq!(recovered, data_key);

    let stored_secret = KeyStoreRepository::load_secret(database.connection(), CONTEXT)
        .expect("load secret")
        .expect("secret");
    let opened = stored_secret.open(&recovered, CONTEXT).expect("open");
    assert_eq!(&opened[..], PHRASE.as_bytes());
}

#[test]
fn secrets_are_stored_per_context_and_can_be_deleted() {
    let vault = TempVault::new("secrets");
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let first = SealedSecret::seal(&data_key, "wallet:a", PHRASE.as_bytes()).expect("seal");
    let second = SealedSecret::seal(&data_key, "wallet:b", b"other secret").expect("seal");

    let mut database = open_migrated(&vault);
    KeyStoreRepository::save_secret(database.connection_mut(), "wallet:a", &first, at())
        .expect("save a");
    KeyStoreRepository::save_secret(database.connection_mut(), "wallet:b", &second, at())
        .expect("save b");
    assert_eq!(
        KeyStoreRepository::secret_count(database.connection()).expect("count"),
        2
    );

    assert_eq!(
        KeyStoreRepository::load_secret(database.connection(), "wallet:a").expect("load"),
        Some(first.clone())
    );
    assert!(KeyStoreRepository::delete_secret(database.connection(), "wallet:a").expect("delete"));
    assert!(!KeyStoreRepository::delete_secret(database.connection(), "wallet:a").expect("delete"));
    assert_eq!(
        KeyStoreRepository::load_secret(database.connection(), "wallet:a").expect("load"),
        None
    );
    assert_eq!(
        KeyStoreRepository::secret_count(database.connection()).expect("count"),
        1
    );
}

#[test]
fn a_secret_does_not_open_under_another_data_key() {
    let vault = TempVault::new("foreign");
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    let mut database = open_migrated(&vault);
    KeyStoreRepository::save_secret(database.connection_mut(), CONTEXT, &sealed, at())
        .expect("save");

    let (_, other_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let stored = KeyStoreRepository::load_secret(database.connection(), CONTEXT)
        .expect("load")
        .expect("secret");
    assert!(stored.open(&other_key, CONTEXT).is_err());
}

#[test]
fn a_row_edited_behind_the_security_layer_is_reported() {
    let vault = TempVault::new("tampered");
    let database = open_migrated(&vault);

    database
        .connection()
        .execute(
            "INSERT INTO key_store (id, envelope, created_at, updated_at)
             VALUES ('vault', x'00ff00', 0, 0)",
            [],
        )
        .expect("insert a bad row");

    assert!(matches!(
        KeyStoreRepository::load_key(database.connection()),
        Err(StorageError::InvalidRecord(_))
    ));

    database
        .connection()
        .execute(
            "INSERT INTO sealed_secrets (context, payload, updated_at)
             VALUES ('wallet:bad', x'0011', 0)",
            [],
        )
        .expect("insert a bad secret");

    assert!(matches!(
        KeyStoreRepository::load_secret(database.connection(), "wallet:bad"),
        Err(StorageError::InvalidRecord(_))
    ));

    assert!(matches!(
        KeyStoreRepository::load_secret(database.connection(), ""),
        Err(StorageError::InvalidRecord(_))
    ));
    assert!(vault.path().exists());
}
