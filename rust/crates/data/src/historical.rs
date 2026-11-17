//! Offline normalization and validation for versioned historical records.

use factorsbt_contracts::{
    compare_utc_timestamps, HistoricalBar, HistoricalInstrumentMapping, InstrumentId, UtcTimestamp,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalDataError {
    DuplicateInstrument(String),
    ProviderSymbolCollision(String),
    UnknownInstrumentMapping(String),
    InvalidMapping(String),
    InvalidBar(String),
    DuplicateRevision(String),
}

impl fmt::Display for HistoricalDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInstrument(id) => write!(f, "instrument mapping duplicates {id}"),
            Self::ProviderSymbolCollision(symbol) => {
                write!(f, "provider symbol collision for {symbol}")
            }
            Self::UnknownInstrumentMapping(symbol) => {
                write!(f, "no instrument mapping for {symbol}")
            }
            Self::InvalidMapping(id) => write!(f, "instrument mapping {id} is invalid"),
            Self::InvalidBar(reason) => write!(f, "historical bar is invalid: {reason}"),
            Self::DuplicateRevision(identity) => {
                write!(f, "duplicate historical revision {identity}")
            }
        }
    }
}

impl Error for HistoricalDataError {}

/// Validate mappings and return provider/symbol lookups with collision checks.
pub fn validate_instrument_mappings(
    mappings: &[HistoricalInstrumentMapping],
) -> Result<BTreeMap<(String, String), HistoricalInstrumentMapping>, HistoricalDataError> {
    let mut by_provider_symbol = BTreeMap::new();
    let mut instrument_ids = HashMap::new();
    for mapping in mappings {
        let id = mapping.instrument_id.as_str();
        if mapping.provider_symbol.is_empty()
            || mapping.exchange_mic.len() != 4
            || !mapping
                .exchange_mic
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || mapping.currency.len() != 3
            || !mapping.currency.bytes().all(|b| b.is_ascii_uppercase())
        {
            return Err(HistoricalDataError::InvalidMapping(id.to_owned()));
        }
        if instrument_ids.insert(id.to_owned(), ()).is_some() {
            return Err(HistoricalDataError::DuplicateInstrument(id.to_owned()));
        }
        let key = (
            mapping.provider.as_str().to_owned(),
            mapping.provider_symbol.clone(),
        );
        if by_provider_symbol
            .insert(key.clone(), mapping.clone())
            .is_some()
        {
            return Err(HistoricalDataError::ProviderSymbolCollision(format!(
                "{}:{}",
                key.0, key.1
            )));
        }
    }
    Ok(by_provider_symbol)
}

/// Resolve an incoming provider symbol to the stable FactorsBT instrument ID.
pub fn normalize_instrument_id(
    provider: &str,
    provider_symbol: &str,
    mappings: &[HistoricalInstrumentMapping],
) -> Result<InstrumentId, HistoricalDataError> {
    let index = validate_instrument_mappings(mappings)?;
    index
        .get(&(provider.to_owned(), provider_symbol.to_owned()))
        .map(|mapping| mapping.instrument_id.clone())
        .ok_or_else(|| {
            HistoricalDataError::UnknownInstrumentMapping(format!("{provider}:{provider_symbol}"))
        })
}

/// Check canonical UTC minute intervals and historical-snapshot availability.
/// Session membership is validated against the versioned schedule separately.
pub fn validate_historical_bar_timestamps(bar: &HistoricalBar) -> Result<(), HistoricalDataError> {
    let start = parse_utc_minute(&bar.interval_start)?;
    if bar.interval_end != timestamp_from_minute(next_minute(start)) {
        return Err(HistoricalDataError::InvalidBar(
            "interval_end must be exactly one minute after interval_start".into(),
        ));
    }
    if compare_utc_timestamps(&bar.available_at, &bar.interval_end) != std::cmp::Ordering::Equal {
        return Err(HistoricalDataError::InvalidBar(
            "snapshot-mode available_at must equal interval_end".into(),
        ));
    }
    Ok(())
}

/// Validate OHLC ordering, positive prices, and nonnegative decimal volume.
pub fn validate_historical_bar_prices(bar: &HistoricalBar) -> Result<(), HistoricalDataError> {
    let parse = |field: &str, value: &str| {
        Decimal::from_str(value)
            .map_err(|_| HistoricalDataError::InvalidBar(format!("{field} is not a valid decimal")))
    };
    let open = parse("open", bar.open.as_str())?;
    let high = parse("high", bar.high.as_str())?;
    let low = parse("low", bar.low.as_str())?;
    let close = parse("close", bar.close.as_str())?;
    let volume = parse("volume", bar.volume.as_str())?;
    if open <= Decimal::ZERO
        || high <= Decimal::ZERO
        || low <= Decimal::ZERO
        || close <= Decimal::ZERO
    {
        return Err(HistoricalDataError::InvalidBar(
            "prices must be positive".into(),
        ));
    }
    if high < open.max(close) || low > open.min(close) || low > high {
        return Err(HistoricalDataError::InvalidBar(
            "OHLC values are not ordered".into(),
        ));
    }
    if volume < Decimal::ZERO {
        return Err(HistoricalDataError::InvalidBar(
            "volume must be nonnegative".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct UtcMinute {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
}

fn parse_utc_minute(value: &UtcTimestamp) -> Result<UtcMinute, HistoricalDataError> {
    let invalid =
        || HistoricalDataError::InvalidBar("timestamps must be minute-aligned UTC instants".into());
    let (date, time) = value.as_str().split_once('T').ok_or_else(invalid)?;
    let time = time.strip_suffix('Z').ok_or_else(invalid)?;
    let mut date_parts = date.split('-');
    let (Some(y), Some(m), Some(d), None) = (
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
        date_parts.next(),
    ) else {
        return Err(invalid());
    };
    let mut time_parts = time.split(':');
    let (Some(h), Some(minute), Some(second), None) = (
        time_parts.next(),
        time_parts.next(),
        time_parts.next(),
        time_parts.next(),
    ) else {
        return Err(invalid());
    };
    if second != "00" {
        return Err(invalid());
    }
    let parsed = UtcMinute {
        year: y.parse().map_err(|_| invalid())?,
        month: m.parse().map_err(|_| invalid())?,
        day: d.parse().map_err(|_| invalid())?,
        hour: h.parse().map_err(|_| invalid())?,
        minute: minute.parse().map_err(|_| invalid())?,
    };
    if !(1..=12).contains(&parsed.month)
        || !(1..=31).contains(&parsed.day)
        || parsed.hour > 23
        || parsed.minute > 59
    {
        return Err(invalid());
    }
    Ok(parsed)
}

fn next_minute(mut value: UtcMinute) -> UtcMinute {
    value.minute += 1;
    if value.minute == 60 {
        value.minute = 0;
        value.hour += 1;
    }
    if value.hour == 24 {
        value.hour = 0;
        value.day += 1;
    }
    let month_days = match value.month {
        2 if value.year % 400 == 0 || (value.year % 4 == 0 && value.year % 100 != 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if value.day > month_days {
        value.day = 1;
        value.month += 1;
    }
    if value.month > 12 {
        value.month = 1;
        value.year += 1;
    }
    value
}

fn timestamp_from_minute(value: UtcMinute) -> UtcTimestamp {
    UtcTimestamp::new(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:00Z",
        value.year, value.month, value.day, value.hour, value.minute
    ))
    .expect("generated UTC timestamp is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{Identifier, InstrumentId};

    fn mapping(provider_symbol: &str, instrument_id: &str) -> HistoricalInstrumentMapping {
        HistoricalInstrumentMapping {
            instrument_id: InstrumentId::new(instrument_id).unwrap(),
            provider: Identifier::new("provider-a").unwrap(),
            provider_symbol: provider_symbol.into(),
            exchange_mic: "XNAS".into(),
            currency: "USD".into(),
            mapping_version: Identifier::new("v1").unwrap(),
        }
    }

    #[test]
    fn maps_provider_symbol_to_stable_instrument_identity() {
        let mappings = vec![mapping("AAPL", "XNAS_AAPL")];
        assert_eq!(
            normalize_instrument_id("provider-a", "AAPL", &mappings)
                .unwrap()
                .as_str(),
            "XNAS_AAPL"
        );
    }

    #[test]
    fn rejects_instrument_and_provider_symbol_collisions() {
        assert!(matches!(
            validate_instrument_mappings(&[
                mapping("AAPL", "XNAS_AAPL"),
                mapping("MSFT", "XNAS_AAPL")
            ]),
            Err(HistoricalDataError::DuplicateInstrument(_))
        ));
        assert!(matches!(
            validate_instrument_mappings(&[
                mapping("AAPL", "XNAS_AAPL"),
                mapping("AAPL", "XNAS_MSFT")
            ]),
            Err(HistoricalDataError::ProviderSymbolCollision(_))
        ));
    }

    fn bar(start: &str, end: &str, available: &str) -> HistoricalBar {
        use factorsbt_contracts::DecimalString;
        HistoricalBar {
            instrument_id: InstrumentId::new("XNAS_AAPL").unwrap(),
            provider_symbol: "AAPL".into(),
            interval_start: UtcTimestamp::new(start).unwrap(),
            interval_end: UtcTimestamp::new(end).unwrap(),
            available_at: UtcTimestamp::new(available).unwrap(),
            open: DecimalString::new("10").unwrap(),
            high: DecimalString::new("11").unwrap(),
            low: DecimalString::new("9").unwrap(),
            close: DecimalString::new("10").unwrap(),
            volume: DecimalString::new("1").unwrap(),
            source_identity: Identifier::new("source:AAPL").unwrap(),
            source_revision: 0,
        }
    }

    #[test]
    fn validates_utc_minute_boundaries_across_hour_and_day() {
        assert!(validate_historical_bar_timestamps(&bar(
            "2026-03-08T23:59:00Z",
            "2026-03-09T00:00:00Z",
            "2026-03-09T00:00:00Z"
        ))
        .is_ok());
        assert!(validate_historical_bar_timestamps(&bar(
            "2026-03-08T14:30:00Z",
            "2026-03-08T14:31:00Z",
            "2026-03-08T14:31:00Z"
        ))
        .is_ok());
    }

    #[test]
    fn rejects_wrong_duration_and_availability_assumption() {
        assert!(validate_historical_bar_timestamps(&bar(
            "2026-03-08T14:30:00Z",
            "2026-03-08T14:32:00Z",
            "2026-03-08T14:32:00Z"
        ))
        .is_err());
        assert!(validate_historical_bar_timestamps(&bar(
            "2026-03-08T14:30:00Z",
            "2026-03-08T14:31:00Z",
            "2026-03-08T14:30:00Z"
        ))
        .is_err());
    }

    #[test]
    fn validates_ohlc_order_and_volume_without_float_conversion() {
        let mut valid = bar(
            "2026-03-08T14:30:00Z",
            "2026-03-08T14:31:00Z",
            "2026-03-08T14:31:00Z",
        );
        valid.open = factorsbt_contracts::DecimalString::new("10.000000000000000001").unwrap();
        assert!(validate_historical_bar_prices(&valid).is_ok());
        valid.high = factorsbt_contracts::DecimalString::new("9").unwrap();
        assert!(validate_historical_bar_prices(&valid).is_err());
        valid.high = factorsbt_contracts::DecimalString::new("11").unwrap();
        valid.volume = factorsbt_contracts::DecimalString::new("-1").unwrap();
        assert!(validate_historical_bar_prices(&valid).is_err());
    }
}
