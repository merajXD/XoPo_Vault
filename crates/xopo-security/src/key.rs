//! The 32-byte data key, with redacted output and zeroization on drop.

use std::fmt;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::SecurityError;
use crate::rng;

/// A 256-bit secret key held in memory.
///
/// Deliberately not `Clone` and not printable: a copy is created only when a
/// caller asks for the bytes, and `Debug` never renders them. The buffer is
/// zeroized when the value is dropped. Zeroization cannot reach copies the
/// operating system may have made (swap, core dumps), which is why
/// `docs/SECURITY.md` calls this defence in depth.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; 32]);

impl SecretKey {
    /// Wraps existing key bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Generates a fresh key from the operating system's random source.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::EntropySource`] when the OS random source fails.
    /// There is no fallback: a predictable key is worse than a refusal.
    pub fn generate() -> Result<Self, SecurityError> {
        Ok(Self(rng::random_bytes::<32>()?))
    }

    /// Borrows the raw bytes.
    ///
    /// Hand these only to the component that needs the key itself (the database
    /// open call, the AEAD helpers). Never format them, never log them and never
    /// write them to a file.
    #[must_use]
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for SecretKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for SecretKey {}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(redacted)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_bytes() {
        let key = SecretKey::from_bytes([0xab; 32]);
        let rendered = format!("{key:?}");
        assert_eq!(rendered, "SecretKey(redacted)");
        assert!(!rendered.contains("abab"));
    }

    #[test]
    fn generated_keys_differ() {
        let first = SecretKey::generate().expect("entropy");
        let second = SecretKey::generate().expect("entropy");
        assert_ne!(first, second);
        assert_ne!(first.expose(), &[0u8; 32]);
    }

    #[test]
    fn equality_compares_the_bytes() {
        assert_eq!(
            SecretKey::from_bytes([1u8; 32]),
            SecretKey::from_bytes([1u8; 32])
        );
        assert_ne!(
            SecretKey::from_bytes([1u8; 32]),
            SecretKey::from_bytes([2u8; 32])
        );
    }
}
