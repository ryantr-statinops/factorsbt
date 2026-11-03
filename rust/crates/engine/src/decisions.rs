//! Deterministic conversion of score responses into snapshot-time targets.

use crate::{portfolio::decimal, EngineError, PortfolioState};
use factorsbt_contracts::{
    instruments::Instrument,
    model::{DecisionPolicy, DerivativeScoreTarget, ModelResponse, OutputMode},
    InstrumentId,
};
use rust_decimal::Decimal;
use std::{collections::HashMap, error::Error, fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentTarget {
    pub instrument_id: InstrumentId,
    pub target_quantity: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionError {
    OutputModeMismatch,
    MissingScorePolicy,
    InvalidTopK,
    DuplicateInstrument(String),
    UnknownInstrument(String),
    InvalidScore(String),
    InvalidDerivativeTarget(String),
    InvalidTarget(String),
    NonPositiveNav,
    Arithmetic(EngineError),
}

impl fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputModeMismatch => {
                formatter.write_str("model response does not match configured output mode")
            }
            Self::MissingScorePolicy => {
                formatter.write_str("scores output requires a score policy")
            }
            Self::InvalidTopK => formatter.write_str("score policy top_k must be positive"),
            Self::DuplicateInstrument(id) => {
                write!(formatter, "duplicate decision instrument {id}")
            }
            Self::UnknownInstrument(id) => {
                write!(formatter, "decision references unknown instrument {id}")
            }
            Self::InvalidScore(id) => write!(formatter, "invalid score for instrument {id}"),
            Self::InvalidDerivativeTarget(id) => {
                write!(formatter, "invalid derivative score target for {id}")
            }
            Self::InvalidTarget(id) => {
                write!(formatter, "invalid or unrepresentable target for {id}")
            }
            Self::NonPositiveNav => {
                formatter.write_str("cannot allocate spot targets from nonpositive NAV")
            }
            Self::Arithmetic(error) => write!(formatter, "decision arithmetic failed: {error}"),
        }
    }
}

impl Error for DecisionError {}

impl From<EngineError> for DecisionError {
    fn from(value: EngineError) -> Self {
        Self::Arithmetic(value)
    }
}

/// Select positive scores with deterministic score-descending/instrument-ID
/// ordering, then convert them to quantities using the current portfolio marks.
pub fn scores_to_targets(
    policy: &DecisionPolicy,
    response: &ModelResponse,
    instruments: &[Instrument],
    portfolio: &PortfolioState,
) -> Result<Vec<InstrumentTarget>, DecisionError> {
    if policy.output_mode != OutputMode::Scores {
        return Err(DecisionError::OutputModeMismatch);
    }
    let score_policy = policy
        .score_policy
        .as_ref()
        .ok_or(DecisionError::MissingScorePolicy)?;
    if score_policy.top_k == 0 {
        return Err(DecisionError::InvalidTopK);
    }
    let ModelResponse::Scores { outputs, .. } = response else {
        return Err(DecisionError::OutputModeMismatch);
    };

    let by_id = instrument_index(instruments)?;
    let scores = parse_scores(&outputs.scores, &by_id)?;
    let derivative_targets = derivative_targets(&score_policy.derivative_score_targets, &by_id)?;
    let mut ranked = scores
        .into_iter()
        .filter(|(_, score)| *score > Decimal::ZERO)
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_id, left_score), (right_id, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_id.cmp(right_id))
    });

    let mut quantities = by_id
        .keys()
        .map(|id| (id.clone(), Decimal::ZERO))
        .collect::<HashMap<_, _>>();
    let spot_weight = Decimal::ONE
        .checked_div(Decimal::from(score_policy.top_k))
        .ok_or(DecisionError::InvalidTopK)?;
    for (id, _) in ranked.into_iter().take(score_policy.top_k as usize) {
        let instrument = by_id.get(&id).expect("score identity was validated");
        let target = match instrument {
            Instrument::Spot(spot) => {
                let nav = portfolio.net_asset_value()?;
                if nav <= Decimal::ZERO {
                    return Err(DecisionError::NonPositiveNav);
                }
                let mark = portfolio
                    .position(&id)
                    .ok_or_else(|| DecisionError::UnknownInstrument(id.clone()))?
                    .mark;
                let multiplier = decimal(spot.contract_multiplier.as_str())?;
                let step = decimal(spot.quantity_step.as_str())?;
                if mark <= Decimal::ZERO || multiplier <= Decimal::ZERO || step <= Decimal::ZERO {
                    return Err(DecisionError::InvalidTarget(id));
                }
                let market_value = nav
                    .checked_mul(spot_weight)
                    .ok_or_else(|| DecisionError::InvalidTarget(id.clone()))?;
                let raw_quantity = market_value
                    .checked_div(mark)
                    .and_then(|value| value.checked_div(multiplier))
                    .ok_or_else(|| DecisionError::InvalidTarget(id.clone()))?;
                let lots = (raw_quantity / step).trunc();
                lots.checked_mul(step)
                    .ok_or_else(|| DecisionError::InvalidTarget(id.clone()))?
            }
            Instrument::LinearFuture(_) | Instrument::Option(_) => {
                Decimal::from(*derivative_targets.get(&id).unwrap_or(&0))
            }
        };
        quantities.insert(id, target);
    }
    let mut targets = quantities
        .into_iter()
        .map(|(id, target_quantity)| InstrumentTarget {
            instrument_id: InstrumentId::new(id).expect("configured ID was validated"),
            target_quantity,
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
    });
    Ok(targets)
}

fn instrument_index(
    instruments: &[Instrument],
) -> Result<HashMap<String, &Instrument>, DecisionError> {
    let mut result = HashMap::new();
    for instrument in instruments {
        let id = instrument_id(instrument).as_str().to_owned();
        if result.insert(id.clone(), instrument).is_some() {
            return Err(DecisionError::DuplicateInstrument(id));
        }
    }
    Ok(result)
}

fn parse_scores(
    scores: &[factorsbt_contracts::model::InstrumentScore],
    instruments: &HashMap<String, &Instrument>,
) -> Result<HashMap<String, Decimal>, DecisionError> {
    let mut result = HashMap::new();
    for item in scores {
        let id = item.instrument_id.as_str().to_owned();
        if !instruments.contains_key(&id) {
            return Err(DecisionError::UnknownInstrument(id));
        }
        let score = Decimal::from_str(item.score.as_str())
            .map_err(|_| DecisionError::InvalidScore(id.clone()))?;
        if result.insert(id.clone(), score).is_some() {
            return Err(DecisionError::DuplicateInstrument(id));
        }
    }
    Ok(result)
}

fn derivative_targets(
    targets: &[DerivativeScoreTarget],
    instruments: &HashMap<String, &Instrument>,
) -> Result<HashMap<String, i32>, DecisionError> {
    let mut result = HashMap::new();
    for target in targets {
        let id = target.instrument_id.as_str().to_owned();
        let Some(instrument) = instruments.get(&id) else {
            return Err(DecisionError::UnknownInstrument(id));
        };
        if matches!(instrument, Instrument::Spot(_))
            || result.insert(id.clone(), target.contracts).is_some()
        {
            return Err(DecisionError::InvalidDerivativeTarget(id));
        }
    }
    Ok(result)
}

fn instrument_id(instrument: &Instrument) -> &InstrumentId {
    match instrument {
        Instrument::Spot(value) => &value.instrument_id,
        Instrument::LinearFuture(value) => &value.instrument_id,
        Instrument::Option(value) => &value.instrument_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_data::FixtureRegistry;
    use std::path::Path;

    fn baseline() -> (Vec<Instrument>, PortfolioState) {
        let mut config: factorsbt_contracts::RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config.instruments.push(serde_json::from_str(
            r#"{"instrument_id":"SPOT_B","instrument_type":"spot","quote_currency":"USD","price_tick":"0.01","quantity_step":"0.001","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0"}"#,
        ).unwrap());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let portfolio = PortfolioState::initialize(&config, &fixture).unwrap();
        (config.instruments, portfolio)
    }

    fn response(scores: &str) -> ModelResponse {
        let value = format!(
            r#"{{"contract_version":"1.0.0","inference_id":"de87564a-ae4a-4768-bb05-97bc1d816d8f","run_id":"c827fcbc-c5fc-48a4-b4aa-21a63f64727e","scenario_id":"baseline","step_id":"step-0001","snapshot_id":"4fe68f00-0ddb-4e72-ad9b-7bc09d84d5aa","model_id":"synthetic-model","model_version":"v1","responded_at":"2026-01-05T16:00:01Z","response_type":"scores","outputs":{{"scores":[{scores}]}}}}"#
        );
        serde_json::from_str(&value).unwrap()
    }

    fn policy() -> DecisionPolicy {
        serde_json::from_str(
            r#"{"output_mode":"scores","score_policy":{"top_k":2,"derivative_score_targets":[]}}"#,
        )
        .unwrap()
    }

    #[test]
    fn top_k_ties_use_instrument_id_and_spot_weights_convert_at_snapshot_marks() {
        let (instruments, portfolio) = baseline();
        let model = response(
            r#"{"instrument_id":"SPOT_B","score":"2"},{"instrument_id":"SPOT_A","score":"2"}"#,
        );
        let targets = scores_to_targets(&policy(), &model, &instruments, &portfolio).unwrap();
        assert_eq!(targets[0].instrument_id.as_str(), "SPOT_A");
        assert_eq!(targets[0].target_quantity, Decimal::from(500));
        assert_eq!(targets[1].instrument_id.as_str(), "SPOT_B");
        assert_eq!(targets[1].target_quantity, Decimal::from(1000));
        assert_eq!(
            targets
                .iter()
                .map(|target| target.target_quantity)
                .sum::<Decimal>(),
            Decimal::from(1500)
        );
    }

    #[test]
    fn nonpositive_or_omitted_scores_keep_zero_targets_and_invalid_ids_fail() {
        let (instruments, portfolio) = baseline();
        let model = response(r#"{"instrument_id":"SPOT_A","score":"-1"}"#);
        let targets = scores_to_targets(&policy(), &model, &instruments, &portfolio).unwrap();
        assert!(targets
            .iter()
            .all(|target| target.target_quantity.is_zero()));
        let unknown = response(r#"{"instrument_id":"UNKNOWN","score":"1"}"#);
        assert!(matches!(
            scores_to_targets(&policy(), &unknown, &instruments, &portfolio),
            Err(DecisionError::UnknownInstrument(_))
        ));
    }
}
