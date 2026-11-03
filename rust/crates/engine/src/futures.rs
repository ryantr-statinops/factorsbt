//! Linear futures fills and variation settlement.

use crate::{
    portfolio::{decimal, validate_quantity},
    EngineError, LedgerPosting, PortfolioState,
};
use factorsbt_contracts::{
    compare_utc_timestamps, instruments::Instrument, InstrumentId, UtcTimestamp,
};
use rust_decimal::{Decimal, RoundingStrategy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureExecution {
    pub instrument_id: InstrumentId,
    pub signed_quantity: Decimal,
    pub fill_price: Decimal,
    pub fee: Decimal,
    pub realized_variation: Decimal,
    pub trade_posting: LedgerPosting,
    pub fee_posting: LedgerPosting,
    pub source_observation_id: String,
    pub source_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureSettlement {
    pub instrument_id: InstrumentId,
    pub settled_quantity: Decimal,
    pub settlement_price: Decimal,
    pub variation_profit_loss: Decimal,
    pub posting: LedgerPosting,
    pub expired: bool,
    pub source_observation_id: String,
    pub source_revision: u64,
}

impl PortfolioState {
    /// Execute a futures order without exchanging notional principal. Opposing
    /// quantity first closes existing exposure, then any remainder opens the
    /// reversed position at this fill price.
    pub fn execute_future_trade(
        &mut self,
        instrument_id: &str,
        signed_quantity: Decimal,
        quote_price: Decimal,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<FutureExecution, EngineError> {
        let configured = self
            .instruments()
            .get(instrument_id)
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::LinearFuture(instrument) = configured else {
            return Err(EngineError::NotFutureInstrument(instrument_id.to_owned()));
        };
        if signed_quantity.is_zero()
            || validate_quantity(
                &Instrument::LinearFuture(instrument.clone()),
                signed_quantity,
            )
            .is_err()
            || quote_price <= Decimal::ZERO
        {
            return Err(EngineError::InvalidFutureTrade(instrument_id.to_owned()));
        }

        let fill_price = adverse_fill_price(
            quote_price,
            signed_quantity,
            decimal(instrument.price_tick.as_str())?,
            decimal(instrument.slippage_bps.as_str())?,
        )?;
        let current = self
            .position(instrument_id)
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let current_quantity = current.quantity;
        let current_basis = current.settlement_basis;
        let multiplier = decimal(instrument.contract_multiplier.as_str())?;
        let closed_quantity =
            if current_quantity.is_zero() || same_sign(current_quantity, signed_quantity) {
                Decimal::ZERO
            } else {
                magnitude(current_quantity)?.min(magnitude(signed_quantity)?)
            };
        let signed_closed_position = if current_quantity < Decimal::ZERO {
            Decimal::ZERO
                .checked_sub(closed_quantity)
                .ok_or(EngineError::ArithmeticOverflow("futures close quantity"))?
        } else {
            closed_quantity
        };
        let realized_variation = if closed_quantity.is_zero() {
            Decimal::ZERO
        } else {
            let basis = current_basis
                .ok_or_else(|| EngineError::InvalidFutureTrade(instrument_id.to_owned()))?;
            signed_closed_position
                .checked_mul(multiplier)
                .and_then(|value| value.checked_mul(fill_price - basis))
                .ok_or(EngineError::ArithmeticOverflow("futures close variation"))?
        };

        let fee = calculate_fee(
            signed_quantity,
            fill_price,
            multiplier,
            decimal(instrument.fee_bps.as_str())?,
            decimal(instrument.fee_per_unit.as_str())?,
        )?;
        let mut candidate = self.clone();
        let trade_posting = candidate.apply_ledger_posting(
            Some(instrument_id),
            realized_variation,
            signed_quantity,
        )?;
        let fee_delta = Decimal::ZERO
            .checked_sub(fee)
            .ok_or(EngineError::ArithmeticOverflow("futures fee delta"))?;
        let fee_posting = candidate.apply_ledger_posting(None, fee_delta, Decimal::ZERO)?;
        let next_quantity = current_quantity
            .checked_add(signed_quantity)
            .ok_or(EngineError::ArithmeticOverflow("futures position"))?;
        let next_basis = settlement_basis_after_trade(
            current_quantity,
            current_basis,
            signed_quantity,
            next_quantity,
            fill_price,
        )?;
        let position = candidate
            .position_mut(instrument_id)
            .expect("configured position exists");
        position.settlement_basis = next_basis;
        candidate.refresh_margin_requirements()?;
        *self = candidate;

        Ok(FutureExecution {
            instrument_id: instrument.instrument_id,
            signed_quantity,
            fill_price,
            fee,
            realized_variation,
            trade_posting,
            fee_posting,
            source_observation_id: source_observation_id.into(),
            source_revision,
        })
    }

    pub fn settle_future_daily(
        &mut self,
        instrument_id: &str,
        settlement_price: Decimal,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<FutureSettlement, EngineError> {
        self.settle_future(
            instrument_id,
            settlement_price,
            false,
            None,
            source_observation_id.into(),
            source_revision,
        )
    }

    pub fn settle_future_expiry(
        &mut self,
        instrument_id: &str,
        event_time: &UtcTimestamp,
        settlement_price: Decimal,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<FutureSettlement, EngineError> {
        let configured = self
            .instruments()
            .get(instrument_id)
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::LinearFuture(instrument) = configured else {
            return Err(EngineError::NotFutureInstrument(instrument_id.to_owned()));
        };
        if compare_utc_timestamps(event_time, &instrument.expiry_at).is_lt() {
            return Err(EngineError::FutureNotExpired(instrument_id.to_owned()));
        }
        self.settle_future(
            instrument_id,
            settlement_price,
            true,
            Some(event_time.clone()),
            source_observation_id.into(),
            source_revision,
        )
    }

    fn settle_future(
        &mut self,
        instrument_id: &str,
        settlement_price: Decimal,
        expired: bool,
        _event_time: Option<UtcTimestamp>,
        source_observation_id: String,
        source_revision: u64,
    ) -> Result<FutureSettlement, EngineError> {
        let configured = self
            .instruments()
            .get(instrument_id)
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::LinearFuture(instrument) = configured else {
            return Err(EngineError::NotFutureInstrument(instrument_id.to_owned()));
        };
        if settlement_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }
        let current = self
            .position(instrument_id)
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let quantity = current.quantity;
        let basis = current.settlement_basis;
        let variation = if quantity.is_zero() {
            Decimal::ZERO
        } else {
            let basis =
                basis.ok_or_else(|| EngineError::InvalidFutureTrade(instrument_id.to_owned()))?;
            quantity
                .checked_mul(decimal(instrument.contract_multiplier.as_str())?)
                .and_then(|value| value.checked_mul(settlement_price - basis))
                .ok_or(EngineError::ArithmeticOverflow("futures daily variation"))?
        };
        let position_delta = if expired {
            Decimal::ZERO
                .checked_sub(quantity)
                .ok_or(EngineError::ArithmeticOverflow("futures expiry quantity"))?
        } else {
            Decimal::ZERO
        };
        let mut candidate = self.clone();
        let posting =
            candidate.apply_ledger_posting(Some(instrument_id), variation, position_delta)?;
        let position = candidate
            .position_mut(instrument_id)
            .expect("configured position exists");
        position.mark = settlement_price;
        position.settlement_basis = if expired {
            None
        } else {
            Some(settlement_price)
        };
        candidate.refresh_margin_requirements()?;
        *self = candidate;

        Ok(FutureSettlement {
            instrument_id: instrument.instrument_id,
            settled_quantity: quantity,
            settlement_price,
            variation_profit_loss: variation,
            posting,
            expired,
            source_observation_id,
            source_revision,
        })
    }
}

fn same_sign(left: Decimal, right: Decimal) -> bool {
    (left > Decimal::ZERO && right > Decimal::ZERO)
        || (left < Decimal::ZERO && right < Decimal::ZERO)
}

pub(super) fn magnitude(value: Decimal) -> Result<Decimal, EngineError> {
    if value < Decimal::ZERO {
        Decimal::ZERO
            .checked_sub(value)
            .ok_or(EngineError::ArithmeticOverflow("absolute value"))
    } else {
        Ok(value)
    }
}

fn settlement_basis_after_trade(
    current_quantity: Decimal,
    current_basis: Option<Decimal>,
    order_quantity: Decimal,
    next_quantity: Decimal,
    fill_price: Decimal,
) -> Result<Option<Decimal>, EngineError> {
    if next_quantity.is_zero() {
        return Ok(None);
    }
    if current_quantity.is_zero() || !same_sign(current_quantity, next_quantity) {
        return Ok(Some(fill_price));
    }
    if same_sign(current_quantity, order_quantity) {
        let basis = current_basis.ok_or(EngineError::InvalidFutureTrade("missing basis".into()))?;
        let old_weight = magnitude(current_quantity)?;
        let new_weight = magnitude(order_quantity)?;
        let weighted = basis
            .checked_mul(old_weight)
            .and_then(|value| value.checked_add(fill_price.checked_mul(new_weight)?))
            .ok_or(EngineError::ArithmeticOverflow("weighted futures basis"))?;
        let total = old_weight
            .checked_add(new_weight)
            .ok_or(EngineError::ArithmeticOverflow("weighted futures quantity"))?;
        return weighted
            .checked_div(total)
            .map(Some)
            .ok_or(EngineError::ArithmeticOverflow("weighted futures basis"));
    }
    Ok(current_basis)
}

pub(super) fn adverse_fill_price(
    quote: Decimal,
    signed_quantity: Decimal,
    tick: Decimal,
    slippage_bps: Decimal,
) -> Result<Decimal, EngineError> {
    if quote <= Decimal::ZERO || tick <= Decimal::ZERO || slippage_bps < Decimal::ZERO {
        return Err(EngineError::InvalidExecutionPrice);
    }
    let rate = slippage_bps
        .checked_div(Decimal::from(10_000))
        .ok_or(EngineError::ArithmeticOverflow("slippage rate"))?;
    let factor = if signed_quantity > Decimal::ZERO {
        Decimal::ONE
            .checked_add(rate)
            .ok_or(EngineError::ArithmeticOverflow("slippage factor"))?
    } else {
        Decimal::ONE
            .checked_sub(rate)
            .ok_or(EngineError::ArithmeticOverflow("slippage factor"))?
    };
    let slipped = quote
        .checked_mul(factor)
        .ok_or(EngineError::ArithmeticOverflow("slipped price"))?;
    if slipped <= Decimal::ZERO {
        return Err(EngineError::InvalidExecutionPrice);
    }
    let strategy = if signed_quantity > Decimal::ZERO {
        RoundingStrategy::ToPositiveInfinity
    } else {
        RoundingStrategy::ToNegativeInfinity
    };
    let units = slipped
        .checked_div(tick)
        .ok_or(EngineError::ArithmeticOverflow("tick conversion"))?
        .round_dp_with_strategy(0, strategy);
    units
        .checked_mul(tick)
        .filter(|value| *value > Decimal::ZERO)
        .ok_or(EngineError::ArithmeticOverflow("tick rounding"))
}

pub(super) fn calculate_fee(
    signed_quantity: Decimal,
    fill_price: Decimal,
    multiplier: Decimal,
    fee_bps: Decimal,
    fee_per_unit: Decimal,
) -> Result<Decimal, EngineError> {
    if fee_bps < Decimal::ZERO || fee_per_unit < Decimal::ZERO {
        return Err(EngineError::InvalidFutureTrade("negative fee".into()));
    }
    let notional = magnitude(
        signed_quantity
            .checked_mul(fill_price)
            .and_then(|value| value.checked_mul(multiplier))
            .ok_or(EngineError::ArithmeticOverflow("futures notional"))?,
    )?;
    let proportional = notional
        .checked_mul(fee_bps)
        .and_then(|value| value.checked_div(Decimal::from(10_000)))
        .ok_or(EngineError::ArithmeticOverflow("futures proportional fee"))?;
    let per_unit = magnitude(signed_quantity)?
        .checked_mul(fee_per_unit)
        .ok_or(EngineError::ArithmeticOverflow("futures unit fee"))?;
    proportional
        .checked_add(per_unit)
        .map(|value| value.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven))
        .ok_or(EngineError::ArithmeticOverflow("futures fee"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::run_configuration::RunConfiguration;
    use factorsbt_data::FixtureRegistry;
    use rust_decimal::prelude::FromStr;
    use std::path::Path;

    fn state() -> PortfolioState {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let mut configuration: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        configuration.instruments.push(
            serde_json::from_str(r#"{"instrument_type":"linear_future","instrument_id":"FUT_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"5","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","expiry_at":"2026-01-09T16:00:00Z","initial_margin_per_contract":"1000","maintenance_margin_per_contract":"800"}"#)
                .unwrap(),
        );
        PortfolioState::initialize(&configuration, &fixture).unwrap()
    }

    fn value(value: &str) -> Decimal {
        Decimal::from_str(value).unwrap()
    }

    #[test]
    fn futures_entry_has_no_notional_cashflow_and_variation_is_posted_once() {
        let mut state = state();
        let entry = state
            .execute_future_trade("FUT_A", Decimal::ONE, value("100"), "OPEN-FUT", 1)
            .unwrap();
        assert_eq!(entry.trade_posting.cash_delta, Decimal::ZERO);
        assert_eq!(state.cash(), Decimal::from(100000));
        assert_eq!(
            state.position("FUT_A").unwrap().settlement_basis,
            Some(value("100"))
        );
        assert_eq!(state.initial_margin_requirement(), Decimal::from(1000));

        let first = state
            .settle_future_daily("FUT_A", value("110"), "CLOSE-FUT", 1)
            .unwrap();
        assert_eq!(first.variation_profit_loss, Decimal::from(50));
        assert_eq!(state.cash(), Decimal::from(100050));
        let second = state
            .settle_future_daily("FUT_A", value("110"), "CLOSE-FUT-2", 1)
            .unwrap();
        assert_eq!(second.variation_profit_loss, Decimal::ZERO);
        assert_eq!(state.cash(), Decimal::from(100050));
    }

    #[test]
    fn opposing_order_realizes_variation_and_reversal_uses_fill_basis() {
        let mut state = state();
        state
            .execute_future_trade("FUT_A", Decimal::from(2), value("100"), "OPEN", 1)
            .unwrap();
        let close = state
            .execute_future_trade("FUT_A", Decimal::from(-1), value("105"), "CLOSE", 1)
            .unwrap();
        assert_eq!(close.realized_variation, Decimal::from(25));
        assert_eq!(state.position("FUT_A").unwrap().quantity, Decimal::ONE);
        assert_eq!(
            state.position("FUT_A").unwrap().settlement_basis,
            Some(value("100"))
        );
        let reverse = state
            .execute_future_trade("FUT_A", Decimal::from(-2), value("105"), "REVERSE", 1)
            .unwrap();
        assert_eq!(reverse.realized_variation, Decimal::from(25));
        assert_eq!(state.position("FUT_A").unwrap().quantity, Decimal::from(-1));
        assert_eq!(
            state.position("FUT_A").unwrap().settlement_basis,
            Some(value("105"))
        );
    }

    #[test]
    fn expiry_settles_variation_and_flattens_only_at_or_after_expiry() {
        let mut state = state();
        state
            .execute_future_trade("FUT_A", Decimal::from(-1), value("100"), "OPEN", 1)
            .unwrap();
        assert!(matches!(
            state.settle_future_expiry(
                "FUT_A",
                &UtcTimestamp::new("2026-01-08T16:00:00Z").unwrap(),
                value("90"),
                "EARLY",
                1,
            ),
            Err(EngineError::FutureNotExpired(_))
        ));
        let expiry = state
            .settle_future_expiry(
                "FUT_A",
                &UtcTimestamp::new("2026-01-09T16:00:00Z").unwrap(),
                value("90"),
                "EXPIRY",
                1,
            )
            .unwrap();
        assert!(expiry.expired);
        assert_eq!(expiry.variation_profit_loss, Decimal::from(50));
        assert_eq!(state.cash(), Decimal::from(100050));
        assert_eq!(state.position("FUT_A").unwrap().quantity, Decimal::ZERO);
        assert_eq!(state.position("FUT_A").unwrap().settlement_basis, None);
    }
}
