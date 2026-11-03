//! Whole-portfolio next-open liquidation and account recovery checks.

use crate::{
    orders::{ExecutionFill, PendingOrderGroup},
    portfolio::decimal,
    EngineError, PortfolioState,
};
use factorsbt_contracts::{
    compare_utc_timestamps, instruments::Instrument, Identifier, InstrumentId, UtcTimestamp,
};
use factorsbt_data::ExecutionPrice;
use rust_decimal::Decimal;
use std::{cmp::Ordering, collections::HashMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiquidationOutcome {
    Completed {
        step_id: Identifier,
        execution_time: UtcTimestamp,
        fills: Vec<ExecutionFill>,
    },
    Failed {
        step_id: Identifier,
        execution_time: UtcTimestamp,
        cash_before: Decimal,
        reason: String,
        instrument_id: Option<String>,
    },
}

pub fn schedule_liquidation(
    portfolio: &PortfolioState,
    step_id: &Identifier,
    decision_time: &UtcTimestamp,
    eligible_at: &UtcTimestamp,
) -> Result<PendingOrderGroup, crate::PendingOrderError> {
    let mut intents = Vec::new();
    let mut ids = portfolio.positions().keys().cloned().collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        let quantity = portfolio
            .position(&id)
            .expect("position key exists")
            .quantity;
        if quantity.is_zero() {
            continue;
        }
        let close_quantity = Decimal::ZERO
            .checked_sub(quantity)
            .ok_or_else(|| crate::PendingOrderError::InvalidTarget(id.clone()))?;
        intents.push(crate::ValidatedOrderIntent {
            instrument_id: InstrumentId::new(id.clone())
                .map_err(|_| crate::PendingOrderError::InvalidTarget(id.clone()))?,
            signed_quantity: close_quantity,
        });
    }
    crate::queue_order_intents(step_id, decision_time, eligible_at, &intents)
}

/// Execute exact close quantities at the eligible open. Failure preserves the
/// original portfolio; no price or recovery cash is synthesized.
pub fn execute_liquidation_group(
    portfolio: &mut PortfolioState,
    group: &PendingOrderGroup,
    execution_time: &UtcTimestamp,
    available_prices: &[ExecutionPrice],
) -> Result<LiquidationOutcome, crate::PendingOrderError> {
    let cash_before = portfolio.cash();
    let fail = |reason: String, instrument_id: Option<String>| LiquidationOutcome::Failed {
        step_id: group.step_id.clone(),
        execution_time: execution_time.clone(),
        cash_before,
        reason,
        instrument_id,
    };
    if compare_utc_timestamps(&group.eligible_at, execution_time) != Ordering::Equal {
        return Err(crate::PendingOrderError::InvalidEligibility);
    }
    if !is_exact_full_close(portfolio, group) {
        return Err(crate::PendingOrderError::InvalidTarget(
            "liquidation must close every open position exactly".to_owned(),
        ));
    }

    let mut prices = HashMap::new();
    for order in &group.orders {
        let id = order.instrument_id.as_str();
        let candidates = available_prices
            .iter()
            .filter(|price| {
                price.instrument_id.as_str() == id
                    && compare_utc_timestamps(&price.session_open, execution_time)
                        == Ordering::Equal
                    && compare_utc_timestamps(&price.available_at, execution_time)
                        != Ordering::Greater
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => {
                return Ok(fail(
                    format!("missing eligible liquidation open for {id}"),
                    Some(id.to_owned()),
                ))
            }
            [price] => {
                let value = decimal(price.price.as_str()).map_err(|source| {
                    crate::PendingOrderError::Execution {
                        instrument_id: id.to_owned(),
                        source,
                    }
                })?;
                if value <= Decimal::ZERO {
                    return Ok(fail(
                        "liquidation price must be positive".to_owned(),
                        Some(id.to_owned()),
                    ));
                }
                prices.insert(id.to_owned(), *price);
            }
            _ => return Err(crate::PendingOrderError::AmbiguousPrice(id.to_owned())),
        }
    }

    let mut candidate = portfolio.clone();
    let mut fills = Vec::new();
    for order in &group.orders {
        let id = order.instrument_id.as_str();
        let price = prices.get(id).expect("all liquidation prices were checked");
        let quote = decimal(price.price.as_str()).map_err(|source| {
            crate::PendingOrderError::Execution {
                instrument_id: id.to_owned(),
                source,
            }
        })?;
        let result = match candidate.instruments().get(id) {
            Some(Instrument::Spot(_)) => candidate
                .execute_spot_trade(
                    id,
                    order.signed_quantity,
                    quote,
                    price.observation_id.as_str(),
                    price.source_revision,
                )
                .map(ExecutionFill::Spot),
            Some(Instrument::LinearFuture(_)) => candidate
                .execute_future_trade(
                    id,
                    order.signed_quantity,
                    quote,
                    price.observation_id.as_str(),
                    price.source_revision,
                )
                .map(ExecutionFill::Future),
            Some(Instrument::Option(_)) => candidate
                .execute_option_trade(
                    id,
                    order.signed_quantity,
                    quote,
                    execution_time,
                    price.observation_id.as_str(),
                    price.source_revision,
                )
                .map(ExecutionFill::Option),
            None => Err(EngineError::MissingPosition(id.to_owned())),
        };
        match result {
            Ok(fill) => fills.push(fill),
            Err(error) => return Ok(fail(error.to_string(), Some(id.to_owned()))),
        }
    }
    if candidate
        .positions()
        .values()
        .any(|position| !position.quantity.is_zero())
    {
        return Ok(fail("liquidation left an open position".to_owned(), None));
    }
    if let Err(error) = candidate.validate_initial_capacity() {
        return Ok(fail(error.to_string(), None));
    }

    *portfolio = candidate;
    Ok(LiquidationOutcome::Completed {
        step_id: group.step_id.clone(),
        execution_time: execution_time.clone(),
        fills,
    })
}

fn is_exact_full_close(portfolio: &PortfolioState, group: &PendingOrderGroup) -> bool {
    let mut expected = portfolio
        .positions()
        .iter()
        .filter_map(|(id, position)| {
            (!position.quantity.is_zero()).then_some((id.clone(), position.quantity))
        })
        .collect::<HashMap<_, _>>();
    for order in &group.orders {
        let id = order.instrument_id.as_str();
        let Some(current) = expected.remove(id) else {
            return false;
        };
        if current.checked_add(order.signed_quantity) != Some(Decimal::ZERO) {
            return false;
        }
    }
    expected.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{run_configuration::RunConfiguration, Identifier, InstrumentId};
    use factorsbt_data::FixtureRegistry;
    use std::path::Path;

    fn distressed_future_account() -> PortfolioState {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config.instruments.push(serde_json::from_str(
            r#"{"instrument_id":"FUT_A","instrument_type":"linear_future","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","expiry_at":"2026-01-09T16:00:00Z","initial_margin_per_contract":"1000","maintenance_margin_per_contract":"800"}"#,
        ).unwrap());
        config
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"FUT_A","quantity":"1"}"#).unwrap());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let mut portfolio = PortfolioState::initialize(&config, &fixture).unwrap();
        portfolio
            .apply_ledger_posting(None, Decimal::from(-100_050), Decimal::ZERO)
            .unwrap();
        portfolio
    }

    fn open_price() -> ExecutionPrice {
        ExecutionPrice {
            instrument_id: InstrumentId::new("FUT_A").unwrap(),
            session_open: UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap(),
            price: factorsbt_contracts::DecimalString::new("200").unwrap(),
            available_at: UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap(),
            observation_id: Identifier::new("LIQ-FUT-A-OPEN").unwrap(),
            source_revision: 1,
        }
    }

    fn plan(portfolio: &PortfolioState) -> PendingOrderGroup {
        schedule_liquidation(
            portfolio,
            &Identifier::new("liq-step-0001").unwrap(),
            &UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap(),
            &UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn liquidation_uses_real_open_variation_and_resumes_only_when_valid() {
        let mut portfolio = distressed_future_account();
        let group = plan(&portfolio);
        let outcome = execute_liquidation_group(
            &mut portfolio,
            &group,
            &UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap(),
            &[open_price()],
        )
        .unwrap();
        assert!(matches!(outcome, LiquidationOutcome::Completed { .. }));
        assert_eq!(portfolio.position("FUT_A").unwrap().quantity, Decimal::ZERO);
        assert_eq!(portfolio.cash(), Decimal::from(50));
    }

    #[test]
    fn missing_open_cannot_invent_a_fill_or_change_the_last_valid_state() {
        let mut portfolio = distressed_future_account();
        let group = plan(&portfolio);
        let outcome = execute_liquidation_group(
            &mut portfolio,
            &group,
            &UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap(),
            &[],
        )
        .unwrap();
        assert!(matches!(outcome, LiquidationOutcome::Failed { .. }));
        assert_eq!(portfolio.position("FUT_A").unwrap().quantity, Decimal::ONE);
        assert_eq!(portfolio.cash(), Decimal::from(-50));
    }
}
