//! End-of-run cancellation and last-valid-state summary.

use crate::{PendingOrderGroup, PortfolioState, RiskError};
use rust_decimal::Decimal;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalRunStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPositionSummary {
    pub instrument_id: String,
    pub quantity: Decimal,
    pub mark: Decimal,
    pub market_value: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFinalization {
    pub status: FinalRunStatus,
    pub failure_reason: Option<String>,
    pub cash: Decimal,
    pub nav: Decimal,
    pub canceled_order_ids: Vec<String>,
    pub open_positions: Vec<OpenPositionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizationError {
    Portfolio(RiskError),
    Position(crate::EngineError),
}

impl fmt::Display for FinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Portfolio(error) => {
                write!(formatter, "final run NAV could not be computed: {error}")
            }
            Self::Position(error) => write!(
                formatter,
                "final position summary could not be computed: {error}"
            ),
        }
    }
}

impl Error for FinalizationError {}

/// Cancel all unfilled orders, retain the exact current ledger, and report open
/// positions. A previously failed run stays failed; remaining positions alone
/// do not synthesize closing fills or turn a valid run into a failure.
pub fn finalize_run_state(
    portfolio: &PortfolioState,
    pending_groups: &mut Vec<PendingOrderGroup>,
    prior_failure: Option<String>,
) -> Result<RunFinalization, FinalizationError> {
    let nav = portfolio
        .net_asset_value()
        .map_err(RiskError::from)
        .map_err(FinalizationError::Portfolio)?;
    let mut open_positions = Vec::new();
    for (instrument_id, position) in portfolio.positions() {
        if position.quantity.is_zero() {
            continue;
        }
        let market_value = portfolio
            .position_market_value(instrument_id)
            .map_err(FinalizationError::Position)?;
        open_positions.push(OpenPositionSummary {
            instrument_id: instrument_id.clone(),
            quantity: position.quantity,
            mark: position.mark,
            market_value,
        });
    }
    open_positions.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
    let mut canceled_order_ids = pending_groups
        .drain(..)
        .flat_map(|group| {
            group
                .orders
                .into_iter()
                .map(|order| order.order_id.as_str().to_owned())
        })
        .collect::<Vec<_>>();
    canceled_order_ids.sort();
    Ok(RunFinalization {
        status: if prior_failure.is_some() {
            FinalRunStatus::Failed
        } else {
            FinalRunStatus::Succeeded
        },
        failure_reason: prior_failure,
        cash: portfolio.cash(),
        nav,
        canceled_order_ids,
        open_positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{queue_order_intents, ValidatedOrderIntent};
    use factorsbt_contracts::{
        run_configuration::RunConfiguration, Identifier, InstrumentId, UtcTimestamp,
    };
    use factorsbt_data::FixtureRegistry;
    use rust_decimal::Decimal;
    use std::path::Path;

    fn portfolio_with_open_spot() -> PortfolioState {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        config
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"SPOT_A","quantity":"2"}"#).unwrap());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        PortfolioState::initialize(&config, &fixture).unwrap()
    }

    #[test]
    fn end_of_run_cancels_unfilled_orders_and_reports_positions_without_fills() {
        let portfolio = portfolio_with_open_spot();
        let step = Identifier::new("step-0001").unwrap();
        let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let intent = ValidatedOrderIntent {
            instrument_id: InstrumentId::new("SPOT_A").unwrap(),
            signed_quantity: Decimal::ONE,
        };
        let group = queue_order_intents(&step, &close, &open, &[intent]).unwrap();
        let mut pending = vec![group];
        let finalization = finalize_run_state(&portfolio, &mut pending, None).unwrap();
        assert_eq!(finalization.status, FinalRunStatus::Succeeded);
        assert_eq!(finalization.canceled_order_ids, vec!["step-0001:SPOT_A"]);
        assert!(pending.is_empty());
        assert_eq!(finalization.open_positions.len(), 1);
        assert_eq!(finalization.open_positions[0].quantity, Decimal::from(2));
        assert_eq!(
            finalization.open_positions[0].market_value,
            Decimal::from(200)
        );
        assert_eq!(finalization.nav, Decimal::from(100_200));
    }

    #[test]
    fn failed_liquidation_reason_survives_end_of_run_finalization() {
        let portfolio = portfolio_with_open_spot();
        let mut pending = Vec::new();
        let finalization = finalize_run_state(
            &portfolio,
            &mut pending,
            Some("liquidation_failed".to_owned()),
        )
        .unwrap();
        assert_eq!(finalization.status, FinalRunStatus::Failed);
        assert_eq!(
            finalization.failure_reason.as_deref(),
            Some("liquidation_failed")
        );
        assert_eq!(finalization.open_positions[0].instrument_id, "SPOT_A");
    }
}
