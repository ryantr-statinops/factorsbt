//! Point-in-time valuation marks and their input provenance.

use crate::{
    portfolio::decimal,
    pricing::{
        black_scholes_european_value, crr_american_value, time_to_expiry_act_365f,
        validate_pricing_inputs, PricingError,
    },
    EngineError,
};
use factorsbt_contracts::{
    compare_utc_timestamps,
    instruments::{ExerciseStyle, Instrument, OptionValuationInputs, ValuationMode},
    temporal::{Observation, ObservationKind, ObservationPayload},
    InstrumentId, UtcTimestamp,
};
use rust_decimal::Decimal;
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkSource {
    BarClose,
    ExecutionOpen,
    OptionQuote,
    BlackScholes,
    Crr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkProvenance {
    pub observation_id: String,
    pub source_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationMark {
    pub instrument_id: InstrumentId,
    pub mark: Decimal,
    pub source: MarkSource,
    pub provenance: Vec<MarkProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuationError {
    MissingObservation {
        instrument_id: String,
        kind: &'static str,
    },
    AmbiguousObservation {
        instrument_id: String,
        kind: &'static str,
    },
    InvalidPayload(String),
    MissingPricer(String),
    PricerMismatch(String),
    Pricing(PricingError),
    InvalidMark(String),
    Arithmetic(EngineError),
}

impl fmt::Display for ValuationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingObservation {
                instrument_id,
                kind,
            } => {
                write!(
                    formatter,
                    "missing current {kind} observation for {instrument_id}"
                )
            }
            Self::AmbiguousObservation {
                instrument_id,
                kind,
            } => {
                write!(
                    formatter,
                    "ambiguous current {kind} observations for {instrument_id}"
                )
            }
            Self::InvalidPayload(id) => write!(formatter, "invalid valuation payload {id}"),
            Self::MissingPricer(id) => write!(formatter, "theoretical option {id} has no pricer"),
            Self::PricerMismatch(id) => write!(
                formatter,
                "configured pricer does not match option style {id}"
            ),
            Self::Pricing(error) => write!(formatter, "option valuation failed: {error}"),
            Self::InvalidMark(id) => write!(formatter, "mark for {id} must be nonnegative"),
            Self::Arithmetic(error) => write!(formatter, "valuation decimal failed: {error}"),
        }
    }
}

impl Error for ValuationError {}

impl From<PricingError> for ValuationError {
    fn from(value: PricingError) -> Self {
        Self::Pricing(value)
    }
}

/// Value configured instruments at a completed simulation timestamp.
/// Current-time observations are required; older values are never carried forward.
pub fn value_instruments(
    instruments: &[Instrument],
    observations: &[Observation],
    simulation_time: &UtcTimestamp,
) -> Result<Vec<ValuationMark>, ValuationError> {
    let mut result = instruments
        .iter()
        .map(|instrument| value_instrument(instrument, observations, simulation_time))
        .collect::<Result<Vec<_>, _>>()?;
    result.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
    });
    Ok(result)
}

fn value_instrument(
    instrument: &Instrument,
    observations: &[Observation],
    simulation_time: &UtcTimestamp,
) -> Result<ValuationMark, ValuationError> {
    match instrument {
        Instrument::Spot(value) => {
            let observation = current_observation(
                observations,
                value.instrument_id.as_str(),
                ObservationKind::Bar,
                simulation_time,
            )?;
            let ObservationPayload::Bar(bar) = &observation.payload else {
                return Err(ValuationError::InvalidPayload(
                    observation.observation_id.as_str().to_owned(),
                ));
            };
            make_mark(
                &value.instrument_id,
                decimal(bar.close.as_str())?,
                MarkSource::BarClose,
                vec![observation],
            )
        }
        Instrument::LinearFuture(value) => {
            let observation = current_observation(
                observations,
                value.instrument_id.as_str(),
                ObservationKind::Bar,
                simulation_time,
            )?;
            let ObservationPayload::Bar(bar) = &observation.payload else {
                return Err(ValuationError::InvalidPayload(
                    observation.observation_id.as_str().to_owned(),
                ));
            };
            make_mark(
                &value.instrument_id,
                decimal(bar.close.as_str())?,
                MarkSource::BarClose,
                vec![observation],
            )
        }
        Instrument::Option(option) if option.valuation_mode == ValuationMode::Quote => {
            let observation = current_observation(
                observations,
                option.instrument_id.as_str(),
                ObservationKind::Quote,
                simulation_time,
            )?;
            let ObservationPayload::Quote(quote) = &observation.payload else {
                return Err(ValuationError::InvalidPayload(
                    observation.observation_id.as_str().to_owned(),
                ));
            };
            make_mark(
                &option.instrument_id,
                decimal(quote.mark.as_str())?,
                MarkSource::OptionQuote,
                vec![observation],
            )
        }
        Instrument::Option(option) => {
            let underlying = current_observation(
                observations,
                option.underlying_id.as_str(),
                ObservationKind::Bar,
                simulation_time,
            )?;
            let ObservationPayload::Bar(bar) = &underlying.payload else {
                return Err(ValuationError::InvalidPayload(
                    underlying.observation_id.as_str().to_owned(),
                ));
            };
            let pricing = current_observation(
                observations,
                option.instrument_id.as_str(),
                ObservationKind::OptionPricingInput,
                simulation_time,
            )?;
            let ObservationPayload::OptionPricingInput(inputs) = &pricing.payload else {
                return Err(ValuationError::InvalidPayload(
                    pricing.observation_id.as_str().to_owned(),
                ));
            };
            let underlying_price = decimal(bar.close.as_str())?;
            let strike = decimal(option.strike.as_str())?;
            let rate = decimal(inputs.risk_free_rate.as_str())?;
            let volatility = decimal(inputs.annualized_volatility.as_str())?;
            let time = time_to_expiry_act_365f(simulation_time, &option.expiry_at)?;
            let validated =
                validate_pricing_inputs(underlying_price, strike, rate, volatility, time)?;
            let (mark, source) = match (&option.exercise_style, option.valuation_inputs.as_ref()) {
                (ExerciseStyle::European, Some(OptionValuationInputs::BlackScholes { .. })) => (
                    black_scholes_european_value(option.option_type, validated)?,
                    MarkSource::BlackScholes,
                ),
                (ExerciseStyle::American, Some(OptionValuationInputs::Crr { crr_steps, .. })) => (
                    crr_american_value(option.option_type, validated, *crr_steps)?,
                    MarkSource::Crr,
                ),
                (_, None) => {
                    return Err(ValuationError::MissingPricer(
                        option.instrument_id.as_str().to_owned(),
                    ))
                }
                _ => {
                    return Err(ValuationError::PricerMismatch(
                        option.instrument_id.as_str().to_owned(),
                    ))
                }
            };
            make_mark(
                &option.instrument_id,
                mark,
                source,
                vec![underlying, pricing],
            )
        }
    }
}

fn current_observation<'a>(
    observations: &'a [Observation],
    instrument_id: &str,
    kind: ObservationKind,
    simulation_time: &UtcTimestamp,
) -> Result<&'a Observation, ValuationError> {
    let name = match kind {
        ObservationKind::Bar => "bar",
        ObservationKind::Quote => "quote",
        ObservationKind::OptionPricingInput => "option pricing input",
        ObservationKind::News => "news",
    };
    let candidates = observations
        .iter()
        .filter(|observation| {
            observation.kind == kind
                && observation
                    .instrument_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == instrument_id)
                && compare_utc_timestamps(&observation.event_time, simulation_time)
                    == Ordering::Equal
                && compare_utc_timestamps(&observation.available_at, simulation_time)
                    != Ordering::Greater
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => Err(ValuationError::MissingObservation {
            instrument_id: instrument_id.to_owned(),
            kind: name,
        }),
        [observation] => Ok(observation),
        _ => Err(ValuationError::AmbiguousObservation {
            instrument_id: instrument_id.to_owned(),
            kind: name,
        }),
    }
}

fn make_mark(
    instrument_id: &InstrumentId,
    mark: Decimal,
    source: MarkSource,
    observations: Vec<&Observation>,
) -> Result<ValuationMark, ValuationError> {
    if mark < Decimal::ZERO {
        return Err(ValuationError::InvalidMark(
            instrument_id.as_str().to_owned(),
        ));
    }
    let provenance = observations
        .into_iter()
        .map(|observation| MarkProvenance {
            observation_id: observation.observation_id.as_str().to_owned(),
            source_revision: observation.revision,
        })
        .collect();
    Ok(ValuationMark {
        instrument_id: instrument_id.clone(),
        mark,
        source,
        provenance,
    })
}

impl From<EngineError> for ValuationError {
    fn from(value: EngineError) -> Self {
        Self::Arithmetic(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::instruments::Instrument;
    use factorsbt_data::{observations_available_at, FixtureRegistry};
    use std::path::Path;

    fn fixture_observations(time: &str) -> Vec<Observation> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-opt-01", "v1").unwrap();
        observations_available_at(&fixture.dataset, &UtcTimestamp::new(time).unwrap()).unwrap()
    }

    fn option(mode: ValuationMode) -> Instrument {
        let valuation = match mode {
            ValuationMode::Quote => r#""quote","valuation_inputs":null"#,
            ValuationMode::Theoretical => {
                r#""theoretical","valuation_inputs":{"pricer":"black_scholes","day_count":"ACT/365F"}"#
            }
        };
        let json = format!(
            r#"{{"instrument_type":"option","instrument_id":"EU_CALL_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"100","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","option_type":"call","strike":"100","expiry_at":"2027-01-05T16:00:00Z","exercise_style":"european","settlement_style":"cash","valuation_mode":{},"initial_margin_per_contract":"1000","maintenance_margin_per_contract":"800"}}"#,
            valuation
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn quote_marks_use_current_fixture_quote_with_single_source_provenance() {
        let time = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let marks = value_instruments(
            &[option(ValuationMode::Quote)],
            &fixture_observations(time.as_str()),
            &time,
        )
        .unwrap();
        assert_eq!(marks[0].mark, decimal("10.45058357").unwrap());
        assert_eq!(marks[0].source, MarkSource::OptionQuote);
        assert_eq!(marks[0].provenance.len(), 1);
        assert_eq!(marks[0].provenance[0].observation_id, "OPT01-EU_CALL_A");
    }

    #[test]
    fn theoretical_marks_keep_underlying_and_pricing_input_provenance() {
        let time = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let marks = value_instruments(
            &[option(ValuationMode::Theoretical)],
            &fixture_observations(time.as_str()),
            &time,
        )
        .unwrap();
        assert!(
            (marks[0].mark - decimal("10.45058357").unwrap()).abs() < decimal("0.00001").unwrap()
        );
        assert_eq!(marks[0].source, MarkSource::BlackScholes);
        assert_eq!(marks[0].provenance.len(), 2);
        assert_eq!(marks[0].provenance[0].observation_id, "OPT01-SPOT_A");
        assert_eq!(
            marks[0].provenance[1].observation_id,
            "OPT01-INPUT-EU_CALL_A"
        );
    }

    #[test]
    fn valuation_does_not_carry_forward_an_older_quote_or_use_a_future_input() {
        let time = UtcTimestamp::new("2026-01-06T16:00:00Z").unwrap();
        let mut observations = fixture_observations("2026-01-05T16:00:00Z");
        assert!(matches!(
            value_instruments(&[option(ValuationMode::Quote)], &observations, &time),
            Err(ValuationError::MissingObservation { kind: "quote", .. })
        ));
        let quote = observations
            .iter_mut()
            .find(|observation| observation.observation_id.as_str() == "OPT01-EU_CALL_A")
            .unwrap();
        quote.available_at = UtcTimestamp::new("2026-01-07T16:00:00Z").unwrap();
        quote.event_time = time.clone();
        assert!(matches!(
            value_instruments(&[option(ValuationMode::Quote)], &observations, &time),
            Err(ValuationError::MissingObservation { kind: "quote", .. })
        ));
    }

    #[test]
    fn portfolio_mark_batch_updates_value_and_provenance_atomically() {
        let config: factorsbt_contracts::RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let time = config.simulation.start_at.clone();
        let observations = observations_available_at(&fixture.dataset, &time).unwrap();
        let marks = value_instruments(&config.instruments, &observations, &time).unwrap();
        let mut portfolio = crate::PortfolioState::initialize(&config, &fixture).unwrap();
        portfolio.apply_valuation_marks(&marks).unwrap();
        let position = portfolio.position("SPOT_A").unwrap();
        assert_eq!(position.mark, decimal("100").unwrap());
        assert_eq!(position.mark_source, Some(MarkSource::BarClose));
        assert_eq!(
            position.mark_provenance[0].observation_id,
            "BAR-SPOT_A-20260105"
        );

        let prior_mark = position.mark;
        let mut duplicate_batch = marks.clone();
        duplicate_batch.push(marks[0].clone());
        assert!(portfolio.apply_valuation_marks(&duplicate_batch).is_err());
        assert_eq!(portfolio.position("SPOT_A").unwrap().mark, prior_mark);
    }
}
