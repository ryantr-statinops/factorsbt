//! Deterministic end-of-session flattening for the first intraday spot slice.

use crate::{PendingOrderGroup, PortfolioState, SpotExecution};
use factorsbt_contracts::{
    compare_utc_timestamps, instruments::Instrument, HistoricalBar, SessionScheduleEntry,
};
use factorsbt_data::{validate_historical_bar_prices, validate_historical_bar_timestamps};
use rust_decimal::Decimal;
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntradayFlattenError {
    MissingFinalClose(String),
    AmbiguousFinalClose(String),
    UnsupportedPosition(String),
    InvalidClose(String),
    Execution(crate::EngineError),
}

impl fmt::Display for IntradayFlattenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFinalClose(id) => write!(f, "final session close bar is missing for {id}"),
            Self::AmbiguousFinalClose(id) => {
                write!(f, "final session close bar is ambiguous for {id}")
            }
            Self::UnsupportedPosition(id) => {
                write!(f, "intraday flatten supports spot positions only: {id}")
            }
            Self::InvalidClose(id) => write!(f, "final session close bar is invalid for {id}"),
            Self::Execution(error) => write!(f, "session flatten execution failed: {error}"),
        }
    }
}

impl Error for IntradayFlattenError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntradayFlattenResult {
    pub fills: Vec<SpotExecution>,
    pub canceled_order_ids: Vec<String>,
}

/// Apply configured fees/slippage to final-close spot fills atomically. Pending
/// orders are canceled only after all required closing fills succeed.
pub fn flatten_at_session_close(
    portfolio: &mut PortfolioState,
    session: &SessionScheduleEntry,
    bars: &[HistoricalBar],
    pending_groups: &mut Vec<PendingOrderGroup>,
) -> Result<IntradayFlattenResult, IntradayFlattenError> {
    let mut open_ids = portfolio
        .positions()
        .iter()
        .filter(|(_, position)| !position.quantity.is_zero())
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    open_ids.sort();

    let mut final_bars = Vec::new();
    for id in &open_ids {
        if !matches!(portfolio.instruments().get(id), Some(Instrument::Spot(_))) {
            return Err(IntradayFlattenError::UnsupportedPosition(id.clone()));
        }
        let candidates = bars
            .iter()
            .filter(|bar| {
                bar.instrument_id.as_str() == id
                    && compare_utc_timestamps(&bar.interval_end, &session.close_at).is_eq()
            })
            .collect::<Vec<_>>();
        let bar = match candidates.as_slice() {
            [] => return Err(IntradayFlattenError::MissingFinalClose(id.clone())),
            [bar] => *bar,
            _ => return Err(IntradayFlattenError::AmbiguousFinalClose(id.clone())),
        };
        if validate_historical_bar_timestamps(bar).is_err()
            || validate_historical_bar_prices(bar).is_err()
            || compare_utc_timestamps(&bar.interval_start, &session.open_at).is_lt()
        {
            return Err(IntradayFlattenError::InvalidClose(id.clone()));
        }
        final_bars.push((id.clone(), bar));
    }

    let mut candidate = portfolio.clone();
    candidate.begin_group_staging();
    let mut fills = Vec::new();
    for (id, bar) in final_bars {
        let quantity = candidate.position(&id).expect("position exists").quantity;
        let signed_quantity = Decimal::ZERO.checked_sub(quantity).ok_or({
            IntradayFlattenError::Execution(crate::EngineError::ArithmeticOverflow(
                "session flatten quantity",
            ))
        })?;
        let close = Decimal::from_str(bar.close.as_str())
            .map_err(|_| IntradayFlattenError::InvalidClose(id.clone()))?;
        fills.push(
            candidate
                .execute_spot_trade(
                    &id,
                    signed_quantity,
                    close,
                    bar.source_identity.as_str(),
                    bar.source_revision,
                )
                .map_err(IntradayFlattenError::Execution)?,
        );
    }
    candidate
        .finish_group_staging()
        .map_err(IntradayFlattenError::Execution)?;
    *portfolio = candidate;

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
    Ok(IntradayFlattenResult {
        fills,
        canceled_order_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{queue_order_intents, ValidatedOrderIntent};
    use factorsbt_contracts::{
        run_configuration::RunConfiguration, DecimalString, Identifier, InstrumentId, SessionType,
        UtcTimestamp,
    };
    use factorsbt_data::FixtureRegistry;
    use rust_decimal::Decimal;
    use std::path::Path;

    fn configured_portfolio() -> PortfolioState {
        let mut config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        let Instrument::Spot(spot) = &mut config.instruments[0] else {
            unreachable!()
        };
        spot.fee_bps = DecimalString::new("10").unwrap();
        spot.slippage_bps = DecimalString::new("5").unwrap();
        config
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"SPOT_A","quantity":"2"}"#).unwrap());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        PortfolioState::initialize(&config, &fixture).unwrap()
    }

    fn final_bar() -> HistoricalBar {
        HistoricalBar {
            instrument_id: InstrumentId::new("SPOT_A").unwrap(),
            provider_symbol: "TEST".into(),
            interval_start: UtcTimestamp::new("2026-03-09T19:59:00Z").unwrap(),
            interval_end: UtcTimestamp::new("2026-03-09T20:00:00Z").unwrap(),
            available_at: UtcTimestamp::new("2026-03-09T20:00:00Z").unwrap(),
            open: DecimalString::new("100").unwrap(),
            high: DecimalString::new("101").unwrap(),
            low: DecimalString::new("99").unwrap(),
            close: DecimalString::new("100.25").unwrap(),
            volume: DecimalString::new("1000").unwrap(),
            source_identity: Identifier::new("fixture:SPOT_A:close").unwrap(),
            source_revision: 0,
        }
    }

    fn session() -> SessionScheduleEntry {
        SessionScheduleEntry {
            session_date: "2026-03-09".into(),
            open_at: UtcTimestamp::new("2026-03-09T13:30:00Z").unwrap(),
            close_at: UtcTimestamp::new("2026-03-09T20:00:00Z").unwrap(),
            session_type: SessionType::Regular,
        }
    }

    fn pending() -> Vec<PendingOrderGroup> {
        vec![queue_order_intents(
            &Identifier::new("pending").unwrap(),
            &UtcTimestamp::new("2026-03-09T19:58:00Z").unwrap(),
            &UtcTimestamp::new("2026-03-09T19:59:00Z").unwrap(),
            &[ValidatedOrderIntent {
                instrument_id: InstrumentId::new("SPOT_A").unwrap(),
                signed_quantity: Decimal::ONE,
            }],
        )
        .unwrap()]
    }

    #[test]
    fn final_close_flattens_with_costs_and_cancels_pending_orders() {
        let mut portfolio = configured_portfolio();
        let mut pending_groups = pending();
        let result = flatten_at_session_close(
            &mut portfolio,
            &session(),
            &[final_bar()],
            &mut pending_groups,
        )
        .unwrap();
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().quantity,
            Decimal::ZERO
        );
        assert_eq!(result.fills.len(), 1);
        assert!(result.fills[0].fee > Decimal::ZERO);
        assert!(result.fills[0].fill_price < Decimal::new(10025, 2));
        assert_eq!(result.canceled_order_ids, vec!["pending:SPOT_A"]);
        assert!(pending_groups.is_empty());
    }

    #[test]
    fn missing_required_final_close_fails_without_mutation_or_cancellation() {
        let mut portfolio = configured_portfolio();
        let quantity_before = portfolio.position("SPOT_A").unwrap().quantity;
        let mut pending_groups = pending();
        assert_eq!(
            flatten_at_session_close(&mut portfolio, &session(), &[], &mut pending_groups)
                .unwrap_err(),
            IntradayFlattenError::MissingFinalClose("SPOT_A".into()),
        );
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().quantity,
            quantity_before
        );
        assert_eq!(pending_groups.len(), 1);
    }
}
