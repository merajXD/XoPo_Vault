//! End-to-end behaviour of the in-memory vault aggregate.

use xopo_core::{Vault, VaultError};
use xopo_domain::{Address, DerivationPath, Label, NetworkId, Timestamp, WalletKind};

fn label(text: &str) -> Label {
    Label::parse(text).expect("valid label")
}

fn timestamp() -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000).expect("valid timestamp")
}

fn bitcoin_address() -> Address {
    Address::parse(
        NetworkId::Bitcoin,
        "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
    )
    .expect("valid address")
}

fn ethereum_address() -> Address {
    Address::parse(
        NetworkId::Ethereum,
        "0x52908400098527886E0F7030069857D2E4169EE7",
    )
    .expect("valid address")
}

#[test]
fn a_new_vault_is_empty() {
    let vault = Vault::new(label("Personal"), timestamp());
    assert!(vault.wallets().is_empty());
    assert_eq!(vault.account_count(), 0);
    assert!(!vault.has_watch_only_wallet());
}

#[test]
fn wallets_and_accounts_are_created_and_found() {
    let mut vault = Vault::new(label("Personal"), timestamp());
    let main = vault.create_wallet(WalletKind::Hierarchical, label("Main"), timestamp());
    let watch = vault.create_wallet(
        WalletKind::WatchOnly,
        label("Cold observation"),
        timestamp(),
    );

    let bitcoin = vault
        .add_account(
            main,
            NetworkId::Bitcoin,
            label("Savings"),
            bitcoin_address(),
            Some(DerivationPath::parse("m/84'/0'/0'/0/0").expect("path")),
            timestamp(),
        )
        .expect("btc account");
    let ethereum = vault
        .add_account(
            main,
            NetworkId::Ethereum,
            label("Spending"),
            ethereum_address(),
            None,
            timestamp(),
        )
        .expect("eth account");
    vault
        .add_account(
            watch,
            NetworkId::Bitcoin,
            label("Watched"),
            bitcoin_address(),
            None,
            timestamp(),
        )
        .expect("watched account");

    assert_eq!(vault.account_count(), 3);
    assert!(vault.has_watch_only_wallet());

    let (owner, account) = vault.account(bitcoin).expect("account exists");
    assert_eq!(owner.id(), main);
    assert_eq!(account.network(), NetworkId::Bitcoin);
    assert!(vault.account(ethereum).is_some());
    assert_eq!(vault.wallet(main).expect("wallet").account_count(), 2);
}

#[test]
fn adding_to_an_unknown_wallet_fails_without_side_effects() {
    let mut vault = Vault::new(label("Personal"), timestamp());
    let missing = xopo_domain::WalletId::new();
    let error = vault
        .add_account(
            missing,
            NetworkId::Bitcoin,
            label("Savings"),
            bitcoin_address(),
            None,
            timestamp(),
        )
        .expect_err("unknown wallet must fail");

    assert!(matches!(error, VaultError::WalletNotFound { wallet_id } if wallet_id == missing));
    assert_eq!(vault.account_count(), 0);
    assert!(vault.wallets().is_empty());
}

#[test]
fn duplicate_addresses_are_rejected_within_a_wallet_but_allowed_across_wallets() {
    let mut vault = Vault::new(label("Personal"), timestamp());
    let main = vault.create_wallet(WalletKind::Hierarchical, label("Main"), timestamp());
    let other = vault.create_wallet(WalletKind::Hierarchical, label("Other"), timestamp());

    vault
        .add_account(
            main,
            NetworkId::Bitcoin,
            label("First"),
            bitcoin_address(),
            None,
            timestamp(),
        )
        .expect("first account");

    let duplicate = vault
        .add_account(
            main,
            NetworkId::Bitcoin,
            label("Second"),
            bitcoin_address(),
            None,
            timestamp(),
        )
        .expect_err("duplicate address must fail");
    assert!(matches!(duplicate, VaultError::Domain(_)));

    vault
        .add_account(
            other,
            NetworkId::Bitcoin,
            label("Second"),
            bitcoin_address(),
            None,
            timestamp(),
        )
        .expect("same address in another wallet is allowed");

    assert_eq!(vault.account_count(), 2);
}

#[test]
fn account_address_network_must_match_its_account() {
    let mut vault = Vault::new(label("Personal"), timestamp());
    let main = vault.create_wallet(WalletKind::Hierarchical, label("Main"), timestamp());
    let error = vault
        .add_account(
            main,
            NetworkId::Bitcoin,
            label("Wrong"),
            ethereum_address(),
            None,
            timestamp(),
        )
        .expect_err("mismatched network must fail");

    assert!(matches!(error, VaultError::Domain(_)));
    assert_eq!(vault.account_count(), 0);
}
