//! Validated option-pricing inputs and intrinsic-value limits.

use factorsbt_contracts::{
    compare_utc_timestamps, instruments::OptionType as ContractOptionType, UtcTimestamp,
};
use rust_decimal::Decimal;
use std::{error::Error, fmt, str::FromStr};

const SECONDS_PER_ACT_365F_YEAR: f64 = 31_536_000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PricingError {
    InvalidUnderlying,
    InvalidStrike,
    InvalidRate,
    InvalidVolatility,
    InvalidTime,
    InvalidTimestamp,
    NonFiniteInput,
    ArithmeticOverflow,
}

impl fmt::Display for PricingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUnderlying => f.write_str("underlying price must be nonnegative"),
            Self::InvalidStrike => f.write_str("option strike must be positive"),
            Self::InvalidRate => f.write_str("annual risk-free rate must be in [-1, 1]"),
            Self::InvalidVolatility => f.write_str("annualized volatility must be in (0, 5]"),
            Self::InvalidTime => f.write_str("time to expiry must be finite and nonnegative"),
            Self::InvalidTimestamp => f.write_str("pricing time must be a valid UTC timestamp"),
            Self::NonFiniteInput => {
                f.write_str("pricing input is outside the finite numeric range")
            }
            Self::ArithmeticOverflow => f.write_str("intrinsic value arithmetic overflow"),
        }
    }
}

impl Error for PricingError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValidatedPricingInputs {
    pub underlying_price: f64,
    pub strike: f64,
    pub risk_free_rate: f64,
    pub annualized_volatility: f64,
    pub time_to_expiry_years: f64,
}

/// Compute ACT/365F years from UTC elapsed seconds. A valuation at or after
/// expiry returns zero so pricers can apply their exact intrinsic limit.
pub fn time_to_expiry_act_365f(
    valuation_time: &UtcTimestamp,
    expiry_time: &UtcTimestamp,
) -> Result<f64, PricingError> {
    if compare_utc_timestamps(valuation_time, expiry_time).is_ge() {
        return Ok(0.0);
    }
    let (valuation_seconds, valuation_fraction) = timestamp_seconds(valuation_time)?;
    let (expiry_seconds, expiry_fraction) = timestamp_seconds(expiry_time)?;
    let elapsed =
        (expiry_seconds - valuation_seconds) as f64 + expiry_fraction - valuation_fraction;
    if !elapsed.is_finite() || elapsed < 0.0 {
        return Err(PricingError::InvalidTime);
    }
    Ok(elapsed / SECONDS_PER_ACT_365F_YEAR)
}

/// Validate the numeric domain accepted for both theoretical pricers.
pub fn validate_pricing_inputs(
    underlying_price: Decimal,
    strike: Decimal,
    risk_free_rate: Decimal,
    annualized_volatility: Decimal,
    time_to_expiry_years: f64,
) -> Result<ValidatedPricingInputs, PricingError> {
    if underlying_price < Decimal::ZERO {
        return Err(PricingError::InvalidUnderlying);
    }
    if strike <= Decimal::ZERO {
        return Err(PricingError::InvalidStrike);
    }
    let underlying_price = to_f64(underlying_price)?;
    let strike = to_f64(strike)?;
    let risk_free_rate = to_f64(risk_free_rate)?;
    let annualized_volatility = to_f64(annualized_volatility)?;
    if !(-1.0..=1.0).contains(&risk_free_rate) {
        return Err(PricingError::InvalidRate);
    }
    if annualized_volatility <= 0.0 || annualized_volatility > 5.0 {
        return Err(PricingError::InvalidVolatility);
    }
    if !time_to_expiry_years.is_finite() || time_to_expiry_years < 0.0 {
        return Err(PricingError::InvalidTime);
    }
    Ok(ValidatedPricingInputs {
        underlying_price,
        strike,
        risk_free_rate,
        annualized_volatility,
        time_to_expiry_years,
    })
}

/// Return the per-unit intrinsic value for an option. Zero time requires no
/// logarithm, volatility division, or theoretical-pricer fallback.
pub fn intrinsic_value(
    option_type: ContractOptionType,
    underlying_price: Decimal,
    strike: Decimal,
) -> Result<Decimal, PricingError> {
    if underlying_price < Decimal::ZERO {
        return Err(PricingError::InvalidUnderlying);
    }
    if strike <= Decimal::ZERO {
        return Err(PricingError::InvalidStrike);
    }
    Ok(match option_type {
        ContractOptionType::Call => underlying_price
            .checked_sub(strike)
            .ok_or(PricingError::ArithmeticOverflow)?
            .max(Decimal::ZERO),
        ContractOptionType::Put => strike
            .checked_sub(underlying_price)
            .ok_or(PricingError::ArithmeticOverflow)?
            .max(Decimal::ZERO),
    })
}

fn to_f64(value: Decimal) -> Result<f64, PricingError> {
    let value = f64::from_str(&value.to_string()).map_err(|_| PricingError::NonFiniteInput)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PricingError::NonFiniteInput)
    }
}

fn timestamp_seconds(timestamp: &UtcTimestamp) -> Result<(i64, f64), PricingError> {
    let body = timestamp
        .as_str()
        .strip_suffix('Z')
        .ok_or(PricingError::InvalidTimestamp)?;
    let (date, time) = body.split_once('T').ok_or(PricingError::InvalidTimestamp)?;
    let mut date_parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) = (
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
    ) else {
        return Err(PricingError::InvalidTimestamp);
    };
    let year = year
        .parse::<i64>()
        .map_err(|_| PricingError::InvalidTimestamp)?;
    let month = month
        .parse::<i64>()
        .map_err(|_| PricingError::InvalidTimestamp)?;
    let day = day
        .parse::<i64>()
        .map_err(|_| PricingError::InvalidTimestamp)?;
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year, month)
    {
        return Err(PricingError::InvalidTimestamp);
    }

    let (clock, fraction) = match time.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (time, None),
    };
    let mut clock_parts = clock.split(':');
    let (Some(hour), Some(minute), Some(second), None) = (
        clock_parts.next(),
        clock_parts.next(),
        clock_parts.next(),
        clock_parts.next(),
    ) else {
        return Err(PricingError::InvalidTimestamp);
    };
    let hour = hour
        .parse::<i64>()
        .map_err(|_| PricingError::InvalidTimestamp)?;
    let minute = minute
        .parse::<i64>()
        .map_err(|_| PricingError::InvalidTimestamp)?;
    let second = second
        .parse::<i64>()
        .map_err(|_| PricingError::InvalidTimestamp)?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=59).contains(&second) {
        return Err(PricingError::InvalidTimestamp);
    }
    let fractional_second = match fraction {
        Some(digits) if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) => {
            f64::from_str(&format!("0.{digits}")).map_err(|_| PricingError::InvalidTimestamp)?
        }
        Some(_) => return Err(PricingError::InvalidTimestamp),
        None => 0.0,
    };
    let whole_seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
    Ok((whole_seconds, fractional_second))
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instant(value: &str) -> UtcTimestamp {
        UtcTimestamp::new(value).unwrap()
    }

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).unwrap()
    }

    #[test]
    fn intrinsic_value_handles_call_put_zero_time_and_at_the_money_limits() {
        assert_eq!(
            intrinsic_value(ContractOptionType::Call, decimal("110"), decimal("100")).unwrap(),
            decimal("10")
        );
        assert_eq!(
            intrinsic_value(ContractOptionType::Put, decimal("90"), decimal("100")).unwrap(),
            decimal("10")
        );
        assert_eq!(
            intrinsic_value(ContractOptionType::Call, decimal("100"), decimal("100")).unwrap(),
            Decimal::ZERO
        );
        let at_expiry = time_to_expiry_act_365f(
            &instant("2026-01-08T16:00:00Z"),
            &instant("2026-01-08T16:00:00Z"),
        )
        .unwrap();
        assert_eq!(at_expiry, 0.0);
    }

    #[test]
    fn act_365f_uses_elapsed_utc_seconds_including_leap_day_and_fractions() {
        let non_leap_year = time_to_expiry_act_365f(
            &instant("2026-01-01T00:00:00Z"),
            &instant("2027-01-01T00:00:00Z"),
        )
        .unwrap();
        assert!((non_leap_year - 1.0).abs() < f64::EPSILON);

        let leap_year = time_to_expiry_act_365f(
            &instant("2024-01-01T00:00:00Z"),
            &instant("2025-01-01T00:00:00Z"),
        )
        .unwrap();
        assert!((leap_year - (366.0 / 365.0)).abs() < 1e-12);

        let fractional = time_to_expiry_act_365f(
            &instant("2026-01-01T00:00:00.25Z"),
            &instant("2026-01-01T00:00:01Z"),
        )
        .unwrap();
        assert!((fractional - (0.75 / SECONDS_PER_ACT_365F_YEAR)).abs() < 1e-20);
    }

    #[test]
    fn theoretical_input_bounds_reject_invalid_rate_volatility_and_time() {
        let valid = validate_pricing_inputs(
            decimal("0"),
            decimal("100"),
            decimal("-1"),
            decimal("5"),
            0.0,
        )
        .unwrap();
        assert_eq!(valid.underlying_price, 0.0);
        assert_eq!(valid.time_to_expiry_years, 0.0);
        assert_eq!(
            validate_pricing_inputs(
                decimal("-0.01"),
                decimal("100"),
                decimal("0"),
                decimal("0.2"),
                1.0
            ),
            Err(PricingError::InvalidUnderlying)
        );
        assert_eq!(
            validate_pricing_inputs(
                decimal("100"),
                decimal("0"),
                decimal("0"),
                decimal("0.2"),
                1.0
            ),
            Err(PricingError::InvalidStrike)
        );
        assert_eq!(
            validate_pricing_inputs(
                decimal("100"),
                decimal("100"),
                decimal("1.01"),
                decimal("0.2"),
                1.0
            ),
            Err(PricingError::InvalidRate)
        );
        assert_eq!(
            validate_pricing_inputs(
                decimal("100"),
                decimal("100"),
                decimal("0"),
                decimal("0"),
                1.0
            ),
            Err(PricingError::InvalidVolatility)
        );
        assert_eq!(
            validate_pricing_inputs(
                decimal("100"),
                decimal("100"),
                decimal("0"),
                decimal("0.2"),
                f64::INFINITY
            ),
            Err(PricingError::InvalidTime)
        );
    }

    #[test]
    fn valuation_after_expiry_has_zero_time_instead_of_negative_time() {
        let time = time_to_expiry_act_365f(
            &instant("2026-01-09T16:00:00Z"),
            &instant("2026-01-08T16:00:00Z"),
        )
        .unwrap();
        assert_eq!(time, 0.0);
    }
}
