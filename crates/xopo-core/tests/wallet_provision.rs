//! Creating wallets whose addresses come from real keys.
//!
//! The expected addresses are published values, not this crate's own output:
//! BIP-84 vectors for Bitcoin, Hardhat's development mnemonic for Ethereum and
//! the `bip_utils` BIP-44 vector for Litecoin. Reusing them here proves that the
//! join between the key layer and the vault does not alter what was derived.

use xopo_core::provision::{create_wallet, AccountRequest};
use xopo_core::{Vault, VaultError};
use xopo_domain::{Address, DomainError, Label, NetworkId, Timestamp, WalletKind};
use xopo_wallet::bitcoin::BitcoinAddressType;
use xopo_wallet::{KeySource, Mnemonic, WatchOnlyAccount};

const ABANDON_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const HARDHAT_PHRASE: &str = "test test test test test test test test test test test junk";
const BIP84_FIRST_ADDRESS: &str = "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu";
const HARDHAT_FIRST_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const LITECOIN_FIRST_ADDRESS: &str = "LUWPbpM43E2p7ZSh8cyTBEkvpHmr3cB8Ez";

fn label(text: &str) -> Label {
    Label::parse(text).expect("valid label")
}

fn timestamp() -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000).expect("valid timestamp")
}

fn source(phrase: &str) -> KeySource {
    KeySource::Created(Mnemonic::parse(phrase).expect("valid phrase"))
}

fn vault() -> Vault {
    Vault::new(label("Personal"), timestamp())
}

#[test]
fn a_bitcoin_account_gets_the_bip84_address_and_account_path() {
    let mut vault = vault();
    let created = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("Main"),
        "",
        &[AccountRequest::new(NetworkId::Bitcoin, label("Bitcoin"))],
        timestamp(),
    )
    .expect("create wallet");

    let wallet = vault.wallet(created.wallet_id()).expect("wallet exists");
    assert_eq!(wallet.kind(), WalletKind::Hierarchical);
    assert_eq!(wallet.account_count(), 1);

    let account = wallet.accounts().first().expect("one account");
    assert_eq!(account.network(), NetworkId::Bitcoin);
    assert_eq!(account.address().as_str(), BIP84_FIRST_ADDRESS);
    assert_eq!(
        account
            .derivation()
            .expect("account path is recorded")
            .to_string(),
        "m/84'/0'/0'"
    );
    assert_eq!(
        wallet.account(created.accounts()[0]).map(|a| a.id()),
        Some(created.accounts()[0])
    );
}

#[test]
fn legacy_bitcoin_uses_purpose_44_and_a_different_address() {
    let mut vault = vault();
    let created = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("Legacy"),
        "",
        &[AccountRequest::new(NetworkId::Bitcoin, label("Bitcoin"))
            .with_bitcoin_type(BitcoinAddressType::Legacy)],
        timestamp(),
    )
    .expect("create wallet");

    let wallet = vault.wallet(created.wallet_id()).expect("wallet exists");
    let account = wallet.accounts().first().expect("one account");
    assert_eq!(
        account
            .derivation()
            .expect("account path is recorded")
            .to_string(),
        "m/44'/0'/0'"
    );
    assert_ne!(account.address().as_str(), BIP84_FIRST_ADDRESS);
    assert!(account.address().as_str().starts_with('1'));
    Address::parse(NetworkId::Bitcoin, account.address().as_str()).expect("domain accepts it");
}

#[test]
fn one_wallet_can_hold_several_networks_from_one_phrase() {
    let mut vault = vault();
    let created = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("Multi"),
        "",
        &[
            AccountRequest::new(NetworkId::Bitcoin, label("Bitcoin")),
            AccountRequest::new(NetworkId::Ethereum, label("Ethereum")),
            AccountRequest::new(NetworkId::Litecoin, label("Litecoin")),
        ],
        timestamp(),
    )
    .expect("create wallet");

    let wallet = vault.wallet(created.wallet_id()).expect("wallet exists");
    assert_eq!(wallet.account_count(), 3);
    assert_eq!(
        wallet.networks(),
        vec![NetworkId::Bitcoin, NetworkId::Ethereum, NetworkId::Litecoin]
    );

    let bitcoin = wallet
        .accounts_on(NetworkId::Bitcoin)
        .next()
        .expect("bitcoin account");
    assert_eq!(bitcoin.address().as_str(), BIP84_FIRST_ADDRESS);

    let ethereum = wallet
        .accounts_on(NetworkId::Ethereum)
        .next()
        .expect("ethereum account");
    xopo_wallet::ethereum::parse_address(ethereum.address().as_str())
        .expect("the stored address carries a valid EIP-55 checksum");
    assert_eq!(
        ethereum.derivation().expect("path").to_string(),
        "m/44'/60'/0'"
    );

    let litecoin = wallet
        .accounts_on(NetworkId::Litecoin)
        .next()
        .expect("litecoin account");
    assert_eq!(litecoin.address().as_str(), LITECOIN_FIRST_ADDRESS);
}

#[test]
fn an_ethereum_account_matches_the_hardhat_derivation() {
    let mut vault = vault();
    let created = create_wallet(
        &mut vault,
        source(HARDHAT_PHRASE),
        label("Developer"),
        "",
        &[AccountRequest::new(NetworkId::Ethereum, label("Ethereum"))],
        timestamp(),
    )
    .expect("create wallet");

    let wallet = vault.wallet(created.wallet_id()).expect("wallet exists");
    let account = wallet.accounts().first().expect("one account");
    assert_eq!(account.address().as_str(), HARDHAT_FIRST_ADDRESS);
}

#[test]
fn an_unsupported_network_leaves_the_vault_untouched() {
    let mut vault = vault();
    let error = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("TON"),
        "",
        &[
            AccountRequest::new(NetworkId::Bitcoin, label("Bitcoin")),
            AccountRequest::new(NetworkId::Ton, label("TON")),
        ],
        timestamp(),
    )
    .expect_err("TON has no derivation yet");

    assert!(matches!(
        error,
        VaultError::UnsupportedNetwork {
            network: NetworkId::Ton
        }
    ));
    assert!(vault.wallets().is_empty(), "no partial wallet may remain");
    assert_eq!(vault.account_count(), 0);
}

#[test]
fn duplicate_requests_are_rejected_before_anything_is_written() {
    let mut vault = vault();
    let error = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("Twice"),
        "",
        &[
            AccountRequest::new(NetworkId::Bitcoin, label("First")),
            AccountRequest::new(NetworkId::Bitcoin, label("Second")),
        ],
        timestamp(),
    )
    .expect_err("same address twice");

    assert!(matches!(
        error,
        VaultError::Domain(DomainError::DuplicateAccountAddress)
    ));
    assert!(vault.wallets().is_empty());
}

#[test]
fn a_signing_wallet_needs_at_least_one_account() {
    let mut vault = vault();
    let error = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("Empty"),
        "",
        &[],
        timestamp(),
    )
    .expect_err("no accounts");
    assert!(matches!(error, VaultError::NoAccounts));
    assert!(vault.wallets().is_empty());
}

#[test]
fn watch_only_stores_the_address_and_no_key() {
    let mut vault = vault();
    let watched = Address::parse(NetworkId::Bitcoin, BIP84_FIRST_ADDRESS).expect("address");
    let created = create_wallet(
        &mut vault,
        KeySource::WatchOnly(WatchOnlyAccount::new(watched.clone())),
        label("Observe"),
        "",
        &[],
        timestamp(),
    )
    .expect("watch-only wallet");

    let wallet = vault.wallet(created.wallet_id()).expect("wallet exists");
    assert_eq!(wallet.kind(), WalletKind::WatchOnly);
    assert!(!wallet.kind().can_sign());
    let account = wallet.accounts().first().expect("one account");
    assert_eq!(account.address(), &watched);
    assert_eq!(account.derivation(), None);
    assert!(created.source().is_watch_only());
    assert!(created.source().mnemonic().is_none());
    assert!(vault.has_watch_only_wallet());
}

#[test]
fn a_watch_only_source_rejects_account_requests() {
    let mut vault = vault();
    let watched = Address::parse(NetworkId::Bitcoin, BIP84_FIRST_ADDRESS).expect("address");
    let error = create_wallet(
        &mut vault,
        KeySource::WatchOnly(WatchOnlyAccount::new(watched)),
        label("Observe"),
        "",
        &[AccountRequest::new(NetworkId::Bitcoin, label("Bitcoin"))],
        timestamp(),
    )
    .expect_err("watch-only takes no requests");
    assert!(matches!(error, VaultError::WatchOnlyWithAccounts));
    assert!(vault.wallets().is_empty());
}

#[test]
fn the_passphrase_is_not_ignored() {
    let mut vault = vault();
    let created = create_wallet(
        &mut vault,
        source(ABANDON_PHRASE),
        label("Salted"),
        "another passphrase",
        &[AccountRequest::new(NetworkId::Bitcoin, label("Bitcoin"))],
        timestamp(),
    )
    .expect("create wallet");

    let wallet = vault.wallet(created.wallet_id()).expect("wallet exists");
    let account = wallet.accounts().first().expect("one account");
    assert_ne!(account.address().as_str(), BIP84_FIRST_ADDRESS);
}
