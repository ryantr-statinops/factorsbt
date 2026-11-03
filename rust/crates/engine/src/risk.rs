//! Close-time maintenance assessment and pending-order cancellation.

use crate::{EngineError, PendingOrderGroup, PortfolioState};
use rust_decimal::Decimal;
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceAssessment {
    pub cash: Decimal,
    pub nav: Decimal,
    pub maintenance_requirement: Decimal,
    pub maintenance_cash_requirement: Decimal,
    pub negative_cash: bool,
    pub cash_collateral_shortfall: bool,
    pub nav_shortfall: bool,
    pub underlying_shortfalls: Vec<(String, Decimal, Decimal)>,
}

impl MaintenanceAssessment {
    pub fn is_breach(&self) -> bool {
        self.negative_cash
            || self.cash_collateral_shortfall
            || self.nav_shortfall
            || !self.underlying_shortfalls.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceBreach {
    pub assessment: MaintenanceAssessment,
    pub canceled_order_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskError {
    Portfolio(EngineError),
}

impl fmt::Display for RiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Portfolio(error) => write!(formatter, "maintenance assessment failed: {error}"),
        }
    }
}

impl Error for RiskError {}

impl From<EngineError> for RiskError {
    fn from(value: EngineError) -> Self {
        Self::Portfolio(value)
    }
}

pub fn assess_maintenance(portfolio: &PortfolioState) -> Result<MaintenanceAssessment, RiskError> {
    let nav = portfolio.net_asset_value()?;
    let underlying_shortfalls = portfolio
        .reserved_underlying()
        .iter()
        .filter_map(|(instrument_id, required)| {
            let available = portfolio
                .position(instrument_id)
                .map(|position| position.quantity)
                .unwrap_or(Decimal::ZERO);
            (available < *required).then(|| (instrument_id.clone(), *required, available))
        })
        .collect::<Vec<_>>();
    Ok(MaintenanceAssessment {
        cash: portfolio.cash(),
        nav,
        maintenance_requirement: portfolio.maintenance_margin_requirement(),
        maintenance_cash_requirement: portfolio.maintenance_cash_encumbrance(),
        negative_cash: portfolio.cash() < Decimal::ZERO,
        cash_collateral_shortfall: portfolio.cash() < portfolio.maintenance_cash_encumbrance(),
        nav_shortfall: nav < portfolio.maintenance_cash_encumbrance(),
        underlying_shortfalls,
    })
}

/// If any accepted maintenance/collateral condition is breached, clear all
/// pending groups and return every canceled order identity. The caller schedules
/// whole-portfolio liquidation for the next synthetic open.
pub fn enforce_maintenance(
    portfolio: &PortfolioState,
    pending_groups: &mut Vec<PendingOrderGroup>,
) -> Result<Option<MaintenanceBreach>, RiskError> {
    let assessment = assess_maintenance(portfolio)?;
    if !assessment.is_breach() {
        return Ok(None);
    }
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
    Ok(Some(MaintenanceBreach {
        assessment,
        canceled_order_ids,
    }))
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

    fn portfolio() -> PortfolioState {
        let config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        PortfolioState::initialize(&config, &fixture).unwrap()
    }

    #[test]
    fn healthy_account_keeps_pending_orders() {
        let mut state = portfolio();
        let step = Identifier::new("step-0001").unwrap();
        let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
        let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
        let intent = ValidatedOrderIntent {
            instrument_id: InstrumentId::new("SPOT_A").unwrap(),
            signed_quantity: Decimal::ONE,
        };
        let group = queue_order_intents(&step, &close, &open, &[intent]).unwrap();
        let mut pending = vec![group];
        assert!(enforce_maintenance(&state, &mut pending).unwrap().is_none());
        assert_eq!(pending.len(), 1);

        state
            .apply_ledger_posting(None, Decimal::from(-100_001), Decimal::ZERO)
            .unwrap();
        let breach = enforce_maintenance(&state, &mut pending).unwrap().unwrap();
        assert!(breach.assessment.negative_cash);
        assert_eq!(breach.canceled_order_ids, vec!["step-0001:SPOT_A"]);
        assert!(pending.is_empty());
    }
}
