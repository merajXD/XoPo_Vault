//! Published test vectors for the key layer.
//!
//! Every value below is copied from the specification or the project that
//! publishes it; none was produced by this crate. Sources:
//!
//! * BIP-84 test vectors (BIP-84 mediawiki) — account keys and the first two
//!   receiving addresses plus the first change address.
//! * BIP-39 English vector #1 (Trezor `vectors.json`) — used in unit tests.
//! * Hardhat Network reference — the development mnemonic and the addresses it
//!   derives at `m/44'/60'/0'/0/{0,1,2}`.
//! * `bip_utils` BIP-44 tests — the Litecoin account `xpub` and first address
//!   for `m/44'/2'/0'/0/0`.
//! * EIP-55 — checksum vectors, used in the module tests.
//!
//! Known gap: Bitcoin legacy P2PKH has no published derivation vector in this
//! suite yet, so it is only round-trip tested. BIP-84 covers the same encoder's
//! checksum through bech32, and Litecoin covers Base58Check.

#![cfg(all(feature = "bitcoin", feature = "ethereum", feature = "litecoin"))]

use bitcoin::Network;
use xopo_wallet::bitcoin::{self as btc, BitcoinAddressType};
use xopo_wallet::ethereum;
use xopo_wallet::litecoin;
use xopo_wallet::{MasterKey, Mnemonic};

const ABANDON_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Hardhat's default development mnemonic. Publicly known; never for real funds.
const HARDHAT_PHRASE: &str = "test test test test test test test test test test test junk";

fn master(phrase: &str, network: Network) -> MasterKey {
    let mnemonic = Mnemonic::parse(phrase).expect("vector phrase is valid");
    MasterKey::from_mnemonic(&mnemonic, "", network).expect("master key")
}

#[test]
fn bip84_vectors_match_the_specification() {
    let master = master(ABANDON_PHRASE, Network::Bitcoin);
    let account = master.account(84, 0, 0).expect("account");

    assert_eq!(
        account.path().to_string(),
        "m/84'/0'/0'",
        "BIP-84 account path"
    );
    assert_eq!(
        account.slip132_xprv().expect("zprv"),
        "zprvAdG4iTXWBoARxkkzNpNh8r6Qag3irQB8PzEMkAFeTRXxHpbF9z4QgEvBRmfvqWvGp42t42nvgGpNgYSJA9iefm1yYNZKEm7z6qUWCroSQnE"
    );
    assert_eq!(
        account.slip132_xpub().expect("zpub"),
        "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs"
    );

    let first = account.derive(0, 0).expect("first address key");
    assert_eq!(
        first.public_key_hex(),
        "0330d54fd0dd420a6e5f8d3624f5f3482cae350f79d5f0753bf5beef9c2d91af3c"
    );
    assert_eq!(
        btc::encode_address(&first, Network::Bitcoin, BitcoinAddressType::NativeSegwit),
        "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
    );

    let second = account.derive(0, 1).expect("second address key");
    assert_eq!(
        second.public_key_hex(),
        "03e775fd51f0dfb8cd865d9ff1cca2a158cf651fe997fdc9fee9c1d3b5e995ea77"
    );
    assert_eq!(
        btc::encode_address(&second, Network::Bitcoin, BitcoinAddressType::NativeSegwit),
        "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g"
    );

    let change = account.derive(1, 0).expect("change key");
    assert_eq!(
        change.public_key_hex(),
        "03025324888e429ab8e3dbaf1f7802648b9cd01e9b418485c5fa4c1b9b5700e1a6"
    );
    assert_eq!(
        btc::encode_address(&change, Network::Bitcoin, BitcoinAddressType::NativeSegwit),
        "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el"
    );
}

#[test]
fn bip84_derivation_helper_returns_the_same_first_address() {
    let master = master(ABANDON_PHRASE, Network::Bitcoin);
    let derived = btc::derive_address(
        &master,
        Network::Bitcoin,
        BitcoinAddressType::NativeSegwit,
        0,
        0,
        0,
    )
    .expect("derive");
    assert_eq!(derived, "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu");
}

#[test]
fn legacy_addresses_round_trip_and_never_match_segwit() {
    let master = master(ABANDON_PHRASE, Network::Bitcoin);
    let legacy = btc::derive_address(
        &master,
        Network::Bitcoin,
        BitcoinAddressType::Legacy,
        0,
        0,
        0,
    )
    .expect("legacy");
    assert!(
        legacy.starts_with('1'),
        "mainnet P2PKH starts with 1: {legacy}"
    );
    let parsed = btc::parse_address(Network::Bitcoin, &legacy).expect("round trip");
    assert_eq!(parsed.to_string(), legacy);

    let segwit = btc::derive_address(
        &master,
        Network::Bitcoin,
        BitcoinAddressType::NativeSegwit,
        0,
        0,
        0,
    )
    .expect("segwit");
    assert_ne!(legacy, segwit);
}

#[test]
fn ethereum_matches_the_hardhat_derivation() {
    let master = master(HARDHAT_PHRASE, Network::Bitcoin);
    let expected = [
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
        "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
    ];
    for (index, expected_address) in expected.iter().enumerate() {
        let derived = ethereum::derive_address(&master, 0, u32::try_from(index).expect("index"))
            .expect("ethereum address");
        assert_eq!(&derived, expected_address, "index {index}");
        ethereum::parse_address(&derived).expect("our own address must pass EIP-55");
    }
}

#[test]
fn litecoin_matches_the_published_bip44_vector() {
    let master = master(ABANDON_PHRASE, Network::Bitcoin);

    let account = master
        .account(44, litecoin::COIN_TYPE, 0)
        .expect("litecoin account");
    assert_eq!(
        account.xpub(),
        "xpub6BnJJjq783EdyBeQPA9P9ao9DTS3fUqyKG5NJDcrCiwwxEkesGoHN94LZRGE7rz1jgcvmmp8j55BNx573KFq1WBwKiemzkdfNKffKx6Mvku"
    );

    let address = litecoin::derive_address(&master, 0, 0, 0).expect("litecoin address");
    assert_eq!(address, "LUWPbpM43E2p7ZSh8cyTBEkvpHmr3cB8Ez");
    let parsed = litecoin::parse_address(&address).expect("round trip");
    assert_eq!(
        litecoin::parse_address("LUWPbpM43E2p7ZSh8cyTBEkvpHmr3cB8Ez").expect("vector"),
        parsed
    );
}

#[test]
fn the_passphrase_changes_the_whole_tree() {
    let mnemonic = Mnemonic::parse(ABANDON_PHRASE).expect("parse");
    let plain = MasterKey::from_mnemonic(&mnemonic, "", Network::Bitcoin).expect("plain");
    let salted = MasterKey::from_mnemonic(&mnemonic, "xopo", Network::Bitcoin).expect("salted");

    assert_eq!(
        plain.account(84, 0, 0).expect("p").path(),
        salted.account(84, 0, 0).expect("s").path()
    );
    assert_ne!(
        plain.account(84, 0, 0).expect("p").xpub(),
        salted.account(84, 0, 0).expect("s").xpub()
    );
}
