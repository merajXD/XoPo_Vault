//! Errors raised by the wallet layer.
//!
//! No variant carries key material, a mnemonic word or a private key. Messages
//! are safe to log.

use xopo_domain::{DomainError, NetworkId};

/// Everything that can fail while turning a phrase into an address.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WalletError {
    /// The operating system refused to provide entropy.
    #[error("could not read entropy from the operating system: {detail}")]
    EntropySource {
        /// Platform message; carries no secret.
        detail: String,
    },

    /// The phrase has a word count BIP-39 does not define.
    #[error("a BIP-39 phrase needs 12, 15, 18, 21 or 24 words, found {words}")]
    UnsupportedWordCount {
        /// Word count found in the input.
        words: usize,
    },

    /// The phrase was empty or only whitespace.
    #[error("the mnemonic phrase is empty")]
    EmptyMnemonic,

    /// A word is not in the list, or the checksum does not match.
    #[error("not a valid BIP-39 English mnemonic: unknown word or checksum mismatch")]
    InvalidMnemonic,

    /// The seed did not produce a BIP-32 master key.
    #[error("the seed could not produce a BIP-32 master key")]
    MasterKey,

    /// BIP-32 child derivation failed (for example a would-be invalid key).
    #[error("BIP-32 derivation failed at the requested path")]
    DerivationFailed,

    /// A path segment could not become a BIP-32 child number.
    #[error("derivation index {index} is out of range")]
    InvalidChildIndex {
        /// Segment index that failed.
        index: u32,
    },

    /// An extended key was not the expected 78 bytes.
    #[error("{kind} is not a valid 78-byte extended key")]
    MalformedExtendedKey {
        /// Which kind of key was being read.
        kind: &'static str,
    },

    /// SLIP-132 re-tagging was attempted on a testnet or otherwise foreign key.
    #[error("extended key version {version} is not the expected mainnet version")]
    UnexpectedExtendedKeyVersion {
        /// Version bytes as hex; public information.
        version: String,
    },

    /// The purpose has no SLIP-132 prefix defined.
    #[error("derivation purpose {purpose} has no registered SLIP-132 prefix")]
    UnknownPurpose {
        /// Purpose that was requested.
        purpose: u32,
    },

    /// The adapter does not handle this network.
    #[error("{network} is not supported by this adapter")]
    UnsupportedNetwork {
        /// Network that was requested.
        network: NetworkId,
    },

    /// The address is structurally wrong for the network or its checksum fails.
    #[error("the address is not a valid {network} address")]
    InvalidAddress {
        /// Network the address was checked against.
        network: NetworkId,
    },

    /// A secp256k1 point was invalid.
    #[error("the public key is not a valid secp256k1 point")]
    InvalidPublicKey,

    /// A domain rule rejected the value.
    #[error(transparent)]
    Domain(#[from] DomainError),
}
