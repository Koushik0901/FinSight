//! The single canonical "amount string -> integer cents" conversion.
//!
//! Every ingest path (CSV import, SimpleFin, Enable Banking, holdings) parses
//! through here so all sources round identically. Parsing goes through
//! `Decimal`, never `f64`: `(s.parse::<f64>() * 100.0).round()` loses exact
//! decimal midpoints (e.g. `"2.675"` is stored below the midpoint and rounds
//! to 267 cents instead of 268). `Decimal` keeps the string's exact value and
//! `round_dp` applies banker's rounding (MidpointNearestEven).
//!
//! The range cap mirrors Actual Budget's `safeNumber` guard: display formats
//! cents as `cents / 100` in JS, and above 2^51 that division can land on a
//! double that renders different cents than were stored. Actual uses 2^51
//! (not 2^53) precisely to leave the division headroom — reject at ingest
//! what the UI cannot faithfully show.

use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Largest cent amount the UI can format without float division corrupting
/// the displayed value. Same bound as Actual Budget's `MAX_SAFE_NUMBER`.
pub const MAX_SAFE_CENTS: i64 = (1 << 51) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CentsError {
    /// Not a number at all.
    Invalid,
    /// A number too large to represent as display-safe cents.
    OutOfRange,
}

/// Parse a signed decimal amount string (e.g. `"-33293.43"`, `"100.5"`) into
/// integer cents, rounded to the cent with banker's rounding.
pub fn parse_decimal_cents(amount: &str) -> Result<i64, CentsError> {
    let decimal = amount
        .trim()
        .parse::<Decimal>()
        .map_err(|_| CentsError::Invalid)?;
    let cents = (decimal.round_dp(2) * Decimal::from(100))
        .round_dp(0)
        .to_i64()
        .ok_or(CentsError::OutOfRange)?;
    if !(-MAX_SAFE_CENTS..=MAX_SAFE_CENTS).contains(&cents) {
        return Err(CentsError::OutOfRange);
    }
    Ok(cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_cents_and_simple_variants() {
        assert_eq!(parse_decimal_cents("100.50"), Ok(10_050));
        assert_eq!(parse_decimal_cents("100.5"), Ok(10_050));
        assert_eq!(parse_decimal_cents(".50"), Ok(50));
        assert_eq!(parse_decimal_cents("100"), Ok(10_000));
        assert_eq!(parse_decimal_cents("-100.5"), Ok(-10_050));
        assert_eq!(parse_decimal_cents(" 8.42 "), Ok(842));
    }

    #[test]
    fn sub_cent_midpoints_round_on_the_exact_decimal_value() {
        // The float path these replace rounded these wrong: 2.675 and 1.015
        // store *below* their decimal value in f64, so `(f * 100).round()`
        // dropped them to 267/101. Banker's rounding on the exact decimal
        // picks the even cent: 268 and 102.
        assert_eq!(parse_decimal_cents("2.675"), Ok(268));
        assert_eq!(parse_decimal_cents("1.015"), Ok(102));
        assert_eq!(parse_decimal_cents("-2.675"), Ok(-268));
    }

    #[test]
    fn near_cent_rounds_normally() {
        assert_eq!(parse_decimal_cents("100.999"), Ok(10_100));
    }

    #[test]
    fn display_safe_bounds() {
        assert_eq!(parse_decimal_cents("22517998136852.47"), Ok(MAX_SAFE_CENTS));
        assert_eq!(
            parse_decimal_cents("22517998136852.48"),
            Err(CentsError::OutOfRange)
        );
        assert_eq!(
            parse_decimal_cents("-22517998136852.48"),
            Err(CentsError::OutOfRange)
        );
    }

    #[test]
    fn unparseable_and_overflow_are_classified() {
        assert_eq!(parse_decimal_cents("abc"), Err(CentsError::Invalid));
        assert_eq!(parse_decimal_cents(""), Err(CentsError::Invalid));
        assert_eq!(
            parse_decimal_cents("99999999999999999999999.99"),
            Err(CentsError::OutOfRange)
        );
    }
}
