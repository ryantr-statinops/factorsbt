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
pub mod validation;

pub use instruments::{
    Currency, Instrument, LinearFutureInstrument, OptionInstrument, SpotInstrument,
};
pub use model::{ModelRequest, ModelResponse, OutputMode};
pub use outcomes::{EngineEvent, HealthResponse, RunResult, RunStatus};

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
    if unsigned.is_empty() {
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
    match fraction {
        Some(value) => {
            (1..=18).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => true,
    }
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
    [hour, minute, second]
        .iter()
        .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit()))
        && fraction
            .is_none_or(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
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
        for invalid in ["1e3", "01", ".5", "1.", "1.1234567890123456789"] {
            assert!(DecimalString::new(invalid).is_err(), "accepted {invalid}");
        }
        assert!(DecimalString::new("-0").is_ok());
    }

    #[test]
    fn identifiers_and_timestamps_follow_wire_lexical_rules() {
        assert!(Uuid::new("123e4567-e89b-12d3-a456-426614174000").is_ok());
        assert!(Uuid::new("123E4567-e89b-12d3-a456-426614174000").is_err());
        assert!(Identifier::new("fixture:v1.0").is_ok());
        assert!(InstrumentId::new("aapl").is_err());
        assert!(UtcTimestamp::new("2026-01-02T09:00:00Z").is_ok());
        assert!(UtcTimestamp::new("2026-01-02T09:00:00+00:00").is_err());
    }

    #[test]
    fn deserialization_enforces_lexical_validation() {
        assert!(serde_json::from_str::<InstrumentId>("\"bad-id\"").is_err());
        assert!(serde_json::from_str::<DecimalString>("\"1e3\"").is_err());
    }
}
