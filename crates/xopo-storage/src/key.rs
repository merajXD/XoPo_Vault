//! The database key.
//!
//! The storage layer never derives, generates or stores this key. It is given
//! once at open time by the security layer (`xopo-security`, Phase 4) and is
//! passed to SQLCipher as a raw 256-bit key, so no passphrase stretching happens
//! here and no key material is written to disk by this crate.

use std::fmt;

/// A 256-bit SQLCipher key.
///
/// `Debug` is redacted: a key must never reach a log line, a panic message or a
/// snapshot test.
pub struct DatabaseKey([u8; 32]);

impl DatabaseKey {
    /// Wraps raw key bytes supplied by the caller.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// SQLCipher literal for a raw key: `x'<64 hex digits>'`.
    ///
    /// The `x'…'` form marks the value as binary, so SQLCipher uses these exact
    /// 32 bytes instead of running a KDF over them.
    pub(crate) fn pragma_literal(&self) -> String {
        let mut literal = String::with_capacity(72);
        literal.push_str("x'");
        for byte in self.0 {
            literal.push_str(&format!("{byte:02x}"));
        }
        literal.push('\'');
        literal
    }
}

impl fmt::Debug for DatabaseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseKey(redacted)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_each_key_byte_as_two_hex_digits() {
        let key = DatabaseKey::from_bytes([0x0f; 32]);
        assert_eq!(
            key.pragma_literal(),
            "x'0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f'"
        );
    }

    #[test]
    fn debug_output_is_redacted() {
        let key = DatabaseKey::from_bytes([0xde; 32]);
        let rendered = format!("{key:?}");
        assert_eq!(rendered, "DatabaseKey(redacted)");
        assert!(!rendered.contains("dededede"));
    }
}
