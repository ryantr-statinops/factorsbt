//! Controlled fixture-priced executions.

use crate::{
    portfolio::{decimal, validate_quantity},
    EngineError, LedgerPosting, PortfolioState,
};
use factorsbt_contracts::{instruments::Instrument, InstrumentId};
use rust_decimal::{Decimal, RoundingStrategy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotExecution {
    pub instrument_id: InstrumentId,
    pub signed_quantity: Decimal,
    pub fill_price: Decimal,
    pub fee: Decimal,
    pub trade_posting: LedgerPosting,
    pub fee_posting: LedgerPosting,
    pub source_observation_id: String,
    pub source_revision: u64,
}

impl PortfolioState {
    /// Execute one full-liquidity spot trade against an available fixture open.
    /// The trade and fee postings are applied together or leave the state as-is.
    pub fn execute_spot_trade(
        &mut self,
        instrument_id: &str,
        signed_quantity: Decimal,
        quote_price: Decimal,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<SpotExecution, EngineError> {
        let instrument = self
            .instruments()
            .get(instrument_id)
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::Spot(instrument) = instrument else {
            return Err(EngineError::UnsupportedTradeInstrument(
                instrument_id.to_owned(),
            ));
        };
        validate_quantity(&Instrument::Spot(instrument.clone()), signed_quantity)
            .map_err(|()| EngineError::InvalidTradeQuantity(instrument_id.to_owned()))?;
        if signed_quantity.is_zero() {
            return Err(EngineError::InvalidTradeQuantity(instrument_id.to_owned()));
        }
        if quote_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }

        let tick = decimal(instrument.price_tick.as_str())?;
        let multiplier = decimal(instrument.contract_multiplier.as_str())?;
        let fee_bps = decimal(instrument.fee_bps.as_str())?;
        let fee_per_unit = decimal(instrument.fee_per_unit.as_str())?;
        let slippage_bps = decimal(instrument.slippage_bps.as_str())?;
        if tick <= Decimal::ZERO
            || multiplier <= Decimal::ZERO
            || fee_bps < Decimal::ZERO
            || fee_per_unit < Decimal::ZERO
            || slippage_bps < Decimal::ZERO
        {
            return Err(EngineError::InvalidExecutionPrice);
        }

        let ten_thousand = Decimal::from(10_000);
        let slippage_rate = slippage_bps
            .checked_div(ten_thousand)
            .ok_or(EngineError::ArithmeticOverflow("slippage rate"))?;
        let price_factor = if signed_quantity > Decimal::ZERO {
            Decimal::ONE
                .checked_add(slippage_rate)
                .ok_or(EngineError::ArithmeticOverflow("slippage factor"))?
        } else {
            Decimal::ONE
                .checked_sub(slippage_rate)
                .ok_or(EngineError::ArithmeticOverflow("slippage factor"))?
        };
        let slipped_price = quote_price
            .checked_mul(price_factor)
            .ok_or(EngineError::ArithmeticOverflow("slipped price"))?;
        if slipped_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }
        let tick_units = slipped_price
            .checked_div(tick)
            .ok_or(EngineError::ArithmeticOverflow("tick conversion"))?
            .round_dp_with_strategy(
                0,
                if signed_quantity > Decimal::ZERO {
                    RoundingStrategy::ToPositiveInfinity
                } else {
                    RoundingStrategy::ToNegativeInfinity
                },
            );
        let fill_price = tick_units
            .checked_mul(tick)
            .ok_or(EngineError::ArithmeticOverflow("tick rounding"))?;
        if fill_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }

        let signed_notional = signed_quantity
            .checked_mul(fill_price)
            .and_then(|value| value.checked_mul(multiplier))
            .ok_or(EngineError::ArithmeticOverflow("spot notional"))?;
        let trade_cash_delta = Decimal::ZERO
            .checked_sub(signed_notional)
            .ok_or(EngineError::ArithmeticOverflow("spot cash delta"))?;
        let absolute_quantity = positive_magnitude(signed_quantity)?;
        let notional = positive_magnitude(signed_notional)?;
        let proportional_fee = notional
            .checked_mul(fee_bps)
            .and_then(|value| value.checked_div(ten_thousand))
            .ok_or(EngineError::ArithmeticOverflow("proportional fee"))?;
        let per_unit_fee = absolute_quantity
            .checked_mul(fee_per_unit)
            .ok_or(EngineError::ArithmeticOverflow("per-unit fee"))?;
        let fee = proportional_fee
            .checked_add(per_unit_fee)
            .ok_or(EngineError::ArithmeticOverflow("total fee"))?
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);

        let mut candidate = self.clone();
        let trade_posting = candidate.apply_ledger_posting(
            Some(instrument_id),
            trade_cash_delta,
            signed_quantity,
        )?;
        let negative_fee = Decimal::ZERO
            .checked_sub(fee)
            .ok_or(EngineError::ArithmeticOverflow("fee cash delta"))?;
        let fee_posting = candidate.apply_ledger_posting(None, negative_fee, Decimal::ZERO)?;
        if candidate.cash() < Decimal::ZERO {
            return Err(EngineError::InsufficientCash);
        }
        *self = candidate;

        Ok(SpotExecution {
            instrument_id: instrument.instrument_id,
            signed_quantity,
            fill_price,
            fee,
            trade_posting,
            fee_posting,
            source_observation_id: source_observation_id.into(),
            source_revision,
        })
    }
}

fn positive_magnitude(value: Decimal) -> Result<Decimal, EngineError> {
    if value < Decimal::ZERO {
        Decimal::ZERO
            .checked_sub(value)
            .ok_or(EngineError::ArithmeticOverflow("absolute value"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::run_configuration::RunConfiguration;
    use factorsbt_data::FixtureRegistry;
    use std::{path::Path, str::FromStr};

    fn state(cash: &str, costs: bool, long_spot: bool) -> PortfolioState {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let mut configuration: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        configuration.initial_portfolio.cash =
            factorsbt_contracts::DecimalString::new(cash).unwrap();
        if costs {
            let Instrument::Spot(instrument) = &mut configuration.instruments[0] else {
                unreachable!()
            };
            instrument.fee_bps = factorsbt_contracts::DecimalString::new("10").unwrap();
            instrument.fee_per_unit = factorsbt_contracts::DecimalString::new("0.02").unwrap();
            instrument.slippage_bps = factorsbt_contracts::DecimalString::new("100").unwrap();
        }
        if long_spot {
            configuration.initial_portfolio.positions.push(
                serde_json::from_str(r#"{"instrument_id":"SPOT_A","quantity":"1"}"#).unwrap(),
            );
        }
        PortfolioState::initialize(&configuration, &fixture).unwrap()
    }

    fn decimal_value(value: &str) -> Decimal {
        Decimal::from_str(value).unwrap()
    }

    #[test]
    fn applies_adverse_slippage_tick_rounding_and_fee_separately() {
        let mut state = state("100000", true, false);
        let execution = state
            .execute_spot_trade(
                "SPOT_A",
                Decimal::ONE,
                decimal_value("100.50"),
                "OPEN-SPOT-A",
                1,
            )
            .unwrap();
        assert_eq!(execution.fill_price, decimal_value("101.51"));
        assert_eq!(execution.fee, decimal_value("0.12"));
        assert_eq!(execution.trade_posting.cash_delta, decimal_value("-101.51"));
        assert_eq!(execution.fee_posting.cash_delta, decimal_value("-0.12"));
        assert_eq!(state.cash(), decimal_value("99898.37"));
        assert_eq!(state.position("SPOT_A").unwrap().quantity, Decimal::ONE);
        assert_eq!(execution.source_observation_id, "OPEN-SPOT-A");
    }

    #[test]
    fn cannot_open_short_spot() {
        let mut state = state("100000", true, false);
        let error = state
            .execute_spot_trade(
                "SPOT_A",
                Decimal::from(-1),
                decimal_value("100.50"),
                "OPEN-SPOT-A",
                1,
            )
            .unwrap_err();
        assert!(matches!(error, EngineError::InvalidTradeQuantity(_)));
        assert_eq!(state.cash(), Decimal::from(100000));
    }

    #[test]
    fn sell_slippage_rounds_down_to_the_adverse_tick() {
        let mut state = state("100000", true, true);
        let execution = state
            .execute_spot_trade(
                "SPOT_A",
                Decimal::from(-1),
                decimal_value("100.50"),
                "OPEN-SPOT-A",
                1,
            )
            .unwrap();
        assert_eq!(execution.fill_price, decimal_value("99.49"));
        assert_eq!(execution.fee, decimal_value("0.12"));
        assert_eq!(state.position("SPOT_A").unwrap().quantity, Decimal::ZERO);
        assert_eq!(state.cash(), decimal_value("100099.37"));
    }

    #[test]
    fn unaffordable_spot_fill_rolls_back_trade_and_fee_together() {
        let mut state = state("100", true, false);
        assert!(matches!(
            state.execute_spot_trade(
                "SPOT_A",
                Decimal::ONE,
                decimal_value("100.50"),
                "OPEN-SPOT-A",
                1,
            ),
            Err(EngineError::InsufficientCash)
        ));
        assert_eq!(state.cash(), Decimal::from(100));
        assert_eq!(state.position("SPOT_A").unwrap().quantity, Decimal::ZERO);
    }
}
