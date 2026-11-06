//! Individual secrets sealed with the data key.
//!
//! A seed phrase is not the data key: it is one payload among several, and each
//! payload is sealed separately so the vault can hand out only what it needs.
//! Every seal is bound to a context string — the wallet's identifier — which is
//! authenticated as associated data. A blob copied from one wallet to another
//! therefore fails to open instead of quietly decrypting.

use zeroize::Zeroizing;

use crate::aead;
use crate::error::SecurityError;
use crate::key::SecretKey;

/// Domain separator mixed into every context, so a secret sealed here can never
/// be confused with any other authenticated blob in the format.
const DOMAIN: &[u8] = b"xopo-secret-v1";

/// Largest secret this crate accepts: a seed phrase is a few hundred bytes, so
/// the limit is generous while still bounding allocations from untrusted input.
pub const MAX_SECRET_LEN: usize = 4096;

/// A secret sealed with the data key, bound to one context.
///
/// `PartialEq` compares the sealed bytes, which is what a store needs when it
/// re-reads a record; it never decrypts anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedSecret {
    nonce: [u8; aead::NONCE_LEN],
    ciphertext: Vec<u8>,
}

impl SealedSecret {
    /// Encrypts `plaintext` under `data_key` for `context`.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::SecretTooLarge`] above [`MAX_SECRET_LEN`],
    /// [`SecurityError::EntropySource`] when the OS random source fails, and
    /// [`SecurityError::EncryptionFailed`] if the cipher refuses the input.
    pub fn seal(
        data_key: &SecretKey,
        context: &str,
        plaintext: &[u8],
    ) -> Result<Self, SecurityError> {
        if plaintext.len() > MAX_SECRET_LEN {
            return Err(SecurityError::SecretTooLarge {
                max: MAX_SECRET_LEN,
            });
        }
        let nonce = aead::random_nonce()?;
        let associated = associated_data(context);
        let ciphertext = aead::seal(data_key.expose(), &nonce, &associated, plaintext)?;
        Ok(Self { nonce, ciphertext })
    }

    /// Decrypts the secret for `context`.
    ///
    /// The returned buffer is zeroized when the caller drops it.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::AuthenticationFailed`] for a wrong data key, a
    /// different context or any altered byte. As with passwords, the reason is
    /// not narrowed down further.
    pub fn open(
        &self,
        data_key: &SecretKey,
        context: &str,
    ) -> Result<Zeroizing<Vec<u8>>, SecurityError> {
        let associated = associated_data(context);
        let plaintext = aead::open(
            data_key.expose(),
            &self.nonce,
            &associated,
            &self.ciphertext,
        )?;
        Ok(Zeroizing::new(plaintext))
    }

    /// Serializes as `nonce || ciphertext`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(aead::NONCE_LEN + self.ciphertext.len());
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        bytes
    }

    /// Reads a sealed secret back from storage.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::Malformed`] for anything shorter than a nonce
    /// plus an empty authenticated message.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SecurityError> {
        if bytes.len() < aead::NONCE_LEN + aead::TAG_LEN {
            return Err(SecurityError::Malformed {
                reason: "a sealed secret needs a nonce and an authentication tag",
            });
        }
        if bytes.len() > aead::NONCE_LEN + aead::TAG_LEN + MAX_SECRET_LEN {
            return Err(SecurityError::Malformed {
                reason: "a sealed secret is longer than the format allows",
            });
        }
        let mut nonce = [0u8; aead::NONCE_LEN];
        nonce.copy_from_slice(&bytes[..aead::NONCE_LEN]);
        Ok(Self {
            nonce,
            ciphertext: bytes[aead::NONCE_LEN..].to_vec(),
        })
    }
}

fn associated_data(context: &str) -> Vec<u8> {
    let mut associated = Vec::with_capacity(DOMAIN.len() + 1 + context.len());
    associated.extend_from_slice(DOMAIN);
    associated.push(b'\0');
    associated.extend_from_slice(context.as_bytes());
    associated
}
