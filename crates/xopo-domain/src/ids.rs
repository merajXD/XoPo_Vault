//! Typed identifiers.
//!
//! Identifiers are random UUIDv4 values wrapped in distinct newtypes so that a
//! wallet identifier can never be passed where an account identifier is
//! expected. The random generator comes from `uuid` (`getrandom`), not from a
//! project-specific scheme.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DomainError;

macro_rules! define_id {
    ($name:ident, $kind:literal) => {
        /// Opaque random identifier.
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a new random identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wraps an existing UUID, for example one read from storage.
            #[must_use]
            pub fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            /// Returns the underlying UUID.
            #[must_use]
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = DomainError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| DomainError::InvalidId { kind: $kind })
            }
        }
    };
}

define_id!(WalletId, "wallet");
define_id!(AccountId, "account");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_unique() {
        assert_ne!(WalletId::new(), WalletId::new());
    }

    #[test]
    fn ids_round_trip_through_text() {
        let id = AccountId::new();
        let parsed: AccountId = id.to_string().parse().expect("parse generated id");
        assert_eq!(id, parsed);
    }

    #[test]
    fn malformed_ids_are_rejected() {
        assert_eq!(
            "not-a-uuid".parse::<WalletId>(),
            Err(DomainError::InvalidId { kind: "wallet" })
        );
    }
}
