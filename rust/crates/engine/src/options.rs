//! Option premium trades and European cash expiry.

use crate::{
    futures::{adverse_fill_price, calculate_fee},
    portfolio::{decimal, validate_lot_quantity},
    EngineError, LedgerPosting, PortfolioState,
};
use factorsbt_contracts::{
    compare_utc_timestamps,
    instruments::{ExerciseStyle, Instrument, OptionType as ContractOptionType, SettlementStyle},
    InstrumentId, UtcTimestamp,
};
use rust_decimal::Decimal;
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionExecution {
    pub instrument_id: InstrumentId,
    pub signed_contracts: Decimal,
    pub fill_premium: Decimal,
    pub fee: Decimal,
    pub premium_posting: LedgerPosting,
    pub fee_posting: LedgerPosting,
    pub source_observation_id: String,
    pub source_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionSettlement {
    pub instrument_id: InstrumentId,
    pub settled_contracts: Decimal,
    pub underlying_price: Decimal,
    pub intrinsic_value_per_unit: Decimal,
    pub cash_amount: Decimal,
    pub posting: LedgerPosting,
    pub source_observation_id: String,
    pub source_revision: u64,
}

impl PortfolioState {
    /// Buy or write whole option contracts at the fixture quote. Positive
    /// quantity buys; negative quantity writes. Premium and fees are separate
    /// cash postings applied atomically with the position change.
    pub fn execute_option_trade(
        &mut self,
        instrument_id: &str,
        signed_contracts: Decimal,
        quote_premium: Decimal,
        simulation_time: &UtcTimestamp,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<OptionExecution, EngineError> {
        let configured = self
            .instruments()
            .get(instrument_id)
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::Option(instrument) = configured else {
            return Err(EngineError::NotOptionInstrument(instrument_id.to_owned()));
        };
        if signed_contracts.is_zero()
            || validate_lot_quantity(&Instrument::Option(instrument.clone()), signed_contracts)
                .is_err()
            || quote_premium <= Decimal::ZERO
            || compare_utc_timestamps(simulation_time, &instrument.expiry_at) != Ordering::Less
        {
            return Err(EngineError::InvalidOptionTrade(instrument_id.to_owned()));
        }

        let fill_premium = adverse_fill_price(
            quote_premium,
            signed_contracts,
            decimal(instrument.price_tick.as_str())?,
            decimal(instrument.slippage_bps.as_str())?,
        )?;
        let multiplier = decimal(instrument.contract_multiplier.as_str())?;
        let fee = calculate_fee(
            signed_contracts,
            fill_premium,
            multiplier,
            decimal(instrument.fee_bps.as_str())?,
            decimal(instrument.fee_per_unit.as_str())?,
        )?;
        let signed_premium = signed_contracts
            .checked_mul(fill_premium)
            .and_then(|value| value.checked_mul(multiplier))
            .ok_or(EngineError::ArithmeticOverflow("option premium"))?;
        let premium_cash_delta = Decimal::ZERO
            .checked_sub(signed_premium)
            .ok_or(EngineError::ArithmeticOverflow("option premium cash"))?;

        let mut candidate = self.clone();
        let premium_posting = candidate.apply_ledger_posting(
            Some(instrument_id),
            premium_cash_delta,
            signed_contracts,
        )?;
        let fee_cash_delta = Decimal::ZERO
            .checked_sub(fee)
            .ok_or(EngineError::ArithmeticOverflow("option fee cash"))?;
        let fee_posting = candidate.apply_ledger_posting(None, fee_cash_delta, Decimal::ZERO)?;
        if candidate.cash() < Decimal::ZERO {
            return Err(EngineError::InsufficientCash);
        }
        *self = candidate;

        Ok(OptionExecution {
            instrument_id: instrument.instrument_id,
            signed_contracts,
            fill_premium,
            fee,
            premium_posting,
            fee_posting,
            source_observation_id: source_observation_id.into(),
            source_revision,
        })
    }

    /// Settle all remaining European cash-settled contracts at expiry. This
    /// does not infer exercise for American options.
    pub fn settle_european_option_expiry(
        &mut self,
        instrument_id: &str,
        event_time: &UtcTimestamp,
        underlying_price: Decimal,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<OptionSettlement, EngineError> {
        let configured = self
            .instruments()
            .get(instrument_id)
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::Option(instrument) = configured else {
            return Err(EngineError::NotOptionInstrument(instrument_id.to_owned()));
        };
        if instrument.exercise_style != ExerciseStyle::European
            || instrument.settlement_style != SettlementStyle::Cash
        {
            return Err(EngineError::NotEuropeanCashOption(instrument_id.to_owned()));
        }
        if compare_utc_timestamps(event_time, &instrument.expiry_at) == Ordering::Less {
            return Err(EngineError::OptionNotExpired(instrument_id.to_owned()));
        }
        if underlying_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }

        let strike = decimal(instrument.strike.as_str())?;
        let intrinsic = match instrument.option_type {
            ContractOptionType::Call => (underlying_price - strike).max(Decimal::ZERO),
            ContractOptionType::Put => (strike - underlying_price).max(Decimal::ZERO),
        };
        let quantity = self
            .position(instrument_id)
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?
            .quantity;
        let multiplier = decimal(instrument.contract_multiplier.as_str())?;
        let calculated_cash_amount = quantity
            .checked_mul(intrinsic)
            .and_then(|value| value.checked_mul(multiplier))
            .ok_or(EngineError::ArithmeticOverflow("European option payoff"))?;
        let position_delta = Decimal::ZERO
            .checked_sub(quantity)
            .ok_or(EngineError::ArithmeticOverflow("option expiry quantity"))?;

        let mut candidate = self.clone();
        let posting = candidate.apply_ledger_posting(
            Some(instrument_id),
            calculated_cash_amount,
            position_delta,
        )?;
        if candidate.cash() < Decimal::ZERO {
            return Err(EngineError::InsufficientCash);
        }
        let cash_amount = posting.cash_delta;
        *self = candidate;

        Ok(OptionSettlement {
            instrument_id: instrument.instrument_id,
            settled_contracts: quantity,
            underlying_price,
            intrinsic_value_per_unit: intrinsic,
            cash_amount,
            posting,
            source_observation_id: source_observation_id.into(),
            source_revision,
        })
    }
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
            serde_json::from_str(r#"{"instrument_type":"option","instrument_id":"EU_CALL_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"100","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","option_type":"call","strike":"100","expiry_at":"2026-01-08T16:00:00Z","exercise_style":"european","settlement_style":"cash","valuation_mode":"quote","initial_margin_per_contract":"600","maintenance_margin_per_contract":"600"}"#)
                .unwrap(),
        );
        PortfolioState::initialize(&configuration, &fixture).unwrap()
    }

    fn value(value: &str) -> Decimal {
        Decimal::from_str(value).unwrap()
    }

    #[test]
    fn option_premium_changes_cash_and_long_position_atomically() {
        let mut state = state();
        let execution = state
            .execute_option_trade(
                "EU_CALL_A",
                Decimal::ONE,
                value("5"),
                &UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap(),
                "EU-OPEN",
                1,
            )
            .unwrap();
        assert_eq!(execution.premium_posting.cash_delta, Decimal::from(-500));
        assert_eq!(state.cash(), Decimal::from(99500));
        assert_eq!(state.position("EU_CALL_A").unwrap().quantity, Decimal::ONE);
        assert_eq!(state.initial_margin_requirement(), Decimal::from(600));
    }

    #[test]
    fn european_call_expiry_settles_intrinsic_and_flattens_position() {
        let mut state = state();
        state
            .execute_option_trade(
                "EU_CALL_A",
                Decimal::ONE,
                value("5"),
                &UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap(),
                "EU-OPEN",
                1,
            )
            .unwrap();
        let settlement = state
            .settle_european_option_expiry(
                "EU_CALL_A",
                &UtcTimestamp::new("2026-01-08T16:00:00Z").unwrap(),
                value("110"),
                "EU-EXPIRY",
                1,
            )
            .unwrap();
        assert_eq!(settlement.intrinsic_value_per_unit, Decimal::from(10));
        assert_eq!(settlement.cash_amount, Decimal::from(1000));
        assert_eq!(state.cash(), Decimal::from(100500));
        assert_eq!(state.position("EU_CALL_A").unwrap().quantity, Decimal::ZERO);
        assert_eq!(state.initial_margin_requirement(), Decimal::ZERO);
    }

    #[test]
    fn written_european_call_pays_cash_intrinsic_at_expiry() {
        let mut state = state();
        state
            .execute_option_trade(
                "EU_CALL_A",
                Decimal::from(-1),
                value("5"),
                &UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap(),
                "EU-WRITE",
                1,
            )
            .unwrap();
        let settlement = state
            .settle_european_option_expiry(
                "EU_CALL_A",
                &UtcTimestamp::new("2026-01-08T16:00:00Z").unwrap(),
                value("110"),
                "EU-EXPIRY",
                1,
            )
            .unwrap();
        assert_eq!(settlement.cash_amount, Decimal::from(-1000));
        assert_eq!(state.cash(), Decimal::from(99500));
        assert_eq!(state.position("EU_CALL_A").unwrap().quantity, Decimal::ZERO);
    }
}
