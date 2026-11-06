//! ChaCha20-Poly1305 sealing, with single-use random nonces.
//!
//! Nothing here is hand-written: the primitive comes from `chacha20poly1305`,
//! and this module only fixes the nonce size and maps failures to project
//! errors. A nonce is generated per seal and never reused, because reuse with the
//! same key breaks the cipher completely.

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};

use crate::error::SecurityError;
use crate::rng;

/// Nonce length for ChaCha20-Poly1305.
pub(crate) const NONCE_LEN: usize = 12;
/// Authentication tag length appended to every ciphertext.
pub(crate) const TAG_LEN: usize = 16;

/// Encrypts and authenticates `plaintext`, binding `aad` to the result.
///
/// The associated data is authenticated but not encrypted; the envelope uses it
/// for the header, so changing a KDF parameter invalidates the tag.
pub(crate) fn seal(
    key: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, SecurityError> {
    let cipher = ChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| SecurityError::EncryptionFailed)
}

/// Verifies the tag and returns the plaintext, or fails without revealing why.
pub(crate) fn open(
    key: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, SecurityError> {
    let cipher = ChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| SecurityError::AuthenticationFailed)
}

/// Generates a fresh nonce from the OS random source.
pub(crate) fn random_nonce() -> Result<[u8; NONCE_LEN], SecurityError> {
    rng::random_bytes::<NONCE_LEN>()
}
