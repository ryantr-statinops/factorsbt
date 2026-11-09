//! Shared-capital admission for complete, synchronized target groups.

use crate::{
    orders::project_executable_targets,
    portfolio::{decimal, validate_lot_quantity},
    EngineError, InstrumentTarget, PortfolioState,
};
use factorsbt_contracts::instruments::Instrument;
use factorsbt_contracts::UtcTimestamp;
use rust_decimal::{Decimal, RoundingStrategy};
use std::{collections::HashMap, error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionAdjustment {
    pub instrument_id: String,
    pub requested_quantity: Decimal,
    pub admitted_quantity: Decimal,
    pub common_scale: Decimal,
    pub scaled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionResult {
    pub adjustments: Vec<AdmissionAdjustment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    DuplicateTarget(String),
    UnknownInstrument(String),
    InvalidTarget(String),
    InfeasibleRiskReduction(EngineError),
    Arithmetic(EngineError),
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTarget(id) => write!(formatter, "duplicate target for {id}"),
            Self::UnknownInstrument(id) => {
                write!(formatter, "target references unknown instrument {id}")
            }
            Self::InvalidTarget(id) => write!(formatter, "target quantity is invalid for {id}"),
            Self::InfeasibleRiskReduction(error) => {
                write!(formatter, "risk-reducing close is infeasible: {error}")
            }
            Self::Arithmetic(error) => write!(formatter, "admission arithmetic failed: {error}"),
        }
    }
}

impl Error for AdmissionError {}

impl From<EngineError> for AdmissionError {
    fn from(value: EngineError) -> Self {
        Self::Arithmetic(value)
    }
}

/// Apply exposure reductions first, then use one 60-iteration bisection scale
/// for every increase in the decision group. The returned quantities are
/// proposals; next-open execution must recheck them against actual prices.
pub fn admit_target_group(
    portfolio: &PortfolioState,
    targets: &[InstrumentTarget],
) -> Result<AdmissionResult, AdmissionError> {
    admit_target_group_at(portfolio, targets, portfolio.start_at())
}

/// Admit targets at a particular simulated close or open using its current
/// marked prices as executable quote estimates.
pub fn admit_target_group_at(
    portfolio: &PortfolioState,
    targets: &[InstrumentTarget],
    at: &UtcTimestamp,
) -> Result<AdmissionResult, AdmissionError> {
    let mut requested = HashMap::new();
    for target in targets {
        let id = target.instrument_id.as_str().to_owned();
        let Some(instrument) = portfolio.instruments().get(&id) else {
            return Err(AdmissionError::UnknownInstrument(id));
        };
        if validate_lot_quantity(instrument, target.target_quantity).is_err()
            || matches!(instrument, Instrument::Spot(_)) && target.target_quantity < Decimal::ZERO
        {
            return Err(AdmissionError::InvalidTarget(id));
        }
        if requested
            .insert(id.clone(), target.target_quantity)
            .is_some()
        {
            return Err(AdmissionError::DuplicateTarget(id));
        }
    }

    let mut reductions = Vec::new();
    let mut base_quantities = HashMap::new();
    for (id, target) in &requested {
        let current = portfolio
            .position(id)
            .ok_or_else(|| AdmissionError::UnknownInstrument(id.clone()))?
            .quantity;
        let reduced = reduction_quantity(current, *target);
        if reduced != current {
            reductions.push((id.clone(), reduced));
            base_quantities.insert(id.clone(), reduced);
        } else {
            base_quantities.insert(id.clone(), current);
        }
    }
    let reduced_portfolio = project_executable_targets(portfolio, &reductions, at)
        .map_err(AdmissionError::InfeasibleRiskReduction)?;

    let full = scaled_targets(&requested, &base_quantities, portfolio, Decimal::ONE)?;
    match project_executable_targets(&reduced_portfolio, &full, at) {
        Ok(_) => return Ok(adjustments(&requested, &full, Decimal::ONE)),
        Err(error) if is_capacity_error(&error) => {}
        Err(error) => return Err(AdmissionError::Arithmetic(error)),
    }

    let zero_scale = scaled_targets(&requested, &base_quantities, portfolio, Decimal::ZERO)?;
    if let Err(error) = project_executable_targets(&reduced_portfolio, &zero_scale, at) {
        return Err(AdmissionError::InfeasibleRiskReduction(error));
    }
    let mut low = Decimal::ZERO;
    let mut high = Decimal::ONE;
    let mut best = zero_scale;
    for _ in 0..60 {
        let midpoint =
            ((low + high) / Decimal::from(2)).round_dp_with_strategy(18, RoundingStrategy::ToZero);
        let candidate = scaled_targets(&requested, &base_quantities, portfolio, midpoint)?;
        match project_executable_targets(&reduced_portfolio, &candidate, at) {
            Ok(_) => {
                low = midpoint;
                best = candidate;
            }
            Err(error) if is_capacity_error(&error) => high = midpoint,
            Err(error) => return Err(AdmissionError::Arithmetic(error)),
        }
    }
    project_executable_targets(&reduced_portfolio, &best, at)
        .map_err(AdmissionError::InfeasibleRiskReduction)?;
    Ok(adjustments(&requested, &best, low))
}

fn is_capacity_error(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::InsufficientCash
            | EngineError::InsufficientInitialMargin
            | EngineError::InsufficientDeliveryCash
            | EngineError::InsufficientUnderlyingCollateral(_)
    )
}

fn reduction_quantity(current: Decimal, requested: Decimal) -> Decimal {
    if current.is_zero() || current.is_sign_positive() != requested.is_sign_positive() {
        return Decimal::ZERO;
    }
    if requested.abs() < current.abs() {
        requested
    } else {
        current
    }
}

fn scaled_targets(
    requested: &HashMap<String, Decimal>,
    base: &HashMap<String, Decimal>,
    portfolio: &PortfolioState,
    scale: Decimal,
) -> Result<Vec<(String, Decimal)>, AdmissionError> {
    let mut result = Vec::with_capacity(requested.len());
    for (id, target) in requested {
        let start = *base.get(id).expect("base target exists");
        let delta = target
            .checked_sub(start)
            .ok_or_else(|| AdmissionError::InvalidTarget(id.clone()))?;
        let instrument = portfolio
            .instruments()
            .get(id)
            .ok_or_else(|| AdmissionError::UnknownInstrument(id.clone()))?;
        let step = match instrument {
            Instrument::Spot(value) => decimal(value.quantity_step.as_str())?,
            Instrument::LinearFuture(value) => decimal(value.quantity_step.as_str())?,
            Instrument::Option(value) => decimal(value.quantity_step.as_str())?,
        };
        let scaled_delta = delta
            .checked_mul(scale)
            .ok_or_else(|| AdmissionError::InvalidTarget(id.clone()))?;
        let rounded_delta = scaled_delta
            .checked_div(step)
            .ok_or_else(|| AdmissionError::InvalidTarget(id.clone()))?
            .trunc()
            .checked_mul(step)
            .ok_or_else(|| AdmissionError::InvalidTarget(id.clone()))?;
        let admitted = start
            .checked_add(rounded_delta)
            .ok_or_else(|| AdmissionError::InvalidTarget(id.clone()))?;
        result.push((id.clone(), admitted));
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(result)
}

fn adjustments(
    requested: &HashMap<String, Decimal>,
    admitted: &[(String, Decimal)],
    scale: Decimal,
) -> AdmissionResult {
    let admitted = admitted.iter().cloned().collect::<HashMap<_, _>>();
    let mut result = requested
        .iter()
        .map(|(id, requested_quantity)| {
            let admitted_quantity = *admitted.get(id).expect("admission result covers targets");
            AdmissionAdjustment {
                instrument_id: id.clone(),
                requested_quantity: *requested_quantity,
                admitted_quantity,
                common_scale: scale,
                scaled: admitted_quantity != *requested_quantity,
            }
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
    AdmissionResult {
        adjustments: result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{
        instruments::Instrument, run_configuration::RunConfiguration, InstrumentId,
    };
    use factorsbt_data::FixtureRegistry;
    use std::path::Path;

    fn baseline() -> (Vec<Instrument>, PortfolioState) {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
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

    fn target(id: &str, quantity: i64) -> InstrumentTarget {
        InstrumentTarget {
            instrument_id: InstrumentId::new(id).unwrap(),
            target_quantity: Decimal::from(quantity),
        }
    }

    #[test]
    fn scales_the_whole_group_against_shared_cash_independent_of_input_order() {
        let (instruments, portfolio) = baseline();
        assert_eq!(instruments.len(), 2);
        let targets = vec![target("SPOT_A", 1000), target("SPOT_B", 2000)];
        let reversed = vec![targets[1].clone(), targets[0].clone()];
        let first = admit_target_group(&portfolio, &targets).unwrap();
        let second = admit_target_group(&portfolio, &reversed).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.adjustments[0].instrument_id, "SPOT_A");
        assert_eq!(first.adjustments[0].admitted_quantity, Decimal::from(500));
        assert_eq!(first.adjustments[1].instrument_id, "SPOT_B");
        assert_eq!(first.adjustments[1].admitted_quantity, Decimal::from(1000));
        assert!(
            (first.adjustments[0].common_scale - Decimal::new(5, 1)).abs() < Decimal::new(1, 6)
        );
        assert!(first.adjustments.iter().all(|adjustment| adjustment.scaled));
    }

    #[test]
    fn close_admission_includes_fee_and_adverse_tick_price() {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config.instruments[0] = serde_json::from_str(
            r#"{"instrument_id":"SPOT_A","instrument_type":"spot","quote_currency":"USD","price_tick":"0.01","quantity_step":"0.001","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"1","slippage_bps":"10"}"#,
        ).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let portfolio = PortfolioState::initialize(&config, &fixture).unwrap();
        let at = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let admission = admit_target_group_at(&portfolio, &[target("SPOT_A", 1000)], &at).unwrap();
        let adjustment = &admission.adjustments[0];
        assert!(adjustment.scaled);
        assert!(adjustment.admitted_quantity < Decimal::from(990));
        assert!(adjustment.admitted_quantity > Decimal::from(989));
        assert!(project_executable_targets(
            &portfolio,
            &[("SPOT_A".to_owned(), adjustment.admitted_quantity)],
            &at,
        )
        .is_ok());
        assert!(matches!(
            project_executable_targets(
                &portfolio,
                &[("SPOT_A".to_owned(), Decimal::from(1000))],
                &at
            ),
            Err(EngineError::InsufficientCash)
        ));
    }
}
