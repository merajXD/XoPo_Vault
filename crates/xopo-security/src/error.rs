//! Errors from the security layer.
//!
//! Every variant is written for a log line or a user-facing message: none of
//! them contains a password, a key, a salt or decrypted bytes.

use thiserror::Error;

/// Something the security layer refused to do.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// The password did not unwrap the data key, or the wrapped key was altered.
    ///
    /// The two cases are deliberately indistinguishable: saying "wrong password"
    /// for a tampered file would tell an attacker which of the two they achieved.
    #[error("the password is wrong or the sealed key was altered")]
    WrongPasswordOrCorrupt,

    /// An AEAD tag did not verify for a sealed secret.
    #[error("the sealed secret could not be authenticated")]
    AuthenticationFailed,

    /// The bytes are not a well-formed structure this build understands.
    #[error("the sealed data is malformed: {reason}")]
    Malformed {
        /// What is structurally wrong, without echoing any of the bytes.
        reason: &'static str,
    },

    /// KDF parameters are outside the accepted range or below the policy floor.
    #[error("key-derivation parameters are not acceptable: {reason}")]
    ParameterPolicy {
        /// Why the parameters were rejected.
        reason: &'static str,
    },

    /// The operating system's random source failed.
    #[error("the operating system random source failed")]
    EntropySource,

    /// The plaintext handed in is larger than the format allows.
    #[error("the secret is larger than the {max}-byte limit")]
    SecretTooLarge {
        /// The limit that was exceeded.
        max: usize,
    },

    /// The cipher refused to encrypt, which only happens for absurd inputs.
    #[error("the cipher refused to seal the data")]
    EncryptionFailed,
}
