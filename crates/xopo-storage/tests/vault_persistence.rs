//! Whole-vault persistence tests.
//!
//! These cover the Phase 2 exit criterion: create a vault with wallets and
//! accounts, close the file, reopen it and read back the same aggregate.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xopo_core::Vault;
use xopo_domain::{Address, DerivationPath, Label, NetworkId, Timestamp, WalletKind};
use xopo_storage::{Database, DatabaseKey, StorageError, VaultRepository};

const KEY: [u8; 32] = [0x33; 32];

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
            "xopo-vault-{name}-{}-{nanos}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn open(&self) -> Database {
        let mut db = Database::open(self.path(), &DatabaseKey::from_bytes(KEY)).expect("open");
        db.migrate().expect("migrate");
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

fn bitcoin() -> Address {
    Address::parse(
        NetworkId::Bitcoin,
        "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
    )
    .expect("btc address")
}

fn ethereum() -> Address {
    Address::parse(
        NetworkId::Ethereum,
        "0x52908400098527886E0F7030069857D2E4169EE7",
    )
    .expect("eth address")
}

fn sample_vault() -> Vault {
    let mut vault = Vault::new(label("Personal"), timestamp());
    let main = vault.create_wallet(WalletKind::Hierarchical, label("Main"), timestamp());
    vault
        .add_account(
            main,
            NetworkId::Bitcoin,
            label("Savings"),
            bitcoin(),
            Some(DerivationPath::parse("m/84'/0'/0'/0/0").expect("path")),
            timestamp(),
        )
        .expect("btc account");
    vault
        .add_account(
            main,
            NetworkId::Ethereum,
            label("Spending"),
            ethereum(),
            None,
            timestamp(),
        )
        .expect("eth account");

    let watch = vault.create_wallet(WalletKind::WatchOnly, label("Cold"), timestamp());
    vault
        .add_account(
            watch,
            NetworkId::Bitcoin,
            label("Watched"),
            bitcoin(),
            None,
            timestamp(),
        )
        .expect("watched account");

    vault
}

#[test]
fn a_whole_vault_survives_reopening_the_file() {
    let temp = TempVault::new("roundtrip");
    let vault = sample_vault();
    {
        let mut db = temp.open();
        VaultRepository::save(&mut db, &vault).expect("save vault");
    }

    let db = Database::open(temp.path(), &DatabaseKey::from_bytes(KEY)).expect("reopen");
    let loaded = VaultRepository::load(&db).expect("load vault");

    assert_eq!(loaded.name(), vault.name());
    assert_eq!(loaded.created_at(), vault.created_at());
    assert_eq!(loaded.wallets().len(), 2);
    assert_eq!(loaded.account_count(), 3);
    assert!(loaded.has_watch_only_wallet());
    let ids: Vec<_> = loaded.wallets().iter().map(|wallet| wallet.id()).collect();
    let expected: Vec<_> = vault.wallets().iter().map(|wallet| wallet.id()).collect();
    assert_eq!(ids, expected);
    assert_eq!(
        loaded.wallets()[0].accounts()[0]
            .derivation()
            .map(ToString::to_string),
        Some("m/84'/0'/0'/0/0".to_string())
    );
}

#[test]
fn deleting_a_wallet_in_memory_deletes_it_on_disk() {
    let temp = TempVault::new("delete");
    let vault = sample_vault();
    let kept = vault.wallets()[0].clone();
    {
        let mut db = temp.open();
        VaultRepository::save(&mut db, &vault).expect("save vault");

        let reduced =
            Vault::restore(label("Personal"), timestamp(), vec![kept]).expect("reduced vault");
        VaultRepository::save(&mut db, &reduced).expect("save reduced vault");
        assert_eq!(
            xopo_storage::WalletRepository::count(db.connection()).expect("count"),
            1
        );
    }

    let db = Database::open(temp.path(), &DatabaseKey::from_bytes(KEY)).expect("reopen");
    let loaded = VaultRepository::load(&db).expect("load");
    assert_eq!(loaded.wallets().len(), 1);
    assert_eq!(loaded.account_count(), 2);
}

#[test]
fn an_empty_vault_is_still_a_vault() {
    let temp = TempVault::new("empty");
    let vault = Vault::new(label("Empty"), timestamp());
    {
        let mut db = temp.open();
        VaultRepository::save(&mut db, &vault).expect("save empty vault");
    }

    let db = Database::open(temp.path(), &DatabaseKey::from_bytes(KEY)).expect("reopen");
    let loaded = VaultRepository::load(&db).expect("load");
    assert!(loaded.wallets().is_empty());
    assert_eq!(loaded.name().as_str(), "Empty");
}

#[test]
fn a_database_without_vault_metadata_is_not_treated_as_empty() {
    let temp = TempVault::new("no-meta");
    let db = temp.open();
    assert!(matches!(
        VaultRepository::load(&db),
        Err(StorageError::InvalidRecord(_))
    ));
}
