//! Exact decimal asset amounts.
//!
//! Amounts are stored as unsigned integer "atomic" units plus the asset's
//! decimals. Floating point is never used for balances or values, so no
//! rounding error can be introduced by the core.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

/// Largest supported number of decimals for an amount.
///
/// Chosen to cover every supported network's native asset (at most 18 decimals
/// on EVM chains) and their tokens. It is an upper bound on formatting
/// precision, not a statement about any particular asset.
pub const MAX_DECIMALS: u8 = 18;

/// An exact, non-negative amount of an asset in atomic units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    atomic: u128,
    decimals: u8,
}

impl Amount {
    /// Builds an amount from atomic units and the asset's decimals.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DecimalsOutOfRange`] when `decimals` exceeds
    /// [`MAX_DECIMALS`].
    pub fn from_atomic(atomic: u128, decimals: u8) -> Result<Self, DomainError> {
        if decimals > MAX_DECIMALS {
            return Err(DomainError::DecimalsOutOfRange {
                max: MAX_DECIMALS,
                actual: decimals,
            });
        }
        Ok(Self { atomic, decimals })
    }

    /// Zero in the given precision.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DecimalsOutOfRange`] when `decimals` is too large.
    pub fn zero(decimals: u8) -> Result<Self, DomainError> {
        Self::from_atomic(0, decimals)
    }

    /// Parses a decimal string such as `"1.25"` into an amount.
    ///
    /// A sign is rejected: amounts are non-negative and a negative value would
    /// silently change meaning. Trailing zeros beyond the asset precision are
    /// rejected rather than rounded away.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidAmountString`] for malformed input,
    /// [`DomainError::AmountFractionTooLong`] when more fractional digits are
    /// supplied than the precision allows, and [`DomainError::AmountOverflow`]
    /// when the value does not fit in 128 bits.
    pub fn from_decimal_str(input: &str, decimals: u8) -> Result<Self, DomainError> {
        if decimals > MAX_DECIMALS {
            return Err(DomainError::DecimalsOutOfRange {
                max: MAX_DECIMALS,
                actual: decimals,
            });
        }

        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.starts_with('+') {
            return Err(DomainError::InvalidAmountString);
        }

        let mut parts = trimmed.split('.');
        let whole = parts.next().unwrap_or_default();
        let fraction = parts.next();
        if parts.next().is_some() {
            return Err(DomainError::InvalidAmountString);
        }
        if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DomainError::InvalidAmountString);
        }

        let fraction = fraction.unwrap_or_default();
        if fraction.is_empty() && trimmed.contains('.') {
            return Err(DomainError::InvalidAmountString);
        }
        if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DomainError::InvalidAmountString);
        }
        if fraction.len() > usize::from(decimals) {
            return Err(DomainError::AmountFractionTooLong {
                decimals,
                actual: fraction.len(),
            });
        }

        // The integer part is digits-only at this point, so a parse failure can
        // only mean the value exceeds 128 bits.
        let whole_value: u128 = whole.parse().map_err(|_| DomainError::AmountOverflow)?;
        let scale = 10u128
            .checked_pow(u32::from(decimals))
            .ok_or(DomainError::AmountOverflow)?;
        let mut atomic = whole_value
            .checked_mul(scale)
            .ok_or(DomainError::AmountOverflow)?;

        if !fraction.is_empty() {
            let mut fractional: u128 = fraction
                .parse()
                .map_err(|_| DomainError::InvalidAmountString)?;
            for _ in 0..(usize::from(decimals) - fraction.len()) {
                fractional = fractional
                    .checked_mul(10)
                    .ok_or(DomainError::AmountOverflow)?;
            }
            atomic = atomic
                .checked_add(fractional)
                .ok_or(DomainError::AmountOverflow)?;
        }

        Self::from_atomic(atomic, decimals)
    }

    /// Atomic units.
    #[must_use]
    pub fn atomic(&self) -> u128 {
        self.atomic
    }

    /// Decimals of the amount's asset.
    #[must_use]
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Whether the amount is exactly zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.atomic == 0
    }

    /// Adds two amounts of the same precision.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::AmountDecimalsMismatch`] for different precision
    /// and [`DomainError::AmountOverflow`] on overflow.
    pub fn checked_add(&self, other: Self) -> Result<Self, DomainError> {
        self.ensure_same_decimals(other)?;
        let atomic = self
            .atomic
            .checked_add(other.atomic)
            .ok_or(DomainError::AmountOverflow)?;
        Self::from_atomic(atomic, self.decimals)
    }

    /// Subtracts an amount of the same precision.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::AmountDecimalsMismatch`] for different precision
    /// and [`DomainError::AmountUnderflow`] when the result would be negative.
    pub fn checked_sub(&self, other: Self) -> Result<Self, DomainError> {
        self.ensure_same_decimals(other)?;
        let atomic = self
            .atomic
            .checked_sub(other.atomic)
            .ok_or(DomainError::AmountUnderflow)?;
        Self::from_atomic(atomic, self.decimals)
    }

    fn ensure_same_decimals(&self, other: Self) -> Result<(), DomainError> {
        if self.decimals != other.decimals {
            return Err(DomainError::AmountDecimalsMismatch {
                left: self.decimals,
                right: other.decimals,
            });
        }
        Ok(())
    }

    /// Exact decimal representation with all of the asset's fractional digits.
    ///
    /// No grouping separators are inserted: separators are a presentation
    /// decision and must not leak into stored or synchronized values.
    #[must_use]
    pub fn to_decimal_string(&self) -> String {
        if self.decimals == 0 {
            return self.atomic.to_string();
        }
        let divisor = 10u128.pow(u32::from(self.decimals));
        let whole = self.atomic / divisor;
        let fraction = self.atomic % divisor;
        format!(
            "{whole}.{fraction:0width$}",
            width = usize::from(self.decimals)
        )
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_decimal_string())
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawAmount {
            atomic: u128,
            decimals: u8,
        }

        let raw = RawAmount::deserialize(deserializer)?;
        Self::from_atomic(raw.atomic, raw.decimals).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_exact_precision() {
        let amount = Amount::from_atomic(1_234_500_000, 8).expect("valid amount");
        assert_eq!(amount.to_decimal_string(), "12.34500000");
    }

    #[test]
    fn formats_whole_numbers_and_zero_decimals() {
        assert_eq!(
            Amount::from_atomic(42, 0)
                .expect("valid")
                .to_decimal_string(),
            "42"
        );
        assert_eq!(
            Amount::from_atomic(0, 8)
                .expect("valid")
                .to_decimal_string(),
            "0.00000000"
        );
    }

    #[test]
    fn formats_the_smallest_unit() {
        let amount = Amount::from_atomic(1, 18).expect("valid amount");
        assert_eq!(amount.to_decimal_string(), "0.000000000000000001");
    }

    #[test]
    fn parses_decimal_strings() {
        let amount = Amount::from_decimal_str("12.345", 8).expect("parse amount");
        assert_eq!(amount.atomic(), 1_234_500_000);
        assert_eq!(amount.decimals(), 8);

        let whole = Amount::from_decimal_str("7", 8).expect("parse whole amount");
        assert_eq!(whole.atomic(), 700_000_000);
    }

    #[test]
    fn decimal_round_trip_is_lossless() {
        for (input, decimals) in [("0.00000001", 8u8), ("1234.5", 8), ("0", 18), ("999", 0)] {
            let amount = Amount::from_decimal_str(input, decimals).expect("parse amount");
            let reparsed =
                Amount::from_decimal_str(&amount.to_decimal_string(), decimals).expect("reparse");
            assert_eq!(amount, reparsed, "round trip failed for {input}");
        }
    }

    #[test]
    fn rejects_malformed_decimal_strings() {
        for input in [
            "", " ", "1.2.3", "1.", ".5", "abc", "-1", "+1", "1,5", "1e5",
        ] {
            assert_eq!(
                Amount::from_decimal_str(input, 8),
                Err(DomainError::InvalidAmountString),
                "input {input:?} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_too_many_fractional_digits_instead_of_rounding() {
        assert_eq!(
            Amount::from_decimal_str("1.123456789", 8),
            Err(DomainError::AmountFractionTooLong {
                decimals: 8,
                actual: 9
            })
        );
    }

    #[test]
    fn rejects_out_of_range_decimals() {
        assert_eq!(
            Amount::from_atomic(1, 19),
            Err(DomainError::DecimalsOutOfRange {
                max: MAX_DECIMALS,
                actual: 19
            })
        );
        assert_eq!(
            Amount::from_decimal_str("1", 19),
            Err(DomainError::DecimalsOutOfRange {
                max: MAX_DECIMALS,
                actual: 19
            })
        );
    }

    #[test]
    fn rejects_value_that_overflows_128_bits() {
        let too_large = "340282366920938463463374607431768211456";
        assert_eq!(
            Amount::from_decimal_str(too_large, 0),
            Err(DomainError::AmountOverflow)
        );
    }

    #[test]
    fn arithmetic_checks_precision_and_bounds() {
        let one = Amount::from_atomic(1, 8).expect("valid");
        let two = Amount::from_atomic(2, 8).expect("valid");
        assert_eq!(
            one.checked_add(two).expect("add").atomic(),
            3,
            "atomic addition"
        );
        assert_eq!(
            two.checked_sub(one).expect("subtract").atomic(),
            1,
            "atomic subtraction"
        );
        assert_eq!(one.checked_sub(two), Err(DomainError::AmountUnderflow));
        assert_eq!(
            one.checked_add(Amount::from_atomic(1, 18).expect("valid")),
            Err(DomainError::AmountDecimalsMismatch { left: 8, right: 18 })
        );
        assert_eq!(
            Amount::from_atomic(u128::MAX, 8)
                .expect("valid")
                .checked_add(one),
            Err(DomainError::AmountOverflow)
        );
    }

    #[test]
    fn zero_is_detected() {
        assert!(Amount::zero(8).expect("zero").is_zero());
        assert!(!Amount::from_atomic(1, 8).expect("valid").is_zero());
    }

    #[test]
    fn deserialization_validates_decimals() {
        let ok: Amount = serde_json::from_str(r#"{"atomic":150000000,"decimals":8}"#)
            .expect("deserialize amount");
        assert_eq!(ok.to_decimal_string(), "1.50000000");

        assert!(serde_json::from_str::<Amount>(r#"{"atomic":1,"decimals":19}"#).is_err());
    }
}
