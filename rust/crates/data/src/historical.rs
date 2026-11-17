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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataQualityCode {
    MissingInterval,
    DuplicateInterval,
    RejectedTimestamp,
    RejectedPrice,
    OutsideSession,
    SourceRequestIncomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCoverageStatus {
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntradayCoverageResult {
    pub diagnostics: Vec<DataQualityDiagnostic>,
    pub all_final_closes_available: bool,
}

/// Safe quality signal. It intentionally omits raw provider responses and credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataQualityDiagnostic {
    pub code: DataQualityCode,
    pub instrument_id: Option<String>,
    pub interval_start: Option<String>,
    pub row_index: Option<usize>,
}

pub fn diagnose_historical_bars(
    expected: &[(InstrumentId, UtcTimestamp)],
    records: &[HistoricalBar],
) -> Vec<DataQualityDiagnostic> {
    use std::collections::BTreeSet;
    let expected_keys = expected
        .iter()
        .map(|(id, start)| (id.as_str().to_owned(), start.as_str().to_owned()))
        .collect::<BTreeSet<_>>();
    let mut seen = HashMap::<(String, String), usize>::new();
    let mut diagnostics = Vec::new();
    for (row_index, record) in records.iter().enumerate() {
        let key = (
            record.instrument_id.as_str().to_owned(),
            record.interval_start.as_str().to_owned(),
        );
        if seen.insert(key.clone(), row_index).is_some() {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::DuplicateInterval,
                instrument_id: Some(key.0),
                interval_start: Some(key.1),
                row_index: Some(row_index),
            });
            continue;
        }
        if validate_historical_bar_timestamps(record).is_err() {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::RejectedTimestamp,
                instrument_id: Some(key.0),
                interval_start: Some(key.1),
                row_index: Some(row_index),
            });
        } else if validate_historical_bar_prices(record).is_err() {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::RejectedPrice,
                instrument_id: Some(key.0),
                interval_start: Some(key.1),
                row_index: Some(row_index),
            });
        }
    }
    for (instrument_id, interval_start) in expected_keys {
        if !seen.contains_key(&(instrument_id.clone(), interval_start.clone())) {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::MissingInterval,
                instrument_id: Some(instrument_id),
                interval_start: Some(interval_start),
                row_index: None,
            });
        }
    }
    diagnostics
}

/// Validate one frozen session without synthesizing missing minute bars.
/// Incomplete source retrieval is reported separately and is not mislabeled as
/// a collection of no-trade intervals.
pub fn check_session_coverage(
    session: &factorsbt_contracts::SessionScheduleEntry,
    instruments: &[InstrumentId],
    records: &[HistoricalBar],
    source_status: SourceCoverageStatus,
) -> Result<IntradayCoverageResult, HistoricalDataError> {
    if source_status == SourceCoverageStatus::Incomplete {
        return Ok(IntradayCoverageResult {
            diagnostics: vec![DataQualityDiagnostic {
                code: DataQualityCode::SourceRequestIncomplete,
                instrument_id: None,
                interval_start: None,
                row_index: None,
            }],
            all_final_closes_available: false,
        });
    }
    let open = parse_utc_minute(&session.open_at)?;
    let close = parse_utc_minute(&session.close_at)?;
    let mut starts = Vec::new();
    let mut cursor = open;
    while timestamp_from_minute(cursor) != session.close_at {
        starts.push(timestamp_from_minute(cursor));
        cursor = next_minute(cursor);
        if starts.len() > 390 {
            return Err(HistoricalDataError::InvalidBar(
                "session exceeds 390 one-minute intervals".into(),
            ));
        }
    }
    if starts.is_empty() || timestamp_from_minute(close) != session.close_at {
        return Err(HistoricalDataError::InvalidBar(
            "session schedule is not minute aligned".into(),
        ));
    }

    let mut diagnostics = Vec::new();
    let mut seen = HashMap::<(String, String), usize>::new();
    let mut usable = std::collections::HashSet::<(String, String)>::new();
    let instrument_set = instruments
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect::<std::collections::HashSet<_>>();
    for (row_index, record) in records.iter().enumerate() {
        let id = record.instrument_id.as_str().to_owned();
        let start = record.interval_start.as_str().to_owned();
        if !instrument_set.contains(&id)
            || compare_utc_timestamps(&record.interval_start, &session.open_at).is_lt()
            || compare_utc_timestamps(&record.interval_end, &session.close_at).is_gt()
        {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::OutsideSession,
                instrument_id: Some(id),
                interval_start: Some(start),
                row_index: Some(row_index),
            });
            continue;
        }
        if seen
            .insert((id.clone(), start.clone()), row_index)
            .is_some()
        {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::DuplicateInterval,
                instrument_id: Some(id),
                interval_start: Some(start),
                row_index: Some(row_index),
            });
            continue;
        }
        if validate_historical_bar_timestamps(record).is_err() {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::RejectedTimestamp,
                instrument_id: Some(id),
                interval_start: Some(start),
                row_index: Some(row_index),
            });
        } else if validate_historical_bar_prices(record).is_err() {
            diagnostics.push(DataQualityDiagnostic {
                code: DataQualityCode::RejectedPrice,
                instrument_id: Some(id),
                interval_start: Some(start),
                row_index: Some(row_index),
            });
        } else {
            usable.insert((id, start));
        }
    }
    let mut final_close_by_instrument = HashMap::<String, bool>::new();
    for instrument in instruments {
        let id = instrument.as_str().to_owned();
        for start in &starts {
            if !usable.contains(&(id.clone(), start.as_str().to_owned())) {
                diagnostics.push(DataQualityDiagnostic {
                    code: DataQualityCode::MissingInterval,
                    instrument_id: Some(id.clone()),
                    interval_start: Some(start.as_str().to_owned()),
                    row_index: None,
                });
            }
        }
        final_close_by_instrument.insert(
            id.clone(),
            usable.contains(&(id, starts.last().unwrap().as_str().to_owned())),
        );
    }
    Ok(IntradayCoverageResult {
        all_final_closes_available: !instruments.is_empty()
            && final_close_by_instrument
                .values()
                .all(|available| *available),
        diagnostics,
    })
}

pub fn select_schedule_sessions<'a>(
    sessions: &'a [factorsbt_contracts::SessionScheduleEntry],
    start_date: &str,
    end_date: &str,
) -> Result<Vec<&'a factorsbt_contracts::SessionScheduleEntry>, HistoricalDataError> {
    if start_date > end_date {
        return Err(HistoricalDataError::InvalidBar(
            "run start date follows end date".into(),
        ));
    }
    let selected = sessions
        .iter()
        .filter(|session| {
            session.session_date.as_str() >= start_date && session.session_date.as_str() <= end_date
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(HistoricalDataError::InvalidBar(
            "run interval contains no scheduled sessions".into(),
        ));
    }
    Ok(selected)
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

/// Normalize a provider Unix-millisecond aggregate start to a minute-aligned UTC instant.
pub fn normalize_unix_millis_start(timestamp_ms: i64) -> Result<UtcTimestamp, HistoricalDataError> {
    if timestamp_ms < 0 || timestamp_ms % 60_000 != 0 {
        return Err(HistoricalDataError::InvalidBar(
            "provider timestamp must be a nonnegative minute-aligned Unix millisecond value".into(),
        ));
    }
    let total_minutes = timestamp_ms / 60_000;
    let (year, month, day) = civil_from_days(total_minutes.div_euclid(1440));
    let minute_of_day = total_minutes.rem_euclid(1440);
    UtcTimestamp::new(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:00Z",
        minute_of_day / 60,
        minute_of_day % 60
    ))
    .map_err(|_| {
        HistoricalDataError::InvalidBar("provider timestamp is outside supported UTC range".into())
    })
}

fn civil_from_days(day_number: i64) -> (i64, i64, i64) {
    let shifted = day_number + 719468;
    let era = shifted.div_euclid(146097);
    let day_of_era = shifted - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
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

/// Select the greatest eligible revision for each source identity at `as_of`.
/// The input slice is never mutated, preserving the sealed snapshot.
pub fn select_historical_revisions_as_of(
    records: &[HistoricalBar],
    as_of: &UtcTimestamp,
) -> Result<Vec<HistoricalBar>, HistoricalDataError> {
    let mut selected = BTreeMap::<String, HistoricalBar>::new();
    for record in records {
        if compare_utc_timestamps(&record.available_at, as_of).is_gt() {
            continue;
        }
        let identity = record.source_identity.as_str().to_owned();
        match selected.get(&identity) {
            Some(current) if current.source_revision == record.source_revision => {
                return Err(HistoricalDataError::DuplicateRevision(identity));
            }
            Some(current) if current.source_revision > record.source_revision => {}
            _ => {
                selected.insert(identity, record.clone());
            }
        }
    }
    let mut values = selected.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
            .then_with(|| compare_utc_timestamps(&left.interval_start, &right.interval_start))
    });
    Ok(values)
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

    fn session() -> factorsbt_contracts::SessionScheduleEntry {
        factorsbt_contracts::SessionScheduleEntry {
            session_date: "2026-03-09".into(),
            open_at: UtcTimestamp::new("2026-03-09T13:30:00Z").unwrap(),
            close_at: UtcTimestamp::new("2026-03-09T20:00:00Z").unwrap(),
            session_type: factorsbt_contracts::SessionType::Regular,
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
    fn normalizes_provider_unix_milliseconds_to_utc_and_rejects_unaligned_values() {
        assert_eq!(
            normalize_unix_millis_start(1_773_063_000_000)
                .unwrap()
                .as_str(),
            "2026-03-09T13:30:00Z"
        );
        assert_eq!(
            normalize_unix_millis_start(1_793_629_800_000)
                .unwrap()
                .as_str(),
            "2026-11-02T14:30:00Z"
        );
        assert!(normalize_unix_millis_start(1_773_063_000_001).is_err());
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

    #[test]
    fn selects_latest_available_revision_without_mutating_source() {
        let mut first = bar(
            "2026-03-08T14:30:00Z",
            "2026-03-08T14:31:00Z",
            "2026-03-08T14:31:00Z",
        );
        let mut revised = first.clone();
        first.source_revision = 1;
        first.close = factorsbt_contracts::DecimalString::new("10").unwrap();
        revised.source_revision = 2;
        revised.available_at = UtcTimestamp::new("2026-03-09T00:00:00Z").unwrap();
        revised.close = factorsbt_contracts::DecimalString::new("12").unwrap();
        let before = select_historical_revisions_as_of(
            &[first.clone(), revised.clone()],
            &UtcTimestamp::new("2026-03-08T16:00:00Z").unwrap(),
        )
        .unwrap();
        let after = select_historical_revisions_as_of(
            &[first, revised],
            &UtcTimestamp::new("2026-03-09T00:00:00Z").unwrap(),
        )
        .unwrap();
        assert_eq!(before[0].source_revision, 1);
        assert_eq!(after[0].source_revision, 2);
        assert_eq!(after[0].close.as_str(), "12");
    }

    #[test]
    fn diagnostics_report_gaps_duplicates_and_invalid_rows_without_payloads() {
        let first = bar(
            "2026-03-08T14:30:00Z",
            "2026-03-08T14:31:00Z",
            "2026-03-08T14:31:00Z",
        );
        let mut invalid = bar(
            "2026-03-08T14:32:00Z",
            "2026-03-08T14:33:00Z",
            "2026-03-08T14:33:00Z",
        );
        invalid.low = factorsbt_contracts::DecimalString::new("12").unwrap();
        let expected = vec![
            (first.instrument_id.clone(), first.interval_start.clone()),
            (
                first.instrument_id.clone(),
                UtcTimestamp::new("2026-03-08T14:31:00Z").unwrap(),
            ),
            (
                first.instrument_id.clone(),
                UtcTimestamp::new("2026-03-08T14:32:00Z").unwrap(),
            ),
        ];
        let diagnostics = diagnose_historical_bars(&expected, &[first.clone(), first, invalid]);
        assert!(diagnostics
            .iter()
            .any(|d| d.code == DataQualityCode::DuplicateInterval));
        assert!(diagnostics
            .iter()
            .any(|d| d.code == DataQualityCode::MissingInterval));
        assert!(diagnostics
            .iter()
            .any(|d| d.code == DataQualityCode::RejectedPrice));
        assert!(diagnostics
            .iter()
            .all(|d| d.row_index.is_none_or(|row| row < 3)));
    }

    #[test]
    fn session_coverage_keeps_absent_bars_distinct_from_incomplete_source_requests() {
        let first = bar(
            "2026-03-09T13:30:00Z",
            "2026-03-09T13:31:00Z",
            "2026-03-09T13:31:00Z",
        );
        let final_bar = bar(
            "2026-03-09T19:59:00Z",
            "2026-03-09T20:00:00Z",
            "2026-03-09T20:00:00Z",
        );
        let instrument = first.instrument_id.clone();
        let complete = check_session_coverage(
            &session(),
            std::slice::from_ref(&instrument),
            &[first.clone(), final_bar],
            SourceCoverageStatus::Complete,
        )
        .unwrap();
        assert!(complete.all_final_closes_available);
        assert_eq!(
            complete
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == DataQualityCode::MissingInterval)
                .count(),
            388
        );

        let incomplete = check_session_coverage(
            &session(),
            &[instrument],
            &[first],
            SourceCoverageStatus::Incomplete,
        )
        .unwrap();
        assert!(!incomplete.all_final_closes_available);
        assert_eq!(incomplete.diagnostics.len(), 1);
        assert_eq!(
            incomplete.diagnostics[0].code,
            DataQualityCode::SourceRequestIncomplete
        );
    }

    #[test]
    fn schedule_run_bounds_select_only_frozen_sessions() {
        let mut second = session();
        second.session_date = "2026-03-10".into();
        let sessions = vec![session(), second];
        assert_eq!(
            select_schedule_sessions(&sessions, "2026-03-10", "2026-03-10")
                .unwrap()
                .len(),
            1
        );
        assert!(select_schedule_sessions(&sessions, "2026-03-11", "2026-03-12").is_err());
        assert!(select_schedule_sessions(&sessions, "2026-03-12", "2026-03-11").is_err());
    }
}
