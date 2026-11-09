//! Fixed action-sequence episodes with independent portfolio state.

use super::{
    engine_support::{build_inputs, open_timestamp},
    fixtures::ResearchFixture,
};
use factorsbt_contracts::{Identifier, InstrumentId, UtcTimestamp};
use factorsbt_data::{execution_prices_available_at, observations_available_at};
use factorsbt_engine::{
    admit_target_group_at, execute_pending_group, queue_target_group, value_instruments,
    ExecutionFill, InstrumentTarget, PortfolioState,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeAction {
    pub target_weight: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeObservation {
    pub simulation_time: String,
    pub mark: Decimal,
    pub cash: Decimal,
    pub quantity: Decimal,
    pub nav: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeFill {
    pub instrument_id: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeTransition {
    pub step: usize,
    pub observation_start: EpisodeObservation,
    pub action: EpisodeAction,
    pub admitted_quantity: Decimal,
    pub fills: Vec<EpisodeFill>,
    pub observation_end: EpisodeObservation,
    pub reward_usd: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeReport {
    pub fixture_id: String,
    pub fixture_version: String,
    pub transitions: Vec<EpisodeTransition>,
    pub final_observation: EpisodeObservation,
    pub canceled_pending_orders: Vec<String>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpisodeError {
    InvalidActionCount,
    InvalidAction(usize),
    InvalidFixture,
    Arithmetic,
    InsufficientCash,
}

impl fmt::Display for EpisodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActionCount => formatter.write_str("episode requires exactly two actions"),
            Self::InvalidAction(index) => {
                write!(formatter, "episode action {index} is not 0, 0.5, or 1")
            }
            Self::InvalidFixture => {
                formatter.write_str("episode fixture must contain one spot and three close points")
            }
            Self::Arithmetic => formatter.write_str("episode decimal arithmetic overflow"),
            Self::InsufficientCash => {
                formatter.write_str("episode target is not affordable at execution open")
            }
        }
    }
}
impl Error for EpisodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeState {
    pub cash: Decimal,
    pub quantity: Decimal,
    pub mark: Decimal,
}

pub fn validate_episode(
    fixture: &ResearchFixture,
    actions: &[EpisodeAction],
) -> Result<EpisodeState, EpisodeError> {
    if actions.len() != 2 {
        return Err(EpisodeError::InvalidActionCount);
    }
    for (index, action) in actions.iter().enumerate() {
        if action.target_weight != Decimal::ZERO
            && action.target_weight != Decimal::new(5, 1)
            && action.target_weight != Decimal::ONE
        {
            return Err(EpisodeError::InvalidAction(index));
        }
    }
    if fixture.points.len() != 3
        || fixture.instruments.len() != 1
        || fixture.kind != super::fixtures::ResearchFixtureKind::Episode
    {
        return Err(EpisodeError::InvalidFixture);
    }
    let initial = fixture.points.first().ok_or(EpisodeError::InvalidFixture)?;
    Ok(EpisodeState {
        cash: fixture.initial_cash,
        quantity: Decimal::ZERO,
        mark: initial.close_a,
    })
}

/// Apply the two recorded actions across the three close points. Each step
/// executes only at the next fixture open and books fees before reward.
pub fn run_episode(
    fixture: &ResearchFixture,
    actions: &[EpisodeAction],
) -> Result<EpisodeReport, EpisodeError> {
    run_episode_with_progress(fixture, actions, |_| {})
}

/// Run an episode while exposing only completed transitions to the caller.
/// Callers can persist the last valid observation if a later step fails.
pub fn run_episode_with_progress<F>(
    fixture: &ResearchFixture,
    actions: &[EpisodeAction],
    mut on_transition: F,
) -> Result<EpisodeReport, EpisodeError>
where
    F: FnMut(&EpisodeTransition),
{
    validate_episode(fixture, actions)?;
    let inputs = build_inputs(fixture).map_err(|_| EpisodeError::InvalidFixture)?;
    let instruments = inputs.configuration.instruments.clone();
    let mut portfolio = PortfolioState::initialize(&inputs.configuration, &inputs.fixture)
        .map_err(|_| EpisodeError::InvalidFixture)?;
    let first_time = inputs.configuration.simulation.start_at.clone();
    let first_observations = observations_available_at(&inputs.fixture.dataset, &first_time)
        .map_err(|_| EpisodeError::InvalidFixture)?;
    let first_marks = value_instruments(&instruments, &first_observations, &first_time)
        .map_err(|_| EpisodeError::InvalidFixture)?;
    portfolio
        .apply_valuation_marks(&first_marks)
        .map_err(|_| EpisodeError::InvalidFixture)?;
    let mut transitions = Vec::with_capacity(2);
    let mut last_completed_time = fixture
        .points
        .first()
        .ok_or(EpisodeError::InvalidFixture)?
        .close_at
        .clone();
    let mut failure = None;
    for (step, action) in actions.iter().enumerate() {
        let checkpoint = portfolio.clone();
        let transition_result = (|| -> Result<EpisodeTransition, EpisodeError> {
            let start = &fixture.points[step];
            let end = &fixture.points[step + 1];
            let observation_start = engine_observation(&start.close_at, &portfolio)?;
            let requested_quantity = observation_start
                .nav
                .checked_mul(action.target_weight)
                .and_then(|value| value.checked_div(start.close_a))
                .ok_or(EpisodeError::Arithmetic)?
                .trunc();
            let decision_time = UtcTimestamp::new(start.close_at.clone())
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let execution_time = UtcTimestamp::new(open_timestamp(&end.close_at))
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let target = InstrumentTarget {
                instrument_id: InstrumentId::new("SPOT_A")
                    .map_err(|_| EpisodeError::InvalidFixture)?,
                target_quantity: requested_quantity,
            };
            let admission = admit_target_group_at(&portfolio, &[target], &decision_time)
                .map_err(|_| EpisodeError::InsufficientCash)?;
            let step_id = Identifier::new(format!("episode-step-{step}"))
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let group = queue_target_group(
                &portfolio,
                &step_id,
                &decision_time,
                &execution_time,
                &admission,
            )
            .map_err(|_| EpisodeError::InvalidFixture)?;
            let prices = execution_prices_available_at(&inputs.fixture.dataset, &execution_time)
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let executed = execute_pending_group(&mut portfolio, &group, &execution_time, &prices)
                .map_err(|_| EpisodeError::InsufficientCash)?;
            let admitted_quantity = executed
                .admission_adjustments
                .first()
                .map(|adjustment| adjustment.admitted_quantity)
                .unwrap_or_else(|| {
                    portfolio
                        .position("SPOT_A")
                        .expect("instrument configured")
                        .quantity
                });
            let fills = executed
                .fills
                .into_iter()
                .map(|fill| match fill {
                    ExecutionFill::Spot(fill) => Ok(EpisodeFill {
                        instrument_id: fill.instrument_id.as_str().to_owned(),
                        quantity: fill.signed_quantity,
                        price: fill.fill_price,
                        fee: fill.fee,
                    }),
                    _ => Err(EpisodeError::InvalidFixture),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let close_time = UtcTimestamp::new(end.close_at.clone())
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let observations = observations_available_at(&inputs.fixture.dataset, &close_time)
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let marks = value_instruments(&instruments, &observations, &close_time)
                .map_err(|_| EpisodeError::InvalidFixture)?;
            portfolio
                .apply_valuation_marks(&marks)
                .map_err(|_| EpisodeError::InvalidFixture)?;
            let observation_end = engine_observation(&end.close_at, &portfolio)?;
            let reward = observation_end
                .nav
                .checked_sub(observation_start.nav)
                .ok_or(EpisodeError::Arithmetic)?;
            let transition = EpisodeTransition {
                step,
                observation_start,
                action: action.clone(),
                admitted_quantity,
                fills,
                observation_end,
                reward_usd: reward,
            };
            Ok(transition)
        })();
        match transition_result {
            Ok(transition) => {
                last_completed_time = transition.observation_end.simulation_time.clone();
                on_transition(&transition);
                transitions.push(transition);
            }
            Err(error) => {
                portfolio = checkpoint;
                failure = Some(error.to_string());
                break;
            }
        }
    }
    let final_observation = engine_observation(&last_completed_time, &portfolio)?;
    Ok(EpisodeReport {
        fixture_id: fixture.fixture_id.clone(),
        fixture_version: fixture.version.clone(),
        transitions,
        final_observation,
        canceled_pending_orders: Vec::new(),
        failure,
    })
}

fn engine_observation(
    at: &str,
    portfolio: &PortfolioState,
) -> Result<EpisodeObservation, EpisodeError> {
    let position = portfolio
        .position("SPOT_A")
        .ok_or(EpisodeError::InvalidFixture)?;
    Ok(EpisodeObservation {
        simulation_time: at.to_owned(),
        mark: position.mark,
        cash: portfolio.cash(),
        quantity: position.quantity,
        nav: portfolio
            .net_asset_value()
            .map_err(|_| EpisodeError::Arithmetic)?,
    })
}

pub fn baseline_actions() -> [EpisodeAction; 2] {
    [
        EpisodeAction {
            target_weight: Decimal::ZERO,
        },
        EpisodeAction {
            target_weight: Decimal::ZERO,
        },
    ]
}

pub fn fixed_trade_actions() -> [EpisodeAction; 2] {
    [
        EpisodeAction {
            target_weight: Decimal::new(5, 1),
        },
        EpisodeAction {
            target_weight: Decimal::new(5, 1),
        },
    ]
}
