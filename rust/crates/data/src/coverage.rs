//! Required input coverage and freshness checks for a configured synthetic run.

use crate::{
    execution_prices_available_at, initial_marks_available_at, observations_available_at,
    FixtureDataset,
};
use factorsbt_contracts::{
    compare_utc_timestamps,
    instruments::{ExerciseStyle, Instrument, SettlementStyle, ValuationMode},
    run_configuration::RunConfiguration,
    temporal::{Observation, ObservationKind, ObservationPayload},
    InstrumentId, UtcTimestamp,
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageIssueCode {
    InvalidRunInterval,
    InvalidFixtureIdentity,
    InvalidInstrumentConfiguration,
    DuplicateInstrument,
    MissingUnderlying,
    MissingInitialMark,
    InitialMarkUnavailable,
    MissingMarketObservation,
    StaleMarketObservation,
    MarketObservationUnavailable,
    MissingPricingInput,
    StalePricingInput,
    PricingInputUnavailable,
    MissingExecutionPrice,
    StaleExecutionPrice,
    ExecutionPriceUnavailable,
    AmbiguousInput,
    InvalidObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageIssue {
    pub code: CoverageIssueCode,
    pub instrument_id: Option<String>,
    pub at: Option<String>,
    pub message: String,
}

#[derive(Clone, Copy)]
struct ObservationRequirement {
    kind: ObservationKind,
    missing: CoverageIssueCode,
    stale: CoverageIssueCode,
    unavailable: CoverageIssueCode,
}

fn bar_requirement() -> ObservationRequirement {
    ObservationRequirement {
        kind: ObservationKind::Bar,
        missing: CoverageIssueCode::MissingMarketObservation,
        stale: CoverageIssueCode::StaleMarketObservation,
        unavailable: CoverageIssueCode::MarketObservationUnavailable,
    }
}

fn issue(
    code: CoverageIssueCode,
    instrument_id: Option<&str>,
    at: Option<&UtcTimestamp>,
    message: impl Into<String>,
) -> CoverageIssue {
    CoverageIssue {
        code,
        instrument_id: instrument_id.map(str::to_owned),
        at: at.map(|value| value.as_str().to_owned()),
        message: message.into(),
    }
}

/// Check that every instrument has a usable initial mark, a close mark at each
/// decision boundary, and an execution open for every eligible session.
/// Missing news is allowed; missing price or pricing input is not carried forward.
pub fn validate_run_coverage(
    dataset: &FixtureDataset,
    configuration: &RunConfiguration,
) -> Result<(), Vec<CoverageIssue>> {
    let mut issues = Vec::new();
    if dataset.fixture_id != configuration.data.fixture_id
        || dataset.fixture_version != configuration.data.fixture_version
    {
        issues.push(issue(
            CoverageIssueCode::InvalidFixtureIdentity,
            None,
            None,
            "run configuration does not reference this fixture ID and version",
        ));
    }
    let (closes, opens) = match session_times(
        &configuration.simulation.start_at,
        &configuration.simulation.end_at,
    ) {
        Ok(times) => times,
        Err(message) => {
            issues.push(issue(
                CoverageIssueCode::InvalidRunInterval,
                None,
                None,
                message,
            ));
            return Err(issues);
        }
    };

    let mut instruments = HashMap::new();
    for instrument in &configuration.instruments {
        let id = instrument_id(instrument);
        if instruments.insert(id.to_owned(), instrument).is_some() {
            issues.push(issue(
                CoverageIssueCode::DuplicateInstrument,
                Some(id),
                None,
                "instrument ID occurs more than once in the run configuration",
            ));
        }
    }
    if configuration.instruments.is_empty() {
        issues.push(issue(
            CoverageIssueCode::InvalidInstrumentConfiguration,
            None,
            None,
            "at least one instrument is required",
        ));
    }
    for instrument in &configuration.instruments {
        match instrument {
            Instrument::Spot(_) => {}
            Instrument::LinearFuture(value) => {
                if !instruments.contains_key(value.underlying_id.as_str()) {
                    issues.push(issue(
                        CoverageIssueCode::MissingUnderlying,
                        Some(value.instrument_id.as_str()),
                        None,
                        "future underlying is not in the run instrument universe",
                    ));
                }
            }
            Instrument::Option(value) => {
                match instruments.get(value.underlying_id.as_str()) {
                    Some(Instrument::Spot(_)) => {}
                    Some(_) => issues.push(issue(
                        CoverageIssueCode::InvalidInstrumentConfiguration,
                        Some(value.instrument_id.as_str()),
                        None,
                        "option underlying must be a configured spot instrument",
                    )),
                    None => issues.push(issue(
                        CoverageIssueCode::MissingUnderlying,
                        Some(value.instrument_id.as_str()),
                        None,
                        "option underlying is not in the run instrument universe",
                    )),
                }
                if (value.exercise_style == ExerciseStyle::European
                    && value.settlement_style != SettlementStyle::Cash)
                    || (value.exercise_style == ExerciseStyle::American
                        && value.settlement_style != SettlementStyle::Physical)
                {
                    issues.push(issue(
                        CoverageIssueCode::InvalidInstrumentConfiguration,
                        Some(value.instrument_id.as_str()),
                        None,
                        "option exercise and settlement styles violate the MVP convention",
                    ));
                }
                if (value.valuation_mode == ValuationMode::Theoretical
                    && value.valuation_inputs.is_none())
                    || (value.valuation_mode == ValuationMode::Quote
                        && value.valuation_inputs.is_some())
                {
                    issues.push(issue(
                        CoverageIssueCode::InvalidInstrumentConfiguration,
                        Some(value.instrument_id.as_str()),
                        None,
                        "option valuation mode and pricer configuration do not match",
                    ));
                }
            }
        }
    }

    let initial_marks =
        match initial_marks_available_at(dataset, &configuration.simulation.start_at) {
            Ok(marks) => marks,
            Err(error) => {
                issues.push(issue(
                    CoverageIssueCode::AmbiguousInput,
                    None,
                    Some(&configuration.simulation.start_at),
                    error.to_string(),
                ));
                Vec::new()
            }
        };
    for instrument in &configuration.instruments {
        let id = instrument_id(instrument);
        if initial_marks
            .iter()
            .any(|mark| mark.instrument_id.as_str() == id)
        {
            continue;
        }
        let late = dataset.initial_marks.iter().any(|mark| {
            mark.instrument_id.as_str() == id
                && compare_utc_timestamps(&mark.available_at, &configuration.simulation.start_at)
                    == Ordering::Greater
        });
        issues.push(issue(
            if late {
                CoverageIssueCode::InitialMarkUnavailable
            } else {
                CoverageIssueCode::MissingInitialMark
            },
            Some(id),
            Some(&configuration.simulation.start_at),
            if late {
                "initial mark exists but is unavailable at simulation start"
            } else {
                "fixture has no initial mark for this instrument"
            },
        ));
    }

    let mut checked_observations = HashSet::new();
    for close in &closes {
        let visible = match observations_available_at(dataset, close) {
            Ok(observations) => observations,
            Err(error) => {
                issues.push(issue(
                    CoverageIssueCode::AmbiguousInput,
                    None,
                    Some(close),
                    error.to_string(),
                ));
                Vec::new()
            }
        };
        for instrument in &configuration.instruments {
            if expired_after(instrument, close) {
                continue;
            }
            match instrument {
                Instrument::Spot(value) => check_observation(
                    dataset,
                    &visible,
                    &value.instrument_id,
                    close,
                    bar_requirement(),
                    &mut issues,
                    &mut checked_observations,
                ),
                Instrument::LinearFuture(value) => check_observation(
                    dataset,
                    &visible,
                    &value.instrument_id,
                    close,
                    bar_requirement(),
                    &mut issues,
                    &mut checked_observations,
                ),
                Instrument::Option(value) => {
                    let requirement = if value.valuation_mode == ValuationMode::Quote {
                        ObservationRequirement {
                            kind: ObservationKind::Quote,
                            missing: CoverageIssueCode::MissingMarketObservation,
                            stale: CoverageIssueCode::StaleMarketObservation,
                            unavailable: CoverageIssueCode::MarketObservationUnavailable,
                        }
                    } else {
                        ObservationRequirement {
                            kind: ObservationKind::OptionPricingInput,
                            missing: CoverageIssueCode::MissingPricingInput,
                            stale: CoverageIssueCode::StalePricingInput,
                            unavailable: CoverageIssueCode::PricingInputUnavailable,
                        }
                    };
                    check_observation(
                        dataset,
                        &visible,
                        &value.instrument_id,
                        close,
                        requirement,
                        &mut issues,
                        &mut checked_observations,
                    );
                    if value.valuation_mode == ValuationMode::Theoretical
                        && !visible.iter().any(|observation| {
                            observation.instrument_id.as_ref() == Some(&value.underlying_id)
                                && observation.kind == ObservationKind::Bar
                                && compare_utc_timestamps(&observation.event_time, close)
                                    == Ordering::Equal
                        })
                    {
                        check_observation(
                            dataset,
                            &visible,
                            &value.underlying_id,
                            close,
                            bar_requirement(),
                            &mut issues,
                            &mut checked_observations,
                        );
                    }
                }
            }
        }
    }

    for open in &opens {
        let available = match execution_prices_available_at(dataset, open) {
            Ok(prices) => prices,
            Err(error) => {
                issues.push(issue(
                    CoverageIssueCode::AmbiguousInput,
                    None,
                    Some(open),
                    error.to_string(),
                ));
                Vec::new()
            }
        };
        for instrument in &configuration.instruments {
            if expired_after(instrument, open) {
                continue;
            }
            let id = instrument_id(instrument);
            if available.iter().any(|price| {
                price.instrument_id.as_str() == id
                    && compare_utc_timestamps(&price.session_open, open) == Ordering::Equal
            }) {
                continue;
            }
            let matching = dataset
                .execution_prices
                .iter()
                .filter(|price| {
                    price.instrument_id.as_str() == id
                        && compare_utc_timestamps(&price.session_open, open) == Ordering::Equal
                })
                .collect::<Vec<_>>();
            let late = matching.iter().any(|price| {
                compare_utc_timestamps(&price.available_at, open) == Ordering::Greater
            });
            let stale = dataset.execution_prices.iter().any(|price| {
                price.instrument_id.as_str() == id
                    && compare_utc_timestamps(&price.session_open, open) == Ordering::Less
            });
            issues.push(issue(
                if late {
                    CoverageIssueCode::ExecutionPriceUnavailable
                } else if stale {
                    CoverageIssueCode::StaleExecutionPrice
                } else {
                    CoverageIssueCode::MissingExecutionPrice
                },
                Some(id),
                Some(open),
                if late {
                    "execution price is not available at the session open"
                } else if stale {
                    "only an earlier session execution price is available"
                } else {
                    "fixture has no execution price for this instrument and open"
                },
            ));
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn check_observation(
    dataset: &FixtureDataset,
    visible: &[Observation],
    instrument_id: &InstrumentId,
    close: &UtcTimestamp,
    requirement: ObservationRequirement,
    issues: &mut Vec<CoverageIssue>,
    checked: &mut HashSet<String>,
) {
    let candidates = visible
        .iter()
        .filter(|observation| {
            observation.instrument_id.as_ref() == Some(instrument_id)
                && observation.kind == requirement.kind
                && compare_utc_timestamps(&observation.event_time, close) == Ordering::Equal
        })
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        let observation = candidates[0];
        if checked.insert(observation.observation_id.as_str().to_owned()) {
            if let Err(detail) = validate_payload(observation) {
                issues.push(issue(
                    CoverageIssueCode::InvalidObservation,
                    Some(instrument_id.as_str()),
                    Some(close),
                    detail,
                ));
            }
        }
        return;
    }
    if candidates.len() > 1 {
        issues.push(issue(
            CoverageIssueCode::AmbiguousInput,
            Some(instrument_id.as_str()),
            Some(close),
            "more than one source observation matches this instrument and event time",
        ));
        return;
    }

    let exact = dataset.observations.iter().any(|observation| {
        observation.instrument_id.as_ref() == Some(instrument_id)
            && observation.kind == requirement.kind
            && compare_utc_timestamps(&observation.event_time, close) == Ordering::Equal
    });
    let stale = dataset.observations.iter().any(|observation| {
        observation.instrument_id.as_ref() == Some(instrument_id)
            && observation.kind == requirement.kind
            && compare_utc_timestamps(&observation.event_time, close) == Ordering::Less
            && compare_utc_timestamps(&observation.available_at, close) != Ordering::Greater
    });
    let (code, detail) = if exact {
        (
            requirement.unavailable,
            "matching observation exists but is not available at the decision time",
        )
    } else if stale {
        (
            requirement.stale,
            "only an earlier event-time observation is available; implicit carry-forward is disabled",
        )
    } else {
        (
            requirement.missing,
            "required observation is missing for this instrument and decision time",
        )
    };
    issues.push(issue(
        code,
        Some(instrument_id.as_str()),
        Some(close),
        detail,
    ));
}

fn validate_payload(observation: &Observation) -> Result<(), &'static str> {
    match (&observation.kind, &observation.payload) {
        (ObservationKind::Bar, ObservationPayload::Bar(bar)) => {
            if compare_utc_timestamps(&bar.interval_start, &bar.interval_end) != Ordering::Less
                || compare_utc_timestamps(&observation.event_time, &bar.interval_end)
                    != Ordering::Equal
                || decimal_cmp(bar.high.as_str(), bar.low.as_str()) == Ordering::Less
                || decimal_cmp(bar.open.as_str(), bar.high.as_str()) == Ordering::Greater
                || decimal_cmp(bar.close.as_str(), bar.high.as_str()) == Ordering::Greater
                || decimal_cmp(bar.open.as_str(), bar.low.as_str()) == Ordering::Less
                || decimal_cmp(bar.close.as_str(), bar.low.as_str()) == Ordering::Less
            {
                Err("bar event time or OHLC values are inconsistent")
            } else {
                Ok(())
            }
        }
        (ObservationKind::Quote, ObservationPayload::Quote(quote)) => {
            if decimal_cmp(quote.bid.as_str(), quote.ask.as_str()) == Ordering::Greater {
                Err("quote bid exceeds ask")
            } else {
                Ok(())
            }
        }
        (ObservationKind::OptionPricingInput, ObservationPayload::OptionPricingInput(input)) => {
            if decimal_cmp(input.risk_free_rate.as_str(), "-1") == Ordering::Less
                || decimal_cmp(input.risk_free_rate.as_str(), "1") == Ordering::Greater
                || decimal_cmp(input.annualized_volatility.as_str(), "0") != Ordering::Greater
                || decimal_cmp(input.annualized_volatility.as_str(), "5") == Ordering::Greater
            {
                Err("option pricing rate or volatility is outside the accepted range")
            } else {
                Ok(())
            }
        }
        (ObservationKind::News, ObservationPayload::News(_)) => Ok(()),
        _ => Err("observation kind does not match its payload shape"),
    }
}

fn decimal_cmp(left: &str, right: &str) -> Ordering {
    fn parts(value: &str) -> (bool, String, String) {
        let negative = value.starts_with('-');
        let unsigned = value.strip_prefix('-').unwrap_or(value);
        let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        let integer = integer.trim_start_matches('0');
        let integer = if integer.is_empty() { "0" } else { integer };
        (
            negative,
            integer.to_owned(),
            fraction.trim_end_matches('0').to_owned(),
        )
    }

    let (left_negative, left_integer, left_fraction) = parts(left);
    let (right_negative, right_integer, right_fraction) = parts(right);
    if left_negative != right_negative {
        return if left_negative {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let integer_order = left_integer
        .len()
        .cmp(&right_integer.len())
        .then_with(|| left_integer.cmp(&right_integer));
    let fraction_order = (0..left_fraction.len().max(right_fraction.len()))
        .map(|index| {
            left_fraction
                .as_bytes()
                .get(index)
                .copied()
                .unwrap_or(b'0')
                .cmp(
                    &right_fraction
                        .as_bytes()
                        .get(index)
                        .copied()
                        .unwrap_or(b'0'),
                )
        })
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal);
    let magnitude = integer_order.then(fraction_order);
    if left_negative {
        magnitude.reverse()
    } else {
        magnitude
    }
}

fn instrument_id(instrument: &Instrument) -> &str {
    match instrument {
        Instrument::Spot(value) => value.instrument_id.as_str(),
        Instrument::LinearFuture(value) => value.instrument_id.as_str(),
        Instrument::Option(value) => value.instrument_id.as_str(),
    }
}

fn expired_after(instrument: &Instrument, instant: &UtcTimestamp) -> bool {
    let expiry = match instrument {
        Instrument::Spot(_) => return false,
        Instrument::LinearFuture(value) => &value.expiry_at,
        Instrument::Option(value) => &value.expiry_at,
    };
    compare_utc_timestamps(instant, expiry) == Ordering::Greater
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CivilDate {
    year: u32,
    month: u32,
    day: u32,
}

impl CivilDate {
    fn from_timestamp(value: &UtcTimestamp) -> Self {
        let date = &value.as_str()[..10];
        Self {
            year: date[..4].parse().expect("validated timestamp year"),
            month: date[5..7].parse().expect("validated timestamp month"),
            day: date[8..10].parse().expect("validated timestamp day"),
        }
    }

    fn next_day(self) -> Self {
        let leap = self.year.is_multiple_of(400)
            || (self.year.is_multiple_of(4) && !self.year.is_multiple_of(100));
        let days = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => unreachable!("validated timestamp month"),
        };
        if self.day < days {
            Self {
                day: self.day + 1,
                ..self
            }
        } else if self.month < 12 {
            Self {
                month: self.month + 1,
                day: 1,
                ..self
            }
        } else {
            Self {
                year: self.year + 1,
                month: 1,
                day: 1,
            }
        }
    }

    fn at(self, hour: u32) -> UtcTimestamp {
        UtcTimestamp::new(format!(
            "{:04}-{:02}-{:02}T{:02}:00:00Z",
            self.year, self.month, self.day, hour
        ))
        .expect("synthetic session time is a valid UTC timestamp")
    }
}

fn session_times(
    start_at: &UtcTimestamp,
    end_at: &UtcTimestamp,
) -> Result<(Vec<UtcTimestamp>, Vec<UtcTimestamp>), &'static str> {
    if !start_at.as_str().ends_with("T16:00:00Z")
        || !end_at.as_str().ends_with("T16:00:00Z")
        || compare_utc_timestamps(start_at, end_at) == Ordering::Greater
    {
        return Err("synthetic MVP boundaries must be ordered daily closes at 16:00 UTC");
    }
    let end_date = CivilDate::from_timestamp(end_at);
    let mut date = CivilDate::from_timestamp(start_at);
    let mut closes = Vec::new();
    let mut opens = Vec::new();
    loop {
        closes.push(date.at(16));
        if date >= end_date {
            break;
        }
        date = date.next_day();
        opens.push(date.at(9));
    }
    Ok((closes, opens))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixtureRegistry;
    use factorsbt_contracts::{Identifier, RunConfiguration};
    use serde_json::json;
    use std::path::Path;

    fn baseline() -> (crate::Fixture, RunConfiguration) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = (*registry.get_by("mvp-synthetic", "v1").unwrap()).clone();
        let config = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        (fixture, config)
    }

    fn has_code(issues: &[CoverageIssue], code: CoverageIssueCode) -> bool {
        issues.iter().any(|issue| issue.code == code)
    }

    #[test]
    fn accepts_baseline_and_allows_empty_news() {
        let (fixture, config) = baseline();
        let mut dataset = (*fixture.dataset).clone();
        dataset
            .observations
            .retain(|observation| observation.kind != ObservationKind::News);
        assert_eq!(validate_run_coverage(&dataset, &config), Ok(()));
    }

    #[test]
    fn differentiates_missing_stale_and_not_yet_available_data() {
        let (fixture, config) = baseline();
        let mut missing = (*fixture.dataset).clone();
        missing.observations.retain(|observation| {
            !(observation
                .instrument_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "SPOT_A")
                && observation.kind == ObservationKind::Bar
                && observation.event_time.as_str() == "2026-01-06T16:00:00Z")
        });
        assert!(has_code(
            &validate_run_coverage(&missing, &config).unwrap_err(),
            CoverageIssueCode::StaleMarketObservation
        ));

        let mut unavailable = (*fixture.dataset).clone();
        let bar = unavailable
            .observations
            .iter_mut()
            .find(|observation| {
                observation
                    .instrument_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == "SPOT_A")
                    && observation.kind == ObservationKind::Bar
                    && observation.event_time.as_str() == "2026-01-06T16:00:00Z"
            })
            .unwrap();
        bar.available_at = UtcTimestamp::new("2026-01-06T17:00:00Z").unwrap();
        assert!(has_code(
            &validate_run_coverage(&unavailable, &config).unwrap_err(),
            CoverageIssueCode::MarketObservationUnavailable
        ));

        let mut no_open = (*fixture.dataset).clone();
        no_open.execution_prices.retain(|price| {
            !(price.instrument_id.as_str() == "SPOT_A"
                && price.session_open.as_str() == "2026-01-07T09:00:00Z")
        });
        assert!(has_code(
            &validate_run_coverage(&no_open, &config).unwrap_err(),
            CoverageIssueCode::StaleExecutionPrice
        ));
    }

    #[test]
    fn rejects_fixture_mismatch_and_invalid_interval() {
        let (fixture, mut config) = baseline();
        config.data.fixture_version = Identifier::new("v2").unwrap();
        assert!(has_code(
            &validate_run_coverage(&fixture.dataset, &config).unwrap_err(),
            CoverageIssueCode::InvalidFixtureIdentity
        ));
        config.data.fixture_version = Identifier::new("v1").unwrap();
        config.simulation.start_at = UtcTimestamp::new("2026-01-05T09:00:00Z").unwrap();
        assert!(has_code(
            &validate_run_coverage(&fixture.dataset, &config).unwrap_err(),
            CoverageIssueCode::InvalidRunInterval
        ));
    }

    #[test]
    fn distinguishes_missing_theoretical_pricing_input_and_rejects_bad_payloads() {
        let (fixture, mut config) = baseline();
        let option: Instrument = serde_json::from_value(json!({
            "instrument_type":"option", "instrument_id":"EU_CALL_A", "quote_currency":"USD",
            "price_tick":"0.01", "quantity_step":"1", "contract_multiplier":"100",
            "fee_bps":"0", "fee_per_unit":"0", "slippage_bps":"0", "underlying_id":"SPOT_A",
            "option_type":"call", "strike":"100", "expiry_at":"2026-01-08T16:00:00Z",
            "exercise_style":"european", "settlement_style":"cash", "valuation_mode":"theoretical",
            "valuation_inputs":{"pricer":"black_scholes","day_count":"ACT/365F"},
            "initial_margin_per_contract":"600", "maintenance_margin_per_contract":"600"
        }))
        .unwrap();
        config.instruments.push(option);
        let mut missing = (*fixture.dataset).clone();
        missing.observations.retain(|observation| {
            !(observation.kind == ObservationKind::OptionPricingInput
                && observation
                    .instrument_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == "EU_CALL_A"))
        });
        assert!(has_code(
            &validate_run_coverage(&missing, &config).unwrap_err(),
            CoverageIssueCode::MissingPricingInput
        ));

        let mut invalid = (*fixture.dataset).clone();
        let quote = invalid
            .observations
            .iter_mut()
            .find(|observation| {
                observation.kind == ObservationKind::Quote
                    && observation
                        .instrument_id
                        .as_ref()
                        .is_some_and(|id| id.as_str() == "EU_CALL_A")
                    && observation.event_time.as_str() == "2026-01-05T16:00:00Z"
            })
            .unwrap();
        quote.payload = ObservationPayload::Quote(factorsbt_contracts::temporal::QuotePayload {
            bid: factorsbt_contracts::DecimalString::new("6").unwrap(),
            ask: factorsbt_contracts::DecimalString::new("5").unwrap(),
            mark: factorsbt_contracts::DecimalString::new("5").unwrap(),
        });
        let quote_config: RunConfiguration = serde_json::from_value({
            let mut value: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
            ))
            .unwrap();
            value["instruments"].as_array_mut().unwrap().push(json!({
                "instrument_type":"option", "instrument_id":"EU_CALL_A", "quote_currency":"USD",
                "price_tick":"0.01", "quantity_step":"1", "contract_multiplier":"100",
                "fee_bps":"0", "fee_per_unit":"0", "slippage_bps":"0", "underlying_id":"SPOT_A",
                "option_type":"call", "strike":"100", "expiry_at":"2026-01-08T16:00:00Z",
                "exercise_style":"european", "settlement_style":"cash", "valuation_mode":"quote",
                "initial_margin_per_contract":"600", "maintenance_margin_per_contract":"600"
            }));
            value
        })
        .unwrap();
        assert!(has_code(
            &validate_run_coverage(&invalid, &quote_config).unwrap_err(),
            CoverageIssueCode::InvalidObservation
        ));
    }
}
