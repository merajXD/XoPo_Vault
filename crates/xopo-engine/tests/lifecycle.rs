//! End-to-end vault lifecycle: create, lock, reopen, add a wallet, reveal it,
//! change the password, and every failure path that must not corrupt the vault.
//!
//! These are the tests that prove the CLI is doing real work: they go through the
//! same engine calls the binary uses, against real SQLCipher files and real
//! Argon2id envelopes (at the policy floor, so the suite stays seconds long).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xopo_domain::NetworkId;
use xopo_engine::{create_vault_with_params, open_vault, EngineError, VaultHandle, VaultPaths};
use xopo_security::KdfParams;
use xopo_wallet::{KeySource, Mnemonic};

/// Official BIP-39 English vector #1. Public data, never a real wallet.
const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// First BIP-84 mainnet receiving address of that phrase (published vector).
const BITCOIN_ADDRESS: &str = "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu";
const PASSWORD: &str = "correct horse battery";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("xopo-engine-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        Self { path }
    }

    fn paths(&self) -> VaultPaths {
        VaultPaths::new(self.path.join("vault.xopo"))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn params() -> KdfParams {
    KdfParams::new(19 * 1024, 2, 1).expect("policy floor")
}

fn create(dir: &TempDir) -> VaultHandle {
    create_vault_with_params(dir.paths(), "Test Vault", PASSWORD, params()).expect("create")
}

fn import(source_phrase: &str) -> KeySource {
    KeySource::Imported(Mnemonic::parse(source_phrase).expect("vector phrase"))
}

#[test]
fn a_new_vault_is_empty_and_reopens() {
    let dir = TempDir::new("empty");
    {
        let handle = create(&dir);
        assert_eq!(handle.vault().wallets().len(), 0);
        assert_eq!(handle.vault().account_count(), 0);
        assert_eq!(handle.schema_version().expect("schema"), 2);
        assert!(handle.paths().database().exists());
        assert!(handle.paths().key_file().exists());
    }

    let handle = open_vault(dir.paths(), PASSWORD).expect("reopen");
    assert_eq!(handle.vault().name().as_str(), "Test Vault");
    assert_eq!(handle.vault().wallets().len(), 0);
}

#[test]
fn a_wrong_password_cannot_open_the_vault() {
    let dir = TempDir::new("wrong");
    create(&dir).lock();

    match open_vault(dir.paths(), "not the password") {
        Err(EngineError::WrongPassword) => {}
        other => panic!("expected WrongPassword, got {other:?}"),
    }
}

#[test]
fn a_wallet_and_its_phrase_survive_a_restart() {
    let dir = TempDir::new("restart");
    let wallet_id = {
        let mut handle = create(&dir);
        let added = handle
            .add_wallet(
                import(PHRASE),
                "Demo Wallet",
                &[NetworkId::Bitcoin, NetworkId::Ethereum],
            )
            .expect("add wallet");
        assert!(added.sealed());
        assert_eq!(added.accounts().len(), 2);
        added.wallet_id()
    };

    let handle = open_vault(dir.paths(), PASSWORD).expect("reopen");
    assert_eq!(handle.vault().wallets().len(), 1);
    let wallet = handle
        .vault()
        .wallets()
        .iter()
        .find(|wallet| wallet.id() == wallet_id)
        .expect("wallet is still there");
    assert_eq!(wallet.label().as_str(), "Demo Wallet");
    assert_eq!(wallet.account_count(), 2);

    let bitcoin = wallet
        .accounts_on(NetworkId::Bitcoin)
        .next()
        .expect("bitcoin account");
    assert_eq!(bitcoin.address().as_str(), BITCOIN_ADDRESS);

    let eth = wallet
        .accounts_on(NetworkId::Ethereum)
        .next()
        .expect("ethereum account");
    assert!(eth.address().as_str().starts_with("0x"));

    let revealed = handle.reveal_phrase(wallet_id).expect("reveal");
    assert_eq!(revealed.as_str(), PHRASE);
}

#[test]
fn changing_the_password_keeps_the_sealed_phrase() {
    let dir = TempDir::new("passwd");
    let wallet_id = {
        let mut handle = create(&dir);
        let added = handle
            .add_wallet(import(PHRASE), "Demo", &[NetworkId::Bitcoin])
            .expect("add wallet");
        handle
            .change_password(PASSWORD, "a completely new password")
            .expect("change password");
        added.wallet_id()
    };

    assert!(matches!(
        open_vault(dir.paths(), PASSWORD),
        Err(EngineError::WrongPassword)
    ));

    let handle = open_vault(dir.paths(), "a completely new password").expect("new password works");
    assert_eq!(
        handle.reveal_phrase(wallet_id).expect("reveal").as_str(),
        PHRASE
    );
}

#[test]
fn a_damaged_or_missing_key_file_is_reported() {
    let dir = TempDir::new("damage");
    create(&dir).lock();

    let key_file = dir.paths().key_file().to_path_buf();
    let mut bytes = fs::read(&key_file).expect("read key file");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(&key_file, &bytes).expect("write key file");

    assert!(matches!(
        open_vault(dir.paths(), PASSWORD),
        Err(EngineError::WrongPassword)
    ));

    fs::remove_file(&key_file).expect("remove key file");
    assert!(matches!(
        open_vault(dir.paths(), PASSWORD),
        Err(EngineError::MissingKeyFile(_))
    ));
}

#[test]
fn creating_over_an_existing_vault_is_refused() {
    let dir = TempDir::new("exists");
    create(&dir).lock();
    assert!(matches!(
        create_vault_with_params(dir.paths(), "Again", PASSWORD, params()),
        Err(EngineError::AlreadyExists(_))
    ));
}

#[test]
fn a_watch_only_wallet_never_has_a_phrase() {
    let dir = TempDir::new("watch");
    let mut handle = create(&dir);
    let address =
        xopo_domain::Address::parse(NetworkId::Bitcoin, BITCOIN_ADDRESS).expect("address");
    let source = KeySource::WatchOnly(xopo_wallet::WatchOnlyAccount::new(address));
    let added = handle
        .add_wallet(source, "Watched", &[])
        .expect("add watch-only");
    assert!(!added.sealed());
    assert!(matches!(
        handle.reveal_phrase(added.wallet_id()),
        Err(EngineError::NoRecoveryPhrase(_))
    ));
}

#[test]
fn a_failed_add_leaves_the_vault_unchanged() {
    let dir = TempDir::new("rollback");
    let mut handle = create(&dir);

    // TON has no derivation path yet, so provisioning fails after the wallet has
    // been created in memory.
    let result = handle.add_wallet(import(PHRASE), "Unsupported", &[NetworkId::Ton]);
    assert!(matches!(
        result,
        Err(EngineError::Core(
            xopo_core::VaultError::UnsupportedNetwork { .. }
        ))
    ));
    assert_eq!(handle.vault().wallets().len(), 0);
    handle.lock();

    let reopened = open_vault(dir.paths(), PASSWORD).expect("reopen");
    assert_eq!(reopened.vault().wallets().len(), 0);
    assert!(dir.path().exists());
}

#[test]
fn a_short_password_is_refused_before_any_file_is_written() {
    let dir = TempDir::new("weak");
    assert!(matches!(
        create_vault_with_params(dir.paths(), "Weak", "short", params()),
        Err(EngineError::WeakPassword { minimum: 8 })
    ));
    assert!(!dir.paths().database().exists());
    assert!(!dir.paths().key_file().exists());
}
