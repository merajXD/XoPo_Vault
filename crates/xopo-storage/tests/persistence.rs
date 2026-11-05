//! Integration tests for the encrypted vault database.
//!
//! Every test uses its own file so failures cannot depend on execution order.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xopo_domain::{
    Address, DerivationPath, Label, NetworkId, Timestamp, Wallet, WalletId, WalletKind,
};
use xopo_storage::{Database, DatabaseKey, StorageError, VaultMeta, WalletRepository};

const KEY: [u8; 32] = [0x5a; 32];

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
            "xopo-storage-{name}-{}-{nanos}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn key() -> DatabaseKey {
        DatabaseKey::from_bytes(KEY)
    }

    fn open(&self) -> Database {
        let mut db = Database::open(self.path(), &Self::key()).expect("open vault");
        db.migrate().expect("migrate vault");
        db
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        for suffix in ["-wal", "-shm", ".pre-migration.bak"] {
            let mut sidecar = self.path.clone().into_os_string();
            sidecar.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

fn label(text: &str) -> Label {
    Label::parse(text).expect("valid label")
}

fn timestamp() -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000).expect("valid timestamp")
}

fn sample_wallet() -> Wallet {
    let mut wallet = Wallet::new(WalletKind::Hierarchical, label("Main"), timestamp());
    wallet
        .add_account(
            NetworkId::Bitcoin,
            label("Savings"),
            Address::parse(
                NetworkId::Bitcoin,
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            )
            .expect("btc address"),
            Some(DerivationPath::parse("m/84'/0'/0'/0/0").expect("path")),
            timestamp(),
        )
        .expect("btc account");
    wallet
        .add_account(
            NetworkId::Ton,
            label("TON"),
            Address::parse(
                NetworkId::Ton,
                "EQDtFpEwcFAEcRe5mLVh2N6C0x-_hJEM7W61_JLnSF74p4q2",
            )
            .expect("ton address"),
            None,
            timestamp(),
        )
        .expect("ton account");
    wallet
}

#[test]
fn wallet_and_accounts_survive_reopening_the_file() {
    let vault = TempVault::new("roundtrip");
    let wallet = sample_wallet();
    {
        let mut db = vault.open();
        WalletRepository::save(db.connection_mut(), &wallet).expect("save wallet");
        assert_eq!(WalletRepository::count(db.connection()).expect("count"), 1);
    }

    let db = Database::open(vault.path(), &TempVault::key()).expect("reopen");
    let loaded = WalletRepository::load(db.connection(), wallet.id()).expect("load wallet");
    assert_eq!(loaded, wallet);
    assert_eq!(loaded.account_count(), 2);
    assert_eq!(
        loaded.accounts()[0].derivation().map(ToString::to_string),
        Some("m/84'/0'/0'/0/0".to_string())
    );
}

#[test]
fn saving_again_updates_the_wallet_and_drops_removed_accounts() {
    let vault = TempVault::new("update");
    let wallet = sample_wallet();
    let first_account = wallet.accounts()[0].clone();
    {
        let mut db = vault.open();
        WalletRepository::save(db.connection_mut(), &wallet).expect("save wallet");

        let reduced = Wallet::restore(
            wallet.id(),
            label("Renamed"),
            wallet.kind(),
            wallet.created_at(),
            vec![first_account],
        )
        .expect("reduced wallet");
        WalletRepository::save(db.connection_mut(), &reduced).expect("save reduced wallet");
    }

    let db = Database::open(vault.path(), &TempVault::key()).expect("reopen");
    let loaded = WalletRepository::load(db.connection(), wallet.id()).expect("load");
    assert_eq!(loaded.label().as_str(), "Renamed");
    assert_eq!(loaded.account_count(), 1);
    assert_eq!(loaded.accounts()[0].network(), NetworkId::Bitcoin);
}

#[test]
fn missing_wallet_is_reported_not_invented() {
    let vault = TempVault::new("missing");
    let db = vault.open();
    assert!(matches!(
        WalletRepository::load(db.connection(), WalletId::new()),
        Err(StorageError::WalletNotFound(_))
    ));
}

#[test]
fn a_wrong_key_cannot_open_the_vault() {
    let vault = TempVault::new("wrong-key");
    {
        let mut db = vault.open();
        WalletRepository::save(db.connection_mut(), &sample_wallet()).expect("save");
    }

    let other = DatabaseKey::from_bytes([0x11; 32]);
    assert!(matches!(
        Database::open(vault.path(), &other),
        Err(StorageError::WrongKeyOrCorrupt)
    ));
}

#[test]
fn nothing_readable_is_left_on_disk() {
    let vault = TempVault::new("at-rest");
    let wallet = sample_wallet();
    {
        let mut db = vault.open();
        WalletRepository::save(db.connection_mut(), &wallet).expect("save");
    }

    let bytes = std::fs::read(vault.path()).expect("read vault file");
    assert_ne!(&bytes[..16], b"SQLite format 3\0");
    assert!(!contains(&bytes, b"Main"));
    assert!(!contains(
        &bytes,
        b"bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
    ));
    assert!(!contains(&bytes, b"hierarchical"));
}

#[test]
fn foreign_keys_are_enforced() {
    let vault = TempVault::new("foreign-key");
    let db = vault.open();
    let result = db.connection().execute(
        "INSERT INTO accounts
             (id, wallet_id, network_id, label, address, derivation, created_at)
         VALUES ('account-without-wallet', 'missing-wallet', 'bitcoin', 'Orphan',
                 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq', NULL, 1)",
        [],
    );
    assert!(result.is_err(), "orphan account must be rejected");
}

#[test]
fn an_unknown_network_in_storage_is_reported() {
    let vault = TempVault::new("bad-network");
    let db = vault.open();
    let wallet_id = WalletId::new();
    db.connection()
        .execute(
            "INSERT INTO wallets (id, label, kind, key_ref, created_at)
             VALUES (?1, 'Foreign', 'hierarchical', NULL, 1)",
            [wallet_id.to_string()],
        )
        .expect("insert wallet row");
    db.connection()
        .execute(
            "INSERT INTO accounts
                 (id, wallet_id, network_id, label, address, derivation, created_at)
             VALUES ('bad-network-account', ?1, 'cardano', 'Foreign',
                     'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq', NULL, 1)",
            [wallet_id.to_string()],
        )
        .expect("insert account row");

    assert!(matches!(
        WalletRepository::load(db.connection(), wallet_id),
        Err(StorageError::InvalidRecord(_))
    ));
}

#[test]
fn migrations_are_idempotent_and_record_their_version() {
    let vault = TempVault::new("migrate");
    let mut db = vault.open();
    assert_eq!(db.schema_version().expect("version"), 1);
    assert_eq!(db.migrate().expect("second migrate"), 1);
    assert_eq!(db.schema_version().expect("version"), 1);
    db.integrity_check().expect("integrity");
    db.foreign_key_check().expect("foreign keys");
}

#[test]
fn a_newer_schema_is_refused() {
    let vault = TempVault::new("newer");
    {
        let db = vault.open();
        db.connection()
            .pragma_update(None, "user_version", 99_i64)
            .expect("set version");
    }

    let mut db = Database::open(vault.path(), &TempVault::key()).expect("open");
    assert!(matches!(
        db.migrate(),
        Err(StorageError::SchemaTooNew {
            found: 99,
            supported: 1
        })
    ));
}

#[test]
fn a_damaged_file_is_rejected_instead_of_recreated() {
    let vault = TempVault::new("damaged");
    {
        let mut db = vault.open();
        WalletRepository::save(db.connection_mut(), &sample_wallet()).expect("save");
    }

    let mut bytes = std::fs::read(vault.path()).expect("read");
    assert!(bytes.len() > 400, "page size sanity");
    bytes[200] ^= 0xff;
    std::fs::write(vault.path(), &bytes).expect("write damaged file");

    assert!(Database::open(vault.path(), &TempVault::key()).is_err());
}

#[test]
fn metadata_round_trips_and_can_be_removed() {
    let vault = TempVault::new("meta");
    let db = vault.open();
    assert_eq!(VaultMeta::get(db.connection(), "name").expect("get"), None);
    VaultMeta::set(db.connection(), "name", "Personal").expect("set");
    VaultMeta::set(db.connection(), "name", "Personal 2").expect("overwrite");
    assert_eq!(
        VaultMeta::get(db.connection(), "name").expect("get"),
        Some("Personal 2".to_string())
    );
    assert!(VaultMeta::remove(db.connection(), "name").expect("remove"));
    assert_eq!(VaultMeta::get(db.connection(), "name").expect("get"), None);
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
