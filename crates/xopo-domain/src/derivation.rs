//! BIP-32 derivation paths.
//!
//! This module parses and formats paths only. It does not derive keys and does
//! not claim that any path is the "correct" default for a network: purposes and
//! coin types are supplied by the caller, and the wallet layer documents each
//! supported scheme.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::DomainError;

/// Offset at which BIP-32 hardened child indexes begin.
pub const HARDENED_OFFSET: u32 = 0x8000_0000;

/// Maximum number of path segments accepted.
///
/// BIP-44 addresses are five levels deep; the limit leaves room for account
/// hierarchies while keeping paths bounded and comparable.
pub const MAX_DEPTH: usize = 10;

/// One BIP-32 path segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSegment {
    index: u32,
    hardened: bool,
}

impl PathSegment {
    /// Creates a segment from an index below [`HARDENED_OFFSET`].
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DerivationPathIndexOutOfRange`] for indexes at or
    /// above the hardened offset; the hardened bit is expressed by the flag, not
    /// by the index.
    pub fn new(index: u32, hardened: bool) -> Result<Self, DomainError> {
        if index >= HARDENED_OFFSET {
            return Err(DomainError::DerivationPathIndexOutOfRange { index });
        }
        Ok(Self { index, hardened })
    }

    /// Child index without the hardened bit.
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Whether this child is hardened.
    #[must_use]
    pub fn is_hardened(&self) -> bool {
        self.hardened
    }

    fn parse(raw: &str) -> Result<Self, DomainError> {
        let (digits, hardened) = if let Some(stripped) = raw.strip_suffix('\'') {
            (stripped, true)
        } else if let Some(stripped) = raw.strip_suffix('h') {
            (stripped, true)
        } else {
            (raw, false)
        };

        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DomainError::DerivationPathInvalidIndex {
                segment: raw.to_owned(),
            });
        }
        let index: u32 = digits
            .parse()
            .map_err(|_| DomainError::DerivationPathInvalidIndex {
                segment: raw.to_owned(),
            })?;
        Self::new(index, hardened)
    }
}

impl fmt::Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.hardened {
            write!(f, "{}'", self.index)
        } else {
            write!(f, "{}", self.index)
        }
    }
}

/// A validated BIP-32 derivation path such as `m/84'/0'/0'/0/0`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DerivationPath {
    segments: Vec<PathSegment>,
}

impl DerivationPath {
    /// Parses a path in `m/…` form. Both `'` and `h` mark hardened segments.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DerivationPathMissingMaster`] when the path does
    /// not start with `m/`, [`DomainError::DerivationPathEmpty`] for `m/`,
    /// [`DomainError::DerivationPathEmptySegment`] for `m//0`,
    /// [`DomainError::DerivationPathInvalidIndex`] for non-numeric segments,
    /// [`DomainError::DerivationPathIndexOutOfRange`] for indexes at or above
    /// 2^31, and [`DomainError::DerivationPathTooDeep`] beyond [`MAX_DEPTH`].
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let rest = raw
            .strip_prefix("m/")
            .ok_or(DomainError::DerivationPathMissingMaster)?;
        if rest.is_empty() {
            return Err(DomainError::DerivationPathEmpty);
        }

        let mut segments = Vec::new();
        for part in rest.split('/') {
            if part.is_empty() {
                return Err(DomainError::DerivationPathEmptySegment);
            }
            segments.push(PathSegment::parse(part)?);
        }
        if segments.len() > MAX_DEPTH {
            return Err(DomainError::DerivationPathTooDeep {
                max: MAX_DEPTH,
                depth: segments.len(),
            });
        }
        Ok(Self { segments })
    }

    /// Builds a path from already validated segments.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DerivationPathEmpty`] for an empty slice and
    /// [`DomainError::DerivationPathTooDeep`] beyond [`MAX_DEPTH`].
    pub fn from_segments(
        segments: impl IntoIterator<Item = PathSegment>,
    ) -> Result<Self, DomainError> {
        let segments: Vec<PathSegment> = segments.into_iter().collect();
        if segments.is_empty() {
            return Err(DomainError::DerivationPathEmpty);
        }
        if segments.len() > MAX_DEPTH {
            return Err(DomainError::DerivationPathTooDeep {
                max: MAX_DEPTH,
                depth: segments.len(),
            });
        }
        Ok(Self { segments })
    }

    /// Account-level path `m/purpose'/coin'/account'`.
    ///
    /// The purpose (for example 44 or 84) and coin type are caller-supplied,
    /// because choosing a standard is a wallet-layer decision.
    ///
    /// # Errors
    ///
    /// Returns an index error if any component is at or above 2^31.
    pub fn for_account(purpose: u32, coin_type: u32, account: u32) -> Result<Self, DomainError> {
        Self::from_segments([
            PathSegment::new(purpose, true)?,
            PathSegment::new(coin_type, true)?,
            PathSegment::new(account, true)?,
        ])
    }

    /// Address-level path `m/purpose'/coin'/account'/change/index`.
    ///
    /// # Errors
    ///
    /// Returns an index error if any component is at or above 2^31.
    pub fn for_address(
        purpose: u32,
        coin_type: u32,
        account: u32,
        change: u32,
        index: u32,
    ) -> Result<Self, DomainError> {
        Self::from_segments([
            PathSegment::new(purpose, true)?,
            PathSegment::new(coin_type, true)?,
            PathSegment::new(account, true)?,
            PathSegment::new(change, false)?,
            PathSegment::new(index, false)?,
        ])
    }

    /// The validated segments, in order.
    #[must_use]
    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    /// Number of segments.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.segments.len()
    }
}

impl fmt::Display for DerivationPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("m")?;
        for segment in &self.segments {
            write!(f, "/{segment}")?;
        }
        Ok(())
    }
}

impl Serialize for DerivationPath {
    /// Paths travel as their canonical text form, never as a raw segment array.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl FromStr for DerivationPath {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for DerivationPath {
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
    fn parses_bip84_paths() {
        let path = DerivationPath::parse("m/84'/0'/0'/0/0").expect("parse path");
        assert_eq!(path.depth(), 5);
        assert_eq!(path.segments()[0].index(), 84);
        assert!(path.segments()[0].is_hardened());
        assert!(!path.segments()[3].is_hardened());
        assert_eq!(path.to_string(), "m/84'/0'/0'/0/0");
    }

    #[test]
    fn accepts_h_as_a_hardened_marker_and_canonicalises_it() {
        let path = DerivationPath::parse("m/44h/0h/0h").expect("parse path");
        assert_eq!(path.to_string(), "m/44'/0'/0'");
    }

    #[test]
    fn rejects_invalid_paths() {
        assert_eq!(
            DerivationPath::parse("84'/0'"),
            Err(DomainError::DerivationPathMissingMaster)
        );
        assert_eq!(
            DerivationPath::parse("m/"),
            Err(DomainError::DerivationPathEmpty)
        );
        assert_eq!(
            DerivationPath::parse("m//0"),
            Err(DomainError::DerivationPathEmptySegment)
        );
        assert_eq!(
            DerivationPath::parse("m/84'/abc"),
            Err(DomainError::DerivationPathInvalidIndex {
                segment: "abc".to_owned()
            })
        );
        assert_eq!(
            DerivationPath::parse("m/84''"),
            Err(DomainError::DerivationPathInvalidIndex {
                segment: "84''".to_owned()
            })
        );
        assert_eq!(
            DerivationPath::parse("m/2147483648"),
            Err(DomainError::DerivationPathIndexOutOfRange {
                index: 2_147_483_648
            })
        );
        // A hardened marker on an out-of-range index is still out of range.
        assert_eq!(
            DerivationPath::parse("m/2147483648'"),
            Err(DomainError::DerivationPathIndexOutOfRange {
                index: 2_147_483_648
            })
        );
    }

    #[test]
    fn depth_is_bounded() {
        let deep = format!("m/{}", ["0"; MAX_DEPTH + 1].join("/"));
        assert_eq!(
            DerivationPath::parse(&deep),
            Err(DomainError::DerivationPathTooDeep {
                max: MAX_DEPTH,
                depth: MAX_DEPTH + 1
            })
        );
        let allowed = format!("m/{}", ["0"; MAX_DEPTH].join("/"));
        assert!(DerivationPath::parse(&allowed).is_ok());
    }

    #[test]
    fn account_and_address_paths_are_built_from_validated_parts() {
        assert_eq!(
            DerivationPath::for_account(84, 0, 0)
                .expect("path")
                .to_string(),
            "m/84'/0'/0'"
        );
        assert_eq!(
            DerivationPath::for_address(44, 607, 0, 0, 7)
                .expect("path")
                .to_string(),
            "m/44'/607'/0'/0/7"
        );
        assert!(DerivationPath::for_account(HARDENED_OFFSET, 0, 0).is_err());
    }

    #[test]
    fn paths_round_trip_through_json() {
        let path = DerivationPath::parse("m/44'/60'/0'/0/1").expect("path");
        let json = serde_json::to_string(&path).expect("serialize path");
        assert_eq!(json, "\"m/44'/60'/0'/0/1\"");
        let parsed: DerivationPath = serde_json::from_str(&json).expect("deserialize path");
        assert_eq!(parsed, path);
        assert!(serde_json::from_str::<DerivationPath>("\"m/x\"").is_err());
    }
}
