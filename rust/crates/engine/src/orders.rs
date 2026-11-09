//! Pending decision groups and next-open execution with an actual-price gate.

use crate::{
    admission::{admit_target_group_at, AdmissionAdjustment, AdmissionError, AdmissionResult},
    decisions::{InstrumentTarget, ValidatedOrderIntent},
    marks::{MarkProvenance, MarkSource, ValuationMark},
    portfolio::decimal,
    EngineError, FutureExecution, OptionExecution, PortfolioState, SpotExecution,
};
use factorsbt_contracts::{
    compare_utc_timestamps, instruments::Instrument, Identifier, InstrumentId, UtcTimestamp,
};
use factorsbt_data::ExecutionPrice;
use rust_decimal::Decimal;
use std::{cmp::Ordering, collections::HashMap, error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOrder {
    pub order_id: Identifier,
    pub instrument_id: InstrumentId,
    pub signed_quantity: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOrderGroup {
    pub step_id: Identifier,
    pub decision_time: UtcTimestamp,
    pub eligible_at: UtcTimestamp,
    pub orders: Vec<PendingOrder>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingOrderError {
    InvalidEligibility,
    DuplicateOrder(String),
    MissingPrice(String),
    AmbiguousPrice(String),
    InvalidPrice(String),
    InvalidTarget(String),
    Admission(AdmissionError),
    Execution {
        instrument_id: String,
        source: EngineError,
    },
}

impl fmt::Display for PendingOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEligibility => {
                formatter.write_str("pending group is not eligible at this open")
            }
            Self::DuplicateOrder(id) => {
                write!(formatter, "pending group has duplicate order for {id}")
            }
            Self::MissingPrice(id) => write!(formatter, "no eligible execution price for {id}"),
            Self::AmbiguousPrice(id) => {
                write!(formatter, "multiple execution prices are eligible for {id}")
            }
            Self::InvalidPrice(id) => write!(formatter, "execution price is invalid for {id}"),
            Self::InvalidTarget(id) => write!(formatter, "execution target is invalid for {id}"),
            Self::Admission(error) => write!(formatter, "execution-time admission failed: {error}"),
            Self::Execution {
                instrument_id,
                source,
            } => write!(formatter, "execution failed for {instrument_id}: {source}"),
        }
    }
}

impl Error for PendingOrderError {}

impl From<AdmissionError> for PendingOrderError {
    fn from(value: AdmissionError) -> Self {
        Self::Admission(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionFill {
    Spot(SpotExecution),
    Future(FutureExecution),
    Option(OptionExecution),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedOrderGroup {
    pub step_id: Identifier,
    pub execution_time: UtcTimestamp,
    pub admission_adjustments: Vec<AdmissionAdjustment>,
    pub fills: Vec<ExecutionFill>,
}

/// Convert a completed, capital-admitted target group into orders eligible at
/// the next open. Target reductions are expressed as signed deltas from current
/// positions; zero deltas do not create orders.
pub fn queue_target_group(
    portfolio: &PortfolioState,
    step_id: &Identifier,
    decision_time: &UtcTimestamp,
    eligible_at: &UtcTimestamp,
    admission: &AdmissionResult,
) -> Result<PendingOrderGroup, PendingOrderError> {
    let mut intents = Vec::new();
    for adjustment in &admission.adjustments {
        let current = portfolio
            .position(&adjustment.instrument_id)
            .ok_or_else(|| PendingOrderError::InvalidTarget(adjustment.instrument_id.clone()))?
            .quantity;
        let delta = adjustment
            .admitted_quantity
            .checked_sub(current)
            .ok_or_else(|| PendingOrderError::InvalidTarget(adjustment.instrument_id.clone()))?;
        if !delta.is_zero() {
            intents.push(ValidatedOrderIntent {
                instrument_id: InstrumentId::new(adjustment.instrument_id.clone()).map_err(
                    |_| PendingOrderError::InvalidTarget(adjustment.instrument_id.clone()),
                )?,
                signed_quantity: delta,
            });
        }
    }
    build_group(step_id, decision_time, eligible_at, intents)
}

/// Queue model-provided market intents. Their capital fit is resolved together
/// at the same next-open gate before any fill is applied.
pub fn queue_order_intents(
    step_id: &Identifier,
    decision_time: &UtcTimestamp,
    eligible_at: &UtcTimestamp,
    intents: &[ValidatedOrderIntent],
) -> Result<PendingOrderGroup, PendingOrderError> {
    build_group(step_id, decision_time, eligible_at, intents.to_vec())
}

fn build_group(
    step_id: &Identifier,
    decision_time: &UtcTimestamp,
    eligible_at: &UtcTimestamp,
    intents: Vec<ValidatedOrderIntent>,
) -> Result<PendingOrderGroup, PendingOrderError> {
    if compare_utc_timestamps(eligible_at, decision_time) != Ordering::Greater {
        return Err(PendingOrderError::InvalidEligibility);
    }
    let mut seen = HashMap::new();
    let mut orders = Vec::with_capacity(intents.len());
    for intent in intents {
        let id = intent.instrument_id.as_str().to_owned();
        if intent.signed_quantity.is_zero() || seen.insert(id.clone(), ()).is_some() {
            return Err(PendingOrderError::DuplicateOrder(id));
        }
        let order_id = Identifier::new(format!("{}:{id}", step_id.as_str()))
            .map_err(|_| PendingOrderError::InvalidTarget(id.clone()))?;
        orders.push(PendingOrder {
            order_id,
            instrument_id: intent.instrument_id,
            signed_quantity: intent.signed_quantity,
        });
    }
    orders.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
    });
    Ok(PendingOrderGroup {
        step_id: step_id.clone(),
        decision_time: decision_time.clone(),
        eligible_at: eligible_at.clone(),
        orders,
    })
}

/// Re-evaluate a complete pending group at its eligible open and execute it on
/// an isolated candidate portfolio. A failure leaves the caller's portfolio
/// untouched. Fills use execution inputs only; valuation marks are temporary
/// and are not copied back to the portfolio.
pub fn execute_pending_group(
    portfolio: &mut PortfolioState,
    group: &PendingOrderGroup,
    execution_time: &UtcTimestamp,
    available_prices: &[ExecutionPrice],
) -> Result<ExecutedOrderGroup, PendingOrderError> {
    if compare_utc_timestamps(execution_time, &group.eligible_at) != Ordering::Equal
        || compare_utc_timestamps(&group.eligible_at, &group.decision_time) != Ordering::Greater
    {
        return Err(PendingOrderError::InvalidEligibility);
    }
    let mut price_by_id = HashMap::new();
    let mut seen = HashMap::new();
    let mut planning_marks = Vec::with_capacity(group.orders.len());
    let mut requested_targets = Vec::with_capacity(group.orders.len());
    for order in &group.orders {
        let id = order.instrument_id.as_str();
        if seen.insert(id.to_owned(), ()).is_some() {
            return Err(PendingOrderError::DuplicateOrder(id.to_owned()));
        }
        let price = available_prices
            .iter()
            .filter(|price| {
                price.instrument_id.as_str() == id
                    && compare_utc_timestamps(&price.session_open, execution_time)
                        == Ordering::Equal
                    && compare_utc_timestamps(&price.available_at, execution_time)
                        != Ordering::Greater
            })
            .collect::<Vec<_>>();
        let price = match price.as_slice() {
            [] => return Err(PendingOrderError::MissingPrice(id.to_owned())),
            [price] => *price,
            _ => return Err(PendingOrderError::AmbiguousPrice(id.to_owned())),
        };
        let value = decimal(price.price.as_str())
            .map_err(|_| PendingOrderError::InvalidPrice(id.to_owned()))?;
        if value <= Decimal::ZERO {
            return Err(PendingOrderError::InvalidPrice(id.to_owned()));
        }
        price_by_id.insert(id.to_owned(), price);
        let instrument = portfolio
            .instruments()
            .get(id)
            .ok_or_else(|| PendingOrderError::InvalidTarget(id.to_owned()))?;
        planning_marks.push(ValuationMark {
            instrument_id: order.instrument_id.clone(),
            mark: value,
            source: MarkSource::ExecutionOpen,
            provenance: vec![MarkProvenance {
                observation_id: price.observation_id.as_str().to_owned(),
                source_revision: price.source_revision,
            }],
        });
        let current = portfolio
            .position(id)
            .ok_or_else(|| PendingOrderError::InvalidTarget(id.to_owned()))?
            .quantity;
        let target = current
            .checked_add(order.signed_quantity)
            .ok_or_else(|| PendingOrderError::InvalidTarget(id.to_owned()))?;
        if matches!(instrument, Instrument::Spot(_)) && target < Decimal::ZERO {
            return Err(PendingOrderError::InvalidTarget(id.to_owned()));
        }
        requested_targets.push(InstrumentTarget {
            instrument_id: order.instrument_id.clone(),
            target_quantity: target,
        });
    }

    let mut planning_portfolio = portfolio.clone();
    planning_portfolio
        .apply_valuation_marks(&planning_marks)
        .map_err(|error| PendingOrderError::Execution {
            instrument_id: "<group>".to_owned(),
            source: error,
        })?;
    let admission = admit_target_group_at(&planning_portfolio, &requested_targets, execution_time)?;
    let mut fill_orders = Vec::new();
    for adjustment in &admission.adjustments {
        let current = portfolio
            .position(&adjustment.instrument_id)
            .ok_or_else(|| PendingOrderError::InvalidTarget(adjustment.instrument_id.clone()))?
            .quantity;
        let delta = adjustment
            .admitted_quantity
            .checked_sub(current)
            .ok_or_else(|| PendingOrderError::InvalidTarget(adjustment.instrument_id.clone()))?;
        if !delta.is_zero() {
            fill_orders.push((
                adjustment.instrument_id.clone(),
                adjustment.admitted_quantity,
                delta,
            ));
        }
    }
    fill_orders.sort_by(|left, right| {
        let left_current = portfolio
            .position(&left.0)
            .expect("validated order position")
            .quantity;
        let right_current = portfolio
            .position(&right.0)
            .expect("validated order position")
            .quantity;
        let left_reduces = reduces_exposure(left_current, left.1);
        let right_reduces = reduces_exposure(right_current, right.1);
        right_reduces
            .cmp(&left_reduces)
            .then_with(|| left.0.cmp(&right.0))
    });

    let (candidate, fills) =
        stage_group_fills(portfolio, fill_orders, &price_by_id, execution_time)?;
    *portfolio = candidate;

    Ok(ExecutedOrderGroup {
        step_id: group.step_id.clone(),
        execution_time: execution_time.clone(),
        admission_adjustments: admission.adjustments,
        fills,
    })
}

fn stage_group_fills(
    portfolio: &PortfolioState,
    fill_orders: Vec<(String, Decimal, Decimal)>,
    price_by_id: &HashMap<String, &ExecutionPrice>,
    execution_time: &UtcTimestamp,
) -> Result<(PortfolioState, Vec<ExecutionFill>), PendingOrderError> {
    let mut candidate = portfolio.clone();
    candidate.begin_group_staging();
    let mut fills = Vec::new();
    for (id, _, quantity) in fill_orders {
        let price = price_by_id
            .get(&id)
            .expect("every order price was validated");
        let quote = decimal(price.price.as_str())
            .map_err(|_| PendingOrderError::InvalidPrice(id.clone()))?;
        let fill = match candidate.instruments().get(&id) {
            Some(Instrument::Spot(_)) => candidate
                .execute_spot_trade(
                    &id,
                    quantity,
                    quote,
                    price.observation_id.as_str(),
                    price.source_revision,
                )
                .map(ExecutionFill::Spot),
            Some(Instrument::LinearFuture(_)) => candidate
                .execute_future_trade(
                    &id,
                    quantity,
                    quote,
                    price.observation_id.as_str(),
                    price.source_revision,
                )
                .map(ExecutionFill::Future),
            Some(Instrument::Option(_)) => candidate
                .execute_option_trade(
                    &id,
                    quantity,
                    quote,
                    execution_time,
                    price.observation_id.as_str(),
                    price.source_revision,
                )
                .map(ExecutionFill::Option),
            None => Err(EngineError::MissingPosition(id.clone())),
        }
        .map_err(|source| PendingOrderError::Execution {
            instrument_id: id,
            source,
        })?;
        fills.push(fill);
    }
    candidate
        .finish_group_staging()
        .map_err(|source| PendingOrderError::Execution {
            instrument_id: "<group>".to_owned(),
            source,
        })?;
    Ok((candidate, fills))
}

/// Evaluate a target group with the same fill, fee, ledger, and final-capacity
/// rules as execution, using the portfolio's current marks as provisional quotes.
/// No fill or portfolio mutation escapes this projection.
pub(crate) fn project_executable_targets(
    portfolio: &PortfolioState,
    targets: &[(String, Decimal)],
    at: &UtcTimestamp,
) -> Result<PortfolioState, EngineError> {
    let mut ordered = targets.to_vec();
    ordered.sort_by(|left, right| {
        let left_current = portfolio
            .position(&left.0)
            .map(|position| position.quantity);
        let right_current = portfolio
            .position(&right.0)
            .map(|position| position.quantity);
        match (left_current, right_current) {
            (Some(left_current), Some(right_current)) => reduces_exposure(right_current, right.1)
                .cmp(&reduces_exposure(left_current, left.1))
                .then_with(|| left.0.cmp(&right.0)),
            _ => left.0.cmp(&right.0),
        }
    });
    let mut candidate = portfolio.clone();
    candidate.begin_group_staging();
    for (id, target) in ordered {
        let current = candidate
            .position(&id)
            .ok_or_else(|| EngineError::MissingPosition(id.clone()))?;
        let delta = target
            .checked_sub(current.quantity)
            .ok_or(EngineError::ArithmeticOverflow("target quantity delta"))?;
        if delta.is_zero() {
            continue;
        }
        let quote = current.mark;
        match candidate.instruments().get(&id) {
            Some(Instrument::Spot(_)) => {
                candidate.execute_spot_trade(&id, delta, quote, "admission-quote", 0)?;
            }
            Some(Instrument::LinearFuture(_)) => {
                candidate.execute_future_trade(&id, delta, quote, "admission-quote", 0)?;
            }
            Some(Instrument::Option(_)) => {
                candidate.execute_option_trade(&id, delta, quote, at, "admission-quote", 0)?;
            }
            None => return Err(EngineError::MissingPosition(id)),
        }
    }
    candidate.finish_group_staging()?;
    Ok(candidate)
}

fn reduces_exposure(current: Decimal, target: Decimal) -> bool {
    !current.is_zero()
        && (target.is_zero()
            || current.is_sign_positive() != target.is_sign_positive()
            || target.abs() < current.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{instruments::Instrument, run_configuration::RunConfiguration};
    use factorsbt_data::{execution_prices_available_at, FixtureRegistry};
    use std::path::Path;

    fn baseline() -> (PortfolioState, factorsbt_data::Fixture) {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config.instruments.push(serde_json::from_str(
            r#"{"instrument_id":"SPOT_B","instrument_type":"spot","quote_currency":"USD","price_tick":"0.01","quantity_step":"0.001","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0"}"#,
        ).unwrap());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = (*registry.get_by("mvp-synthetic", "v1").unwrap()).clone();
        let portfolio = PortfolioState::initialize(&config, &fixture).unwrap();
        assert!(matches!(
            portfolio.instruments().get("SPOT_A"),
            Some(Instrument::Spot(_))
        ));
        (portfolio, fixture)
    }

    fn admitted_targets() -> AdmissionResult {
        AdmissionResult {
            adjustments: vec![
                AdmissionAdjustment {
                    instrument_id: "SPOT_A".to_owned(),
                    requested_quantity: Decimal::from(1000),
                    admitted_quantity: Decimal::from(1000),
                    common_scale: Decimal::ONE,
                    scaled: false,
                },
                AdmissionAdjustment {
                    instrument_id: "SPOT_B".to_owned(),
                    requested_quantity: Decimal::from(2000),
                    admitted_quantity: Decimal::from(2000),
                    common_scale: Decimal::ONE,
                    scaled: false,
                },
            ],
        }
    }

    fn covered_call_portfolio() -> (PortfolioState, factorsbt_data::Fixture) {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config.instruments.push(serde_json::from_str(
            r#"{"instrument_type":"option","instrument_id":"AM_CALL_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"100","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","option_type":"call","strike":"100","expiry_at":"2026-01-09T16:00:00Z","exercise_style":"american","settlement_style":"physical","valuation_mode":"quote","initial_margin_per_contract":"600","maintenance_margin_per_contract":"500"}"#,
        ).unwrap());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = (*registry.get_by("mvp-settle-01", "v1").unwrap()).clone();
        (
            PortfolioState::initialize(&config, &fixture).unwrap(),
            fixture,
        )
    }

    fn intent(id: &str, quantity: i64) -> ValidatedOrderIntent {
        ValidatedOrderIntent {
            instrument_id: InstrumentId::new(id).unwrap(),
            signed_quantity: Decimal::from(quantity),
        }
    }

    #[test]
    fn covered_call_group_opens_and_closes_regardless_of_input_order() {
        let (initial, fixture) = covered_call_portfolio();
        let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let prices = execution_prices_available_at(&fixture.dataset, &open).unwrap();
        let step = Identifier::new("covered-call").unwrap();
        let opening = vec![intent("AM_CALL_A", -1), intent("SPOT_A", 100)];
        let mut reversed = opening.clone();
        reversed.reverse();
        let mut results = Vec::new();
        for intents in [opening, reversed] {
            let mut portfolio = initial.clone();
            let group = queue_order_intents(&step, &close, &open, &intents).unwrap();
            let executed = execute_pending_group(&mut portfolio, &group, &open, &prices).unwrap();
            assert_eq!(executed.fills.len(), 2);
            assert_eq!(
                portfolio.position("SPOT_A").unwrap().quantity,
                Decimal::from(100)
            );
            assert_eq!(
                portfolio.position("AM_CALL_A").unwrap().quantity,
                Decimal::from(-1)
            );
            assert_eq!(
                portfolio.reserved_underlying()["SPOT_A"],
                Decimal::from(100)
            );

            let closing = vec![intent("SPOT_A", -100), intent("AM_CALL_A", 1)];
            let close_group = queue_order_intents(&step, &close, &open, &closing).unwrap();
            let closed =
                execute_pending_group(&mut portfolio, &close_group, &open, &prices).unwrap();
            assert_eq!(closed.fills.len(), 2);
            assert_eq!(
                portfolio.position("SPOT_A").unwrap().quantity,
                Decimal::ZERO
            );
            assert_eq!(
                portfolio.position("AM_CALL_A").unwrap().quantity,
                Decimal::ZERO
            );
            results.push((executed.fills, closed.fills, portfolio.cash()));
        }
        assert_eq!(results[0], results[1]);
    }

    #[test]
    fn invalid_group_collateral_rolls_back_all_staged_legs() {
        let (portfolio, fixture) = covered_call_portfolio();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let prices = execution_prices_available_at(&fixture.dataset, &open).unwrap();
        let price_by_id = prices
            .iter()
            .map(|price| (price.instrument_id.as_str().to_owned(), price))
            .collect::<HashMap<_, _>>();
        let result = stage_group_fills(
            &portfolio,
            vec![("AM_CALL_A".to_owned(), Decimal::from(-1), Decimal::from(-1))],
            &price_by_id,
            &open,
        );
        assert!(matches!(
            result,
            Err(PendingOrderError::Execution {
                source: EngineError::InsufficientUnderlyingCollateral(_),
                ..
            })
        ));
        assert_eq!(portfolio.cash(), Decimal::from(100_000));
        assert_eq!(
            portfolio.position("AM_CALL_A").unwrap().quantity,
            Decimal::ZERO
        );
    }

    #[test]
    fn fee_and_gap_reduce_next_open_quantity_before_any_fill_is_published() {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config.instruments[0] = serde_json::from_str(
            r#"{"instrument_id":"SPOT_A","instrument_type":"spot","quote_currency":"USD","price_tick":"0.01","quantity_step":"0.001","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"1","slippage_bps":"0"}"#,
        ).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let mut portfolio = PortfolioState::initialize(&config, &fixture).unwrap();
        let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let target = InstrumentTarget {
            instrument_id: InstrumentId::new("SPOT_A").unwrap(),
            target_quantity: Decimal::from(1000),
        };
        let close_admission = admit_target_group_at(&portfolio, &[target], &close).unwrap();
        assert_eq!(
            close_admission.adjustments[0].admitted_quantity,
            Decimal::new(990099, 3)
        );
        let group = queue_target_group(
            &portfolio,
            &Identifier::new("fee-gap").unwrap(),
            &close,
            &open,
            &close_admission,
        )
        .unwrap();
        let prices = execution_prices_available_at(&fixture.dataset, &open).unwrap();
        let result = execute_pending_group(&mut portfolio, &group, &open, &prices).unwrap();
        assert_eq!(
            result.admission_adjustments[0].admitted_quantity,
            Decimal::new(985221, 3)
        );
        assert!(result.admission_adjustments[0].scaled);
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().quantity,
            Decimal::new(985221, 3)
        );
        assert!(portfolio.cash() >= Decimal::ZERO);
        assert_eq!(result.fills.len(), 1);
    }

    #[test]
    fn queues_for_the_next_open_and_rechecks_shared_cash_at_gap_prices() {
        let (mut portfolio, fixture) = baseline();
        let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let step = Identifier::new("step-0001").unwrap();
        let group =
            queue_target_group(&portfolio, &step, &close, &open, &admitted_targets()).unwrap();
        assert_eq!(group.orders.len(), 2);
        let prices = execution_prices_available_at(&fixture.dataset, &open).unwrap();
        let result = execute_pending_group(&mut portfolio, &group, &open, &prices).unwrap();
        assert_eq!(result.fills.len(), 2);
        assert!(portfolio.cash() >= Decimal::ZERO);
        assert!(portfolio.position("SPOT_A").unwrap().quantity < Decimal::from(1000));
        assert!(portfolio.position("SPOT_B").unwrap().quantity < Decimal::from(2000));
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().mark,
            Decimal::from(100)
        );
    }

    #[test]
    fn missing_price_or_early_execution_leaves_portfolio_unchanged() {
        let (mut portfolio, _) = baseline();
        let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let step = Identifier::new("step-0001").unwrap();
        let group =
            queue_target_group(&portfolio, &step, &close, &open, &admitted_targets()).unwrap();
        let early = execute_pending_group(&mut portfolio, &group, &close, &[]);
        assert!(matches!(early, Err(PendingOrderError::InvalidEligibility)));
        assert_eq!(portfolio.cash(), Decimal::from(100000));
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().quantity,
            Decimal::ZERO
        );
        let missing = execute_pending_group(&mut portfolio, &group, &open, &[]);
        assert!(matches!(missing, Err(PendingOrderError::MissingPrice(_))));
        assert_eq!(portfolio.cash(), Decimal::from(100000));
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().quantity,
            Decimal::ZERO
        );
    }
}
