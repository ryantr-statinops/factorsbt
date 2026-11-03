//! Shared primitives for the FactorsBT JSON wire contracts.
//!
//! These types check the lexical form of values. Cross-field rules such as
//! referenced instruments existing remain semantic validation responsibilities.

#![forbid(unsafe_code)]

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractValueError(&'static str);

impl fmt::Display for ContractValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for ContractValueError {}

macro_rules! string_contract_type {
    ($name:ident, $validator:ident, $message:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ContractValueError> {
                let value = value.into();
                if $validator(&value) {
                    Ok(Self(value))
                } else {
                    Err(ContractValueError($message))
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(D::Error::custom)
            }
        }
    };
}

string_contract_type!(Uuid, is_uuid, "expected a lowercase UUID string");
string_contract_type!(Identifier, is_identifier, "expected a contract identifier");
string_contract_type!(InstrumentId, is_instrument_id, "expected an instrument ID");
string_contract_type!(
    DecimalString,
    is_decimal,
    "expected a decimal string without exponent notation"
);
string_contract_type!(
    UtcTimestamp,
    is_utc_timestamp,
    "expected an RFC 3339 UTC timestamp ending in Z"
);

pub mod instruments;
pub mod model;
pub mod outcomes;
pub mod run_configuration;
pub mod temporal;
pub mod validation;

pub use instruments::{
    Currency, Instrument, LinearFutureInstrument, OptionInstrument, SpotInstrument,
};
pub use model::{ModelRequest, ModelResponse, OutputMode};
pub use outcomes::{EngineEvent, HealthResponse, RunResult, RunStatus};
pub use run_configuration::{RunConfiguration, SimulationConfiguration};
pub use temporal::{DecisionSnapshot, Observation, PortfolioSnapshot};

/// Compare normalized UTC RFC 3339 values, including fractional seconds.
pub fn compare_utc_timestamps(left: &UtcTimestamp, right: &UtcTimestamp) -> std::cmp::Ordering {
    let (left_base, left_fraction) = timestamp_components(left.as_str());
    let (right_base, right_fraction) = timestamp_components(right.as_str());
    match left_base.cmp(&right_base) {
        std::cmp::Ordering::Equal => compare_fractions(left_fraction, right_fraction),
        ordering => ordering,
    }
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

fn is_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(byte))
}

fn is_instrument_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_uppercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"_-".contains(byte))
}

fn is_decimal(value: &str) -> bool {
    if value.is_empty() || value.len() > 48 {
        return false;
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    if unsigned.is_empty() || (value.starts_with('-') && is_zero_value(unsigned)) {
        return false;
    }
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || (integer.len() > 1 && integer.starts_with('0'))
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    let lexical_valid = match fraction {
        Some(value) => {
            (1..=18).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => true,
    };
    lexical_valid && significant_digits(unsigned) <= 28
}

fn is_zero_value(unsigned: &str) -> bool {
    unsigned
        .bytes()
        .filter(|byte| *byte != b'.')
        .all(|byte| byte == b'0')
}

fn significant_digits(unsigned: &str) -> usize {
    let digits = unsigned
        .bytes()
        .filter(|byte| byte.is_ascii_digit())
        .collect::<Vec<_>>();
    digits
        .iter()
        .position(|byte| *byte != b'0')
        .map_or(1, |first_nonzero| digits.len() - first_nonzero)
}

fn is_utc_timestamp(value: &str) -> bool {
    let Some(body) = value.strip_suffix('Z') else {
        return false;
    };
    let Some((date, time)) = body.split_once('T') else {
        return false;
    };
    let mut date_parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) = (
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
    ) else {
        return false;
    };
    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || !month.bytes().all(|byte| byte.is_ascii_digit())
        || !day.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
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
        return false;
    };
    if ![hour, minute, second]
        .iter()
        .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit()))
        || !fraction
            .is_none_or(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
        year.parse::<u32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
        hour.parse::<u32>(),
        minute.parse::<u32>(),
        second.parse::<u32>(),
    ) else {
        return false;
    };
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => return false,
    };
    day >= 1 && day <= month_days && hour <= 23 && minute <= 59 && second <= 59
}

fn timestamp_components(value: &str) -> ((u32, u32, u32, u32, u32, u32), &str) {
    let body = value.strip_suffix('Z').expect("validated UTC timestamp");
    let (date, time) = body.split_once('T').expect("validated timestamp separator");
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    let mut date = date.split('-');
    let mut clock = clock.split(':');
    let base = (
        date.next()
            .expect("validated year")
            .parse()
            .expect("year digits"),
        date.next()
            .expect("validated month")
            .parse()
            .expect("month digits"),
        date.next()
            .expect("validated day")
            .parse()
            .expect("day digits"),
        clock
            .next()
            .expect("validated hour")
            .parse()
            .expect("hour digits"),
        clock
            .next()
            .expect("validated minute")
            .parse()
            .expect("minute digits"),
        clock
            .next()
            .expect("validated second")
            .parse()
            .expect("second digits"),
    );
    (base, fraction)
}

fn compare_fractions(left: &str, right: &str) -> std::cmp::Ordering {
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let left_digit = left.as_bytes().get(index).copied().unwrap_or(b'0');
        let right_digit = right.as_bytes().get(index).copied().unwrap_or(b'0');
        match left_digit.cmp(&right_digit) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_serialize_as_json_strings() {
        let id = InstrumentId::new("AAPL").unwrap();
        let price = DecimalString::new("12.3400").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"AAPL\"");
        assert_eq!(serde_json::to_string(&price).unwrap(), "\"12.3400\"");
    }

    #[test]
    fn decimal_wire_form_rejects_exponents_and_leading_zeroes() {
        for invalid in [
            "1e3",
            "01",
            ".5",
            "1.",
            "1.1234567890123456789",
            "-0",
            "-0.000",
            "12345678901234567890123456789",
        ] {
            assert!(DecimalString::new(invalid).is_err(), "accepted {invalid}");
        }
        assert!(DecimalString::new("1234567890123456789012345678").is_ok());
        assert!(DecimalString::new("0.000000000000000001").is_ok());
    }

    #[test]
    fn identifiers_and_timestamps_follow_wire_lexical_rules() {
        assert!(Uuid::new("123e4567-e89b-12d3-a456-426614174000").is_ok());
        assert!(Uuid::new("123E4567-e89b-12d3-a456-426614174000").is_err());
        assert!(Identifier::new("fixture:v1.0").is_ok());
        assert!(InstrumentId::new("aapl").is_err());
        assert!(UtcTimestamp::new("2026-01-02T09:00:00Z").is_ok());
        assert!(UtcTimestamp::new("2026-01-02T09:00:00+00:00").is_err());
        assert!(UtcTimestamp::new("2026-02-30T09:00:00Z").is_err());
        assert!(UtcTimestamp::new("2026-01-02T24:00:00Z").is_err());
    }

    #[test]
    fn utc_timestamp_order_compares_fractional_seconds_numerically() {
        let exact = UtcTimestamp::new("2026-01-02T09:00:00Z").unwrap();
        let tenth = UtcTimestamp::new("2026-01-02T09:00:00.1Z").unwrap();
        let tenth_extended = UtcTimestamp::new("2026-01-02T09:00:00.10Z").unwrap();
        assert_eq!(
            compare_utc_timestamps(&exact, &tenth),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_utc_timestamps(&tenth, &tenth_extended),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn deserialization_enforces_lexical_validation() {
        assert!(serde_json::from_str::<InstrumentId>("\"bad-id\"").is_err());
        assert!(serde_json::from_str::<DecimalString>("\"1e3\"").is_err());
    }
}
