//! Domain error type.
//!
//! Every constructor in this crate that receives untrusted input returns a
//! [`Result`] with a [`DomainError`]. Errors are explicit and carry the data
//! needed to explain the failure; nothing is silently truncated, trimmed or
//! coerced into a valid value.

use thiserror::Error;

use crate::ids::{AccountId, WalletId};
use crate::network::NetworkId;

/// Errors produced while building or validating domain values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    /// A string could not be parsed into a typed identifier.
    #[error("invalid {kind} identifier")]
    InvalidId {
        /// The kind of identifier that was being parsed ("wallet", "account").
        kind: &'static str,
    },

    // ----- Labels and other short human-readable text -----
    /// Text value was empty.
    #[error("value must not be empty")]
    EmptyLabel,

    /// Text value exceeded the maximum character count.
    #[error("value must be at most {max} characters, got {actual}")]
    LabelTooLong {
        /// Maximum allowed characters (Unicode scalar values).
        max: usize,
        /// Actual character count.
        actual: usize,
    },

    /// Text value contained a control character.
    #[error("value must not contain control characters")]
    LabelControlCharacter,

    /// Text value had leading or trailing whitespace.
    #[error("value must not have leading or trailing whitespace")]
    LabelSurroundingWhitespace,

    // ----- Asset symbols -----
    /// Asset symbol contained characters outside `A-Z` and `0-9`.
    #[error("asset symbol must contain only ASCII uppercase letters and digits")]
    InvalidSymbol,

    /// Asset symbol was outside the allowed length range.
    #[error("asset symbol must be {min} to {max} characters, got {actual}")]
    SymbolLength {
        /// Minimum allowed characters.
        min: usize,
        /// Maximum allowed characters.
        max: usize,
        /// Actual character count.
        actual: usize,
    },

    // ----- Amounts -----
    /// The number of decimals for an amount is not supported.
    #[error("decimals must be at most {max}, got {actual}")]
    DecimalsOutOfRange {
        /// Maximum supported decimals.
        max: u8,
        /// Actual value.
        actual: u8,
    },

    /// Arithmetic between amounts with different precision.
    #[error("cannot combine amounts with different decimals ({left} and {right})")]
    AmountDecimalsMismatch {
        /// Decimals of the left operand.
        left: u8,
        /// Decimals of the right operand.
        right: u8,
    },

    /// Arithmetic overflowed the 128-bit atomic representation.
    #[error("amount arithmetic overflowed")]
    AmountOverflow,

    /// Subtraction would produce a negative amount.
    #[error("amount arithmetic underflowed")]
    AmountUnderflow,

    /// A decimal string was malformed.
    #[error("invalid decimal amount string: expected digits with an optional fractional part")]
    InvalidAmountString,

    /// A decimal string had more fractional digits than the amount allows.
    #[error("amount has at most {decimals} fractional digits, got {actual}")]
    AmountFractionTooLong {
        /// Decimals allowed by the target amount.
        decimals: u8,
        /// Fractional digits that were supplied.
        actual: usize,
    },

    // ----- Addresses -----
    /// Address string was empty.
    #[error("address must not be empty")]
    EmptyAddress,

    /// Address string contained whitespace.
    #[error("address must not contain whitespace")]
    AddressWhitespace,

    /// Address string contained a non-printable or non-ASCII character.
    #[error("address must contain only printable ASCII characters")]
    AddressNonPrintable,

    /// Address string length was outside the supported range.
    #[error("address must be {min} to {max} characters, got {actual}")]
    AddressLength {
        /// Minimum supported length.
        min: usize,
        /// Maximum supported length.
        max: usize,
        /// Actual length in bytes.
        actual: usize,
    },

    /// Address string does not match the structural format of its network.
    #[error("not a structurally valid {network} address")]
    AddressFormat {
        /// Network the address was expected to belong to.
        network: NetworkId,
    },

    // ----- Derivation paths -----
    /// Derivation path did not start with `m/`.
    #[error("derivation path must start with \"m/\"")]
    DerivationPathMissingMaster,

    /// Derivation path had no segments after `m/`.
    #[error("derivation path must contain at least one segment")]
    DerivationPathEmpty,

    /// Derivation path contained an empty segment.
    #[error("derivation path must not contain empty segments")]
    DerivationPathEmptySegment,

    /// Derivation path segment was not a decimal index with an optional `'`.
    #[error("invalid derivation path segment: {segment}")]
    DerivationPathInvalidIndex {
        /// The offending segment, as supplied.
        segment: String,
    },

    /// Derivation path index is not below the hardened offset.
    #[error("derivation path index {index} must be below 2147483648")]
    DerivationPathIndexOutOfRange {
        /// The offending index.
        index: u32,
    },

    /// Derivation path had more segments than supported.
    #[error("derivation path depth must be at most {max}, got {depth}")]
    DerivationPathTooDeep {
        /// Maximum supported depth.
        max: usize,
        /// Actual depth.
        depth: usize,
    },

    // ----- Composition rules -----
    /// An account address belongs to a different network than the account.
    #[error("address belongs to {actual} but the account is on {expected}")]
    NetworkMismatch {
        /// Network of the account.
        expected: NetworkId,
        /// Network of the address.
        actual: NetworkId,
    },

    /// A token standard is not usable on the given network.
    #[error("{standard} tokens are not supported on {network}")]
    TokenStandardNotSupported {
        /// The requested token standard.
        standard: crate::asset::TokenStandard,
        /// The network the token was declared on.
        network: NetworkId,
    },

    /// Two accounts in the same wallet used the same address on one network.
    #[error("this address is already present in the wallet")]
    DuplicateAccountAddress,

    /// An account was placed in a wallet that is not its owner.
    #[error("account {account} belongs to wallet {owner}, not {wallet}")]
    AccountWalletMismatch {
        /// The account being restored.
        account: AccountId,
        /// The wallet that contains it.
        wallet: WalletId,
        /// The wallet recorded on the account.
        owner: WalletId,
    },

    /// A timestamp was outside the supported range.
    #[error("timestamp must be a non-negative number of milliseconds since the Unix epoch")]
    TimestampOutOfRange,

    /// The system clock reported a time before the Unix epoch.
    #[error("system clock is set before the Unix epoch")]
    ClockBeforeEpoch,
}
