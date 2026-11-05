//! Validation of short human-readable text and asset symbols.
//!
//! Both types are parsed once at the boundary and then carry their own
//! guarantee, so later code cannot accidentally store an empty label, an
//! over-long name or an emoji-laden symbol that breaks table layout.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

/// Maximum characters in a label, wallet name or asset display name.
pub const MAX_LABEL_CHARS: usize = 64;

/// Shortest asset symbol accepted.
pub const MIN_SYMBOL_CHARS: usize = 2;

/// Longest asset symbol accepted.
pub const MAX_SYMBOL_CHARS: usize = 12;

/// A trimmed, non-empty, single-line piece of user-visible text.
///
/// Used for wallet labels, account labels and asset names. Characters are
/// counted as Unicode scalar values, not bytes, so a Persian label is measured
/// the way the user sees it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    /// Validates and wraps a label.
    ///
    /// Surrounding whitespace is rejected rather than silently removed: the
    /// stored value must be exactly what the user typed.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyLabel`], [`DomainError::LabelSurroundingWhitespace`],
    /// [`DomainError::LabelControlCharacter`] or [`DomainError::LabelTooLong`].
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        if raw.is_empty() {
            return Err(DomainError::EmptyLabel);
        }
        if raw.trim() != raw {
            return Err(DomainError::LabelSurroundingWhitespace);
        }
        if raw.chars().any(char::is_control) {
            return Err(DomainError::LabelControlCharacter);
        }
        let length = raw.chars().count();
        if length > MAX_LABEL_CHARS {
            return Err(DomainError::LabelTooLong {
                max: MAX_LABEL_CHARS,
                actual: length,
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// The validated text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Label {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// A canonical asset ticker symbol.
///
/// Symbols are uppercase ASCII letters and digits only. Lowercase input is
/// rejected instead of being uppercased behind the user's back, so imported
/// metadata that does not follow the convention is visible immediately.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Symbol(String);

impl Symbol {
    /// Validates and wraps an asset symbol.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidSymbol`] or [`DomainError::SymbolLength`].
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        if !raw
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return Err(DomainError::InvalidSymbol);
        }
        let length = raw.chars().count();
        if !(MIN_SYMBOL_CHARS..=MAX_SYMBOL_CHARS).contains(&length) {
            return Err(DomainError::SymbolLength {
                min: MIN_SYMBOL_CHARS,
                max: MAX_SYMBOL_CHARS,
                actual: length,
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// The validated symbol.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Symbol {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for Symbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_labels() {
        assert_eq!(
            Label::parse("Main wallet").expect("label").as_str(),
            "Main wallet"
        );
        assert_eq!(
            Label::parse("کیف پول اصلی")
                .expect("persian label")
                .as_str(),
            "کیف پول اصلی"
        );
    }

    #[test]
    fn rejects_invalid_labels() {
        assert_eq!(Label::parse(""), Err(DomainError::EmptyLabel));
        assert_eq!(
            Label::parse(" leading"),
            Err(DomainError::LabelSurroundingWhitespace)
        );
        assert_eq!(
            Label::parse("trailing "),
            Err(DomainError::LabelSurroundingWhitespace)
        );
        assert_eq!(
            Label::parse("line\nbreak"),
            Err(DomainError::LabelControlCharacter)
        );
        let too_long = "a".repeat(MAX_LABEL_CHARS + 1);
        assert_eq!(
            Label::parse(&too_long),
            Err(DomainError::LabelTooLong {
                max: MAX_LABEL_CHARS,
                actual: MAX_LABEL_CHARS + 1
            })
        );
    }

    #[test]
    fn label_limit_counts_characters_not_bytes() {
        let persian = "ژ".repeat(MAX_LABEL_CHARS);
        assert!(Label::parse(&persian).is_ok());
    }

    #[test]
    fn accepts_valid_symbols() {
        assert_eq!(Symbol::parse("BTC").expect("symbol").as_str(), "BTC");
        assert_eq!(Symbol::parse("USDT").expect("symbol").as_str(), "USDT");
        assert_eq!(Symbol::parse("1INCH").expect("symbol").as_str(), "1INCH");
    }

    #[test]
    fn rejects_invalid_symbols() {
        assert_eq!(Symbol::parse("btc"), Err(DomainError::InvalidSymbol));
        assert_eq!(Symbol::parse("B TC"), Err(DomainError::InvalidSymbol));
        assert_eq!(
            Symbol::parse("B"),
            Err(DomainError::SymbolLength {
                min: 2,
                max: 12,
                actual: 1
            })
        );
        assert_eq!(
            Symbol::parse("ABCDEFGHIJKLM"),
            Err(DomainError::SymbolLength {
                min: 2,
                max: 12,
                actual: 13
            })
        );
    }

    #[test]
    fn deserialization_applies_the_same_rules() {
        assert!(serde_json::from_str::<Label>("\"\"").is_err());
        assert!(serde_json::from_str::<Symbol>("\"btc\"").is_err());
        let label: Label = serde_json::from_str("\"Savings\"").expect("deserialize label");
        assert_eq!(label.as_str(), "Savings");
    }
}
