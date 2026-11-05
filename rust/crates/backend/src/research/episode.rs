//! Fixed action-sequence episodes with independent portfolio state.

use super::fixtures::ResearchFixture;
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
            Self::InvalidAction(index) => write!(formatter, "episode action {index} is not 0, 0.5, or 1"),
            Self::InvalidFixture => formatter.write_str("episode fixture must contain one spot and three close points"),
            Self::Arithmetic => formatter.write_str("episode decimal arithmetic overflow"),
            Self::InsufficientCash => formatter.write_str("episode target is not affordable at execution open"),
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

pub fn validate_episode(fixture: &ResearchFixture, actions: &[EpisodeAction]) -> Result<EpisodeState, EpisodeError> {
    if actions.len() != 2 { return Err(EpisodeError::InvalidActionCount); }
    for (index, action) in actions.iter().enumerate() {
        if action.target_weight != Decimal::ZERO && action.target_weight != Decimal::new(5, 1) && action.target_weight != Decimal::ONE {
            return Err(EpisodeError::InvalidAction(index));
        }
    }
    if fixture.points.len() != 3 || fixture.instruments.len() != 1 || fixture.kind != super::fixtures::ResearchFixtureKind::Episode {
        return Err(EpisodeError::InvalidFixture);
    }
    let initial = fixture.points.first().ok_or(EpisodeError::InvalidFixture)?;
    Ok(EpisodeState { cash: fixture.initial_cash, quantity: Decimal::ZERO, mark: initial.close_a })
}

fn observation(at: &str, mark: Decimal, state: &EpisodeState) -> Result<EpisodeObservation, EpisodeError> {
    let nav = state.cash.checked_add(state.quantity.checked_mul(mark).ok_or(EpisodeError::Arithmetic)?).ok_or(EpisodeError::Arithmetic)?;
    Ok(EpisodeObservation { simulation_time: at.into(), mark, cash: state.cash, quantity: state.quantity, nav })
}

/// Apply the two recorded actions across the three close points. Each step
/// executes only at the next fixture open and books fees before reward.
pub fn run_episode(fixture: &ResearchFixture, actions: &[EpisodeAction]) -> Result<EpisodeReport, EpisodeError> {
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
    let mut state = validate_episode(fixture, actions)?;
    let mut transitions = Vec::with_capacity(2);
    for (step, action) in actions.iter().enumerate() {
        let start = &fixture.points[step];
        let end = &fixture.points[step + 1];
        let observation_start = observation(&start.close_at, start.close_a, &state)?;
        let admitted_quantity = observation_start.nav.checked_mul(action.target_weight)
            .and_then(|value| value.checked_div(end.open_a)).ok_or(EpisodeError::Arithmetic)?.trunc();
        let delta = admitted_quantity.checked_sub(state.quantity).ok_or(EpisodeError::Arithmetic)?;
        let fee = if delta.is_zero() { Decimal::ZERO } else { fixture.fee_per_fill };
        let cash_after = state.cash.checked_sub(delta.checked_mul(end.open_a).ok_or(EpisodeError::Arithmetic)?)
            .and_then(|value| value.checked_sub(fee)).ok_or(EpisodeError::Arithmetic)?;
        if cash_after < Decimal::ZERO { return Err(EpisodeError::InsufficientCash); }
        state.cash = cash_after;
        state.quantity = admitted_quantity;
        state.mark = end.close_a;
        let observation_end = observation(&end.close_at, end.close_a, &state)?;
        let reward = observation_end.nav.checked_sub(observation_start.nav).ok_or(EpisodeError::Arithmetic)?;
        let fills = if delta.is_zero() { Vec::new() } else { vec![EpisodeFill { instrument_id: "SPOT_A".into(), quantity: delta, price: end.open_a, fee }] };
        let transition = EpisodeTransition { step, observation_start, action: action.clone(), admitted_quantity, fills, observation_end, reward_usd: reward };
        on_transition(&transition);
        transitions.push(transition);
    }
    let last = fixture.points.last().ok_or(EpisodeError::InvalidFixture)?;
    let final_observation = observation(&last.close_at, last.close_a, &state)?;
    Ok(EpisodeReport { fixture_id: fixture.fixture_id.clone(), fixture_version: fixture.version.clone(), transitions, final_observation, canceled_pending_orders: Vec::new(), failure: None })
}

pub fn baseline_actions() -> [EpisodeAction; 2] {
    [EpisodeAction { target_weight: Decimal::ZERO }, EpisodeAction { target_weight: Decimal::ZERO }]
}

pub fn fixed_trade_actions() -> [EpisodeAction; 2] {
    [EpisodeAction { target_weight: Decimal::new(5, 1) }, EpisodeAction { target_weight: Decimal::new(5, 1) }]
}

