//! The wrapped data key: password in, sealed 90-byte header out.
//!
//! ```text
//! offset  size  field
//!      0     4  magic "XDVK"
//!      4     1  format version
//!      5     4  Argon2id memory cost (KiB, big endian)
//!      9     4  Argon2id iterations
//!     13     1  Argon2id parallelism
//!     14    16  salt
//!     30    12  nonce
//!     42    48  DEK sealed with ChaCha20-Poly1305 (32-byte key + 16-byte tag)
//! ```
//!
//! Bytes 0–41 are the associated data of the seal, so every header field is
//! authenticated: flipping the iteration count downwards does not just fail the
//! policy check, it also invalidates the tag. The data key is random and
//! independent of the password, which is what makes a password change a 90-byte
//! rewrite instead of a full re-encryption.

use std::fmt;

use zeroize::Zeroizing;

use crate::aead;
use crate::error::SecurityError;
use crate::kdf::{KdfParams, SALT_LEN};
use crate::key::SecretKey;
use crate::rng;

/// Magic bytes at the start of every envelope.
const MAGIC: [u8; 4] = *b"XDVK";
/// Format version written by this build.
pub const ENVELOPE_VERSION: u8 = 1;

const VERSION_AT: usize = 4;
const MEMORY_AT: usize = 5;
const ITERATIONS_AT: usize = 9;
const PARALLELISM_AT: usize = 13;
const SALT_AT: usize = 14;
const NONCE_AT: usize = SALT_AT + SALT_LEN;
const WRAPPED_AT: usize = NONCE_AT + aead::NONCE_LEN;
/// Length of the sealed data key: 32 bytes plus the authentication tag.
const WRAPPED_LEN: usize = 32 + aead::TAG_LEN;

/// Total serialized length of an envelope.
pub const ENVELOPE_LEN: usize = WRAPPED_AT + WRAPPED_LEN;

/// A data key wrapped by a password-derived key.
///
/// The value is a fixed 90-byte record; serialize it with [`Self::to_bytes`] and
/// store those bytes wherever the key store lives. `Debug` prints the parameters
/// and the format version, never the sealed bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyEnvelope {
    bytes: [u8; ENVELOPE_LEN],
}

impl KeyEnvelope {
    /// Creates a new random data key and wraps it under `password`.
    ///
    /// Returns the envelope to store and the data key to use for this session.
    /// The caller owns the key from here on; dropping it zeroizes it.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::EntropySource`] when the OS random source fails,
    /// and [`SecurityError::EncryptionFailed`] if the cipher refuses the input.
    pub fn seal(password: &str, params: KdfParams) -> Result<(Self, SecretKey), SecurityError> {
        let data_key = SecretKey::generate()?;
        let envelope = Self::wrap(&data_key, password, params)?;
        Ok((envelope, data_key))
    }

    /// Wraps an existing data key under a new password and a fresh salt.
    ///
    /// This is the password-change path: the data key, and therefore every
    /// secret already sealed with it, stays valid.
    ///
    /// # Errors
    ///
    /// As [`Self::seal`].
    pub fn wrap(
        data_key: &SecretKey,
        password: &str,
        params: KdfParams,
    ) -> Result<Self, SecurityError> {
        let salt = rng::random_bytes::<SALT_LEN>()?;
        let nonce = aead::random_nonce()?;

        let mut bytes = [0u8; ENVELOPE_LEN];
        bytes[..4].copy_from_slice(&MAGIC);
        bytes[VERSION_AT] = ENVELOPE_VERSION;
        bytes[MEMORY_AT..ITERATIONS_AT].copy_from_slice(&params.memory_kib().to_be_bytes());
        bytes[ITERATIONS_AT..PARALLELISM_AT].copy_from_slice(&params.iterations().to_be_bytes());
        bytes[PARALLELISM_AT] = params.parallelism().to_le_bytes()[0];
        bytes[SALT_AT..NONCE_AT].copy_from_slice(&salt);
        bytes[NONCE_AT..WRAPPED_AT].copy_from_slice(&nonce);

        let key_encryption_key = params.derive(password, &salt)?;
        let plaintext = Zeroizing::new(*data_key.expose());
        let wrapped = aead::seal(
            key_encryption_key.expose(),
            &nonce,
            &bytes[..WRAPPED_AT],
            &plaintext[..],
        )?;
        bytes[WRAPPED_AT..].copy_from_slice(&wrapped);

        Ok(Self { bytes })
    }

    /// Recovers the data key from the password.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::WrongPasswordOrCorrupt`] for a wrong password or
    /// any alteration of the sealed bytes. The two cannot be told apart, by
    /// design: both leave the data key unrecoverable.
    pub fn unseal(&self, password: &str) -> Result<SecretKey, SecurityError> {
        let parsed = self.parsed()?;
        let key_encryption_key = parsed.params.derive(password, &parsed.salt)?;
        let wrapped = &self.bytes[WRAPPED_AT..];
        let plaintext = aead::open(
            key_encryption_key.expose(),
            &parsed.nonce,
            &self.bytes[..WRAPPED_AT],
            wrapped,
        )
        .map_err(|_| SecurityError::WrongPasswordOrCorrupt)?;
        let plaintext = Zeroizing::new(plaintext);

        let mut key = [0u8; 32];
        key.copy_from_slice(&plaintext);
        Ok(SecretKey::from_bytes(key))
    }

    /// The parameters recorded in this envelope.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::Malformed`] when the bytes are not a usable
    /// envelope. A value obtained from [`Self::from_bytes`] always parses.
    pub fn params(&self) -> Result<KdfParams, SecurityError> {
        Ok(self.parsed()?.params)
    }

    /// Serializes the envelope for storage.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; ENVELOPE_LEN] {
        self.bytes
    }

    /// Reads an envelope read back from storage.
    ///
    /// Every field is validated here — length, magic, version and the KDF
    /// parameters — so the rest of the API can assume a well-formed record.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::Malformed`] for a wrong length, magic or
    /// version, and [`SecurityError::ParameterPolicy`] when the recorded
    /// parameters are outside the accepted range.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SecurityError> {
        if bytes.len() != ENVELOPE_LEN {
            return Err(SecurityError::Malformed {
                reason: "an envelope is exactly 90 bytes",
            });
        }
        let mut value = [0u8; ENVELOPE_LEN];
        value.copy_from_slice(bytes);
        let envelope = Self { bytes: value };
        envelope.parsed()?;
        Ok(envelope)
    }

    fn parsed(&self) -> Result<Parsed, SecurityError> {
        if self.bytes[..4] != MAGIC {
            return Err(SecurityError::Malformed {
                reason: "the magic bytes do not match",
            });
        }
        if self.bytes[VERSION_AT] != ENVELOPE_VERSION {
            return Err(SecurityError::Malformed {
                reason: "the format version is not supported",
            });
        }
        let params = KdfParams::new(
            read_u32(&self.bytes, MEMORY_AT)?,
            read_u32(&self.bytes, ITERATIONS_AT)?,
            u32::from(self.bytes[PARALLELISM_AT]),
        )?;
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&self.bytes[SALT_AT..NONCE_AT]);
        let mut nonce = [0u8; aead::NONCE_LEN];
        nonce.copy_from_slice(&self.bytes[NONCE_AT..WRAPPED_AT]);
        Ok(Parsed {
            params,
            salt,
            nonce,
        })
    }
}

struct Parsed {
    params: KdfParams,
    salt: [u8; SALT_LEN],
    nonce: [u8; aead::NONCE_LEN],
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, SecurityError> {
    let slice = bytes.get(at..at + 4).ok_or(SecurityError::Malformed {
        reason: "a header field is truncated",
    })?;
    let mut value = [0u8; 4];
    value.copy_from_slice(slice);
    Ok(u32::from_be_bytes(value))
}

impl fmt::Debug for KeyEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyEnvelope")
            .field("version", &ENVELOPE_VERSION)
            .field("params", &self.params().ok())
            .field("sealed", &"redacted")
            .finish()
    }
}
