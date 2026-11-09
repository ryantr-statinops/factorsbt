//! Option premium trades and European cash expiry.

use crate::{
    execution::{adverse_fill_price, calculate_fee},
    futures::magnitude,
    portfolio::{decimal, validate_lot_quantity},
    EngineError, LedgerPosting, PortfolioState,
};
use factorsbt_contracts::{
    compare_utc_timestamps,
    instruments::{ExerciseStyle, Instrument, OptionType as ContractOptionType, SettlementStyle},
    InstrumentId, UtcTimestamp,
};
use factorsbt_data::{FixtureSettlementEvent, FixtureSettlementType};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalOptionSettlement {
    pub fixture_event_id: String,
    pub instrument_id: InstrumentId,
    pub event_type: FixtureSettlementType,
    pub contracts: u64,
    pub underlying_price: Decimal,
    pub cash_amount: Decimal,
    pub underlying_quantity_delta: Decimal,
    pub option_posting: LedgerPosting,
    pub underlying_posting: LedgerPosting,
    pub event_time: UtcTimestamp,
    pub source_observation_id: String,
    pub source_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmericanWorthlessExpiry {
    pub instrument_id: InstrumentId,
    pub expired_contracts: Decimal,
    pub underlying_price: Decimal,
    pub posting: LedgerPosting,
    pub event_time: UtcTimestamp,
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
        if candidate.cash() < candidate.initial_margin_requirement() {
            return Err(EngineError::InsufficientInitialMargin);
        }
        if candidate.cash() < candidate.cash_encumbrance() {
            return Err(EngineError::InsufficientDeliveryCash);
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

    /// Apply one explicit American exercise or assignment event. The event's
    /// contract count, option liability/asset, underlying delivery and cash
    /// flow are validated on a private candidate state and committed together.
    pub fn apply_american_settlement_event(
        &mut self,
        event: &FixtureSettlementEvent,
    ) -> Result<PhysicalOptionSettlement, EngineError> {
        let event_id = event.fixture_event_id.as_str().to_owned();
        if self.consumed_settlement_event_ids.contains(&event_id) {
            return Err(EngineError::DuplicateSettlementEvent(event_id));
        }
        let configured = self
            .instruments()
            .get(event.instrument_id.as_str())
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(event.instrument_id.as_str().to_owned()))?;
        let Instrument::Option(instrument) = configured else {
            return Err(EngineError::InvalidAmericanSettlement(
                event.instrument_id.as_str().to_owned(),
            ));
        };
        if instrument.exercise_style != ExerciseStyle::American
            || instrument.settlement_style != SettlementStyle::Physical
        {
            return Err(EngineError::InvalidAmericanSettlement(
                event.instrument_id.as_str().to_owned(),
            ));
        }
        let underlying_price = decimal(event.underlying_price.as_str())?;
        if underlying_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }
        let current_quantity = self
            .position(event.instrument_id.as_str())
            .ok_or_else(|| EngineError::MissingPosition(event.instrument_id.as_str().to_owned()))?
            .quantity;
        let contracts = Decimal::from(event.contracts);
        if event.contracts == 0 || magnitude(current_quantity)? < contracts {
            return Err(EngineError::ExcessSettlementQuantity(
                event.instrument_id.as_str().to_owned(),
            ));
        }
        let is_exercise = event.event_type == FixtureSettlementType::AmericanExercise;
        if (is_exercise && current_quantity <= Decimal::ZERO)
            || (!is_exercise && current_quantity >= Decimal::ZERO)
        {
            return Err(EngineError::SettlementEventMismatch(
                event.instrument_id.as_str().to_owned(),
            ));
        }

        let units = contracts
            .checked_mul(decimal(instrument.contract_multiplier.as_str())?)
            .ok_or(EngineError::ArithmeticOverflow("physical delivery units"))?;
        let strike_cash = units
            .checked_mul(decimal(instrument.strike.as_str())?)
            .ok_or(EngineError::ArithmeticOverflow("physical delivery cash"))?;
        let (option_quantity_delta, underlying_quantity_delta, cash_flow) =
            match (event.event_type, instrument.option_type) {
                (FixtureSettlementType::AmericanExercise, ContractOptionType::Call) => (
                    Decimal::ZERO
                        .checked_sub(contracts)
                        .ok_or(EngineError::ArithmeticOverflow("exercise contracts"))?,
                    units,
                    Decimal::ZERO
                        .checked_sub(strike_cash)
                        .ok_or(EngineError::ArithmeticOverflow("call exercise cash"))?,
                ),
                (FixtureSettlementType::AmericanExercise, ContractOptionType::Put) => (
                    Decimal::ZERO
                        .checked_sub(contracts)
                        .ok_or(EngineError::ArithmeticOverflow("exercise contracts"))?,
                    Decimal::ZERO
                        .checked_sub(units)
                        .ok_or(EngineError::ArithmeticOverflow("put delivery units"))?,
                    strike_cash,
                ),
                (FixtureSettlementType::AmericanAssignment, ContractOptionType::Call) => (
                    contracts,
                    Decimal::ZERO
                        .checked_sub(units)
                        .ok_or(EngineError::ArithmeticOverflow("call assignment units"))?,
                    strike_cash,
                ),
                (FixtureSettlementType::AmericanAssignment, ContractOptionType::Put) => (
                    contracts,
                    units,
                    Decimal::ZERO
                        .checked_sub(strike_cash)
                        .ok_or(EngineError::ArithmeticOverflow("put assignment cash"))?,
                ),
            };

        let mut candidate = self.clone();
        let option_posting = candidate.apply_ledger_posting(
            Some(event.instrument_id.as_str()),
            Decimal::ZERO,
            option_quantity_delta,
        )?;
        let underlying_posting = candidate
            .apply_ledger_posting(
                Some(instrument.underlying_id.as_str()),
                cash_flow,
                underlying_quantity_delta,
            )
            .map_err(|error| match error {
                EngineError::InvalidPostingQuantity(_) => {
                    EngineError::InsufficientUnderlyingCollateral(
                        instrument.underlying_id.as_str().to_owned(),
                    )
                }
                other => other,
            })?;
        if candidate.cash() < Decimal::ZERO {
            return Err(EngineError::InsufficientCash);
        }
        if candidate.cash() < candidate.cash_encumbrance() {
            return Err(EngineError::InsufficientDeliveryCash);
        }
        candidate
            .consumed_settlement_event_ids
            .insert(event_id.clone());
        *self = candidate;

        Ok(PhysicalOptionSettlement {
            fixture_event_id: event_id,
            instrument_id: instrument.instrument_id,
            event_type: event.event_type,
            contracts: event.contracts,
            underlying_price,
            cash_amount: underlying_posting.cash_delta,
            underlying_quantity_delta,
            option_posting,
            underlying_posting,
            event_time: event.event_time.clone(),
            source_observation_id: event.observation_id.as_str().to_owned(),
            source_revision: event.source_revision,
        })
    }

    /// Expire an American physical option without an exercise event only when
    /// it is out of the money. In-the-money positions require explicit fixture
    /// evidence and are left untouched on failure.
    pub fn expire_american_option_without_event(
        &mut self,
        instrument_id: &str,
        event_time: &UtcTimestamp,
        underlying_price: Decimal,
        source_observation_id: impl Into<String>,
        source_revision: u64,
    ) -> Result<AmericanWorthlessExpiry, EngineError> {
        let configured = self
            .instruments()
            .get(instrument_id)
            .cloned()
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?;
        let Instrument::Option(instrument) = configured else {
            return Err(EngineError::NotOptionInstrument(instrument_id.to_owned()));
        };
        if instrument.exercise_style != ExerciseStyle::American
            || instrument.settlement_style != SettlementStyle::Physical
        {
            return Err(EngineError::InvalidAmericanSettlement(
                instrument_id.to_owned(),
            ));
        }
        if compare_utc_timestamps(event_time, &instrument.expiry_at) == Ordering::Less {
            return Err(EngineError::OptionNotExpired(instrument_id.to_owned()));
        }
        if underlying_price <= Decimal::ZERO {
            return Err(EngineError::InvalidExecutionPrice);
        }
        let strike = decimal(instrument.strike.as_str())?;
        let in_the_money = match instrument.option_type {
            ContractOptionType::Call => underlying_price > strike,
            ContractOptionType::Put => underlying_price < strike,
        };
        let quantity = self
            .position(instrument_id)
            .ok_or_else(|| EngineError::MissingPosition(instrument_id.to_owned()))?
            .quantity;
        if in_the_money && !quantity.is_zero() {
            return Err(EngineError::MissingPhysicalSettlement(
                instrument_id.to_owned(),
            ));
        }
        let mut candidate = self.clone();
        let position_delta = Decimal::ZERO
            .checked_sub(quantity)
            .ok_or(EngineError::ArithmeticOverflow("American expiry quantity"))?;
        let posting =
            candidate.apply_ledger_posting(Some(instrument_id), Decimal::ZERO, position_delta)?;
        *self = candidate;

        Ok(AmericanWorthlessExpiry {
            instrument_id: instrument.instrument_id,
            expired_contracts: quantity,
            underlying_price,
            posting,
            event_time: event_time.clone(),
            source_observation_id: source_observation_id.into(),
            source_revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{run_configuration::RunConfiguration, Identifier, InstrumentId};
    use factorsbt_data::{FixtureRegistry, FixtureSettlementEvent};
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

    fn physical_state(
        option_type: &str,
        option_quantity: &str,
        spot_quantity: &str,
        cash: &str,
    ) -> PortfolioState {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = registry.get_by("mvp-settle-01", "v1").unwrap();
        let instrument_id = if option_type == "call" {
            "AM_CALL_A"
        } else {
            "AM_PUT_A"
        };
        let option: Instrument = serde_json::from_str(&format!(
            r#"{{"instrument_type":"option","instrument_id":"{instrument_id}","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"100","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","option_type":"{option_type}","strike":"100","expiry_at":"2026-01-09T16:00:00Z","exercise_style":"american","settlement_style":"physical","valuation_mode":"quote","initial_margin_per_contract":"600","maintenance_margin_per_contract":"500"}}"#
        ))
        .unwrap();
        let mut configuration: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        configuration.instruments.push(option);
        configuration.initial_portfolio.cash =
            factorsbt_contracts::DecimalString::new(cash).unwrap();
        configuration.initial_portfolio.positions.push(
            serde_json::from_str(&format!(
                r#"{{"instrument_id":"{instrument_id}","quantity":"{option_quantity}"}}"#
            ))
            .unwrap(),
        );
        if spot_quantity != "0" {
            configuration.initial_portfolio.positions.push(
                serde_json::from_str(&format!(
                    r#"{{"instrument_id":"SPOT_A","quantity":"{spot_quantity}"}}"#
                ))
                .unwrap(),
            );
        }
        PortfolioState::initialize(&configuration, &fixture).unwrap()
    }

    fn assignment_event() -> FixtureSettlementEvent {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        registry
            .get_by("mvp-settle-01", "v1")
            .unwrap()
            .dataset
            .settlement_events[0]
            .clone()
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
        assert_eq!(state.initial_margin_requirement(), Decimal::ZERO);
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

    #[test]
    fn assignment_releases_call_reserve_and_delivers_underlying_atomically_once() {
        let mut state = physical_state("call", "-1", "100", "90000");
        let event = assignment_event();
        let settlement = state.apply_american_settlement_event(&event).unwrap();
        assert_eq!(settlement.cash_amount, Decimal::from(10000));
        assert_eq!(settlement.underlying_quantity_delta, Decimal::from(-100));
        assert_eq!(state.cash(), Decimal::from(100000));
        assert_eq!(state.position("AM_CALL_A").unwrap().quantity, Decimal::ZERO);
        assert_eq!(state.position("SPOT_A").unwrap().quantity, Decimal::ZERO);
        assert!(state.reserved_underlying().is_empty());
        assert!(matches!(
            state.apply_american_settlement_event(&event),
            Err(EngineError::DuplicateSettlementEvent(_))
        ));
        assert_eq!(state.cash(), Decimal::from(100000));
    }

    #[test]
    fn physical_event_mismatch_or_excess_contracts_is_rejected_atomically() {
        let mut portfolio = physical_state("call", "-1", "100", "90000");
        let mut event = assignment_event();
        event.contracts = 2;
        assert!(matches!(
            portfolio.apply_american_settlement_event(&event),
            Err(EngineError::ExcessSettlementQuantity(_))
        ));
        assert_eq!(portfolio.cash(), Decimal::from(90000));
        assert_eq!(
            portfolio.position("AM_CALL_A").unwrap().quantity,
            Decimal::from(-1)
        );

        event.contracts = 1;
        event.event_type = FixtureSettlementType::AmericanExercise;
        assert!(matches!(
            portfolio.apply_american_settlement_event(&event),
            Err(EngineError::SettlementEventMismatch(_))
        ));
        assert_eq!(
            portfolio.position("SPOT_A").unwrap().quantity,
            Decimal::from(100)
        );

        let mut european = state();
        let mut european_event = event;
        european_event.fixture_event_id = Identifier::new("EUROPEAN-ASSIGNMENT").unwrap();
        european_event.instrument_id = InstrumentId::new("EU_CALL_A").unwrap();
        european_event.event_type = FixtureSettlementType::AmericanAssignment;
        assert!(matches!(
            european.apply_american_settlement_event(&european_event),
            Err(EngineError::InvalidAmericanSettlement(_))
        ));
    }

    #[test]
    fn long_exercise_requires_cash_or_deliverable_underlying_and_rolls_back() {
        let mut call_state = physical_state("call", "1", "0", "9999.99");
        let mut call_event = assignment_event();
        call_event.fixture_event_id = Identifier::new("EXERCISE-CALL").unwrap();
        call_event.event_type = FixtureSettlementType::AmericanExercise;
        call_event.instrument_id = InstrumentId::new("AM_CALL_A").unwrap();
        assert!(matches!(
            call_state.apply_american_settlement_event(&call_event),
            Err(EngineError::InsufficientCash)
        ));
        assert_eq!(call_state.cash(), value("9999.99"));
        assert_eq!(
            call_state.position("AM_CALL_A").unwrap().quantity,
            Decimal::ONE
        );
        assert_eq!(
            call_state.position("SPOT_A").unwrap().quantity,
            Decimal::ZERO
        );

        let mut put_state = physical_state("put", "1", "0", "100000");
        let mut put_event = call_event;
        put_event.fixture_event_id = Identifier::new("EXERCISE-PUT").unwrap();
        put_event.instrument_id = InstrumentId::new("AM_PUT_A").unwrap();
        assert!(matches!(
            put_state.apply_american_settlement_event(&put_event),
            Err(EngineError::InsufficientUnderlyingCollateral(_))
        ));
        assert_eq!(
            put_state.position("AM_PUT_A").unwrap().quantity,
            Decimal::ONE
        );
        assert_eq!(put_state.cash(), Decimal::from(100000));
    }

    #[test]
    fn american_option_expires_worthless_only_when_out_of_the_money() {
        let expiry = UtcTimestamp::new("2026-01-09T16:00:00Z").unwrap();
        let mut otm = physical_state("call", "1", "0", "100000");
        let result = otm
            .expire_american_option_without_event(
                "AM_CALL_A",
                &expiry,
                value("90"),
                "SPOT-CLOSE",
                1,
            )
            .unwrap();
        assert_eq!(result.expired_contracts, Decimal::ONE);
        assert_eq!(result.posting.cash_delta, Decimal::ZERO);
        assert_eq!(otm.position("AM_CALL_A").unwrap().quantity, Decimal::ZERO);

        let mut itm = physical_state("call", "1", "0", "100000");
        assert!(matches!(
            itm.expire_american_option_without_event(
                "AM_CALL_A",
                &expiry,
                value("110"),
                "SPOT-CLOSE",
                1,
            ),
            Err(EngineError::MissingPhysicalSettlement(_))
        ));
        assert_eq!(itm.position("AM_CALL_A").unwrap().quantity, Decimal::ONE);
    }
}
