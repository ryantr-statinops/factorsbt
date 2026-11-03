//! Per-run portfolio state and start-of-run initialization.

use factorsbt_contracts::{
    instruments::{
        ExerciseStyle, Instrument, LinearFutureInstrument, OptionInstrument,
        OptionType as ContractOptionType, SettlementStyle,
    },
    run_configuration::RunConfiguration,
    InstrumentId, UtcTimestamp,
};
use factorsbt_data::{initial_marks_available_at, Fixture};
use rust_decimal::{Decimal, RoundingStrategy};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    InvalidDecimal(String),
    MissingInitialMark(String),
    InvalidInitialMark(String),
    DuplicateInstrument(String),
    UnknownPosition(String),
    DuplicatePosition(String),
    InvalidInitialCash,
    InvalidInitialPosition(String),
    InvalidPostingQuantity(String),
    InvalidTradeQuantity(String),
    UnsupportedTradeInstrument(String),
    InvalidFutureTrade(String),
    NotFutureInstrument(String),
    FutureNotExpired(String),
    NotOptionInstrument(String),
    InvalidOptionTrade(String),
    NotEuropeanCashOption(String),
    OptionNotExpired(String),
    InvalidAmericanSettlement(String),
    SettlementEventMismatch(String),
    DuplicateSettlementEvent(String),
    ExcessSettlementQuantity(String),
    MissingPhysicalSettlement(String),
    InvalidExecutionPrice,
    MissingPosition(String),
    InsufficientInitialMargin,
    InsufficientDeliveryCash,
    InsufficientUnderlyingCollateral(String),
    InvalidDeliveryUnderlying(String),
    InsufficientCash,
    ArithmeticOverflow(&'static str),
    FixtureTimeline(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDecimal(value) => write!(f, "invalid decimal value: {value}"),
            Self::MissingInitialMark(id) => write!(f, "missing initial mark for {id}"),
            Self::InvalidInitialMark(id) => write!(f, "initial mark for {id} must be positive"),
            Self::DuplicateInstrument(id) => write!(f, "duplicate configured instrument {id}"),
            Self::UnknownPosition(id) => {
                write!(f, "initial position references unknown instrument {id}")
            }
            Self::DuplicatePosition(id) => write!(f, "duplicate initial position for {id}"),
            Self::InvalidInitialCash => f.write_str("initial cash cannot be negative"),
            Self::InvalidInitialPosition(id) => write!(f, "invalid initial quantity for {id}"),
            Self::InvalidPostingQuantity(id) => write!(f, "invalid posting quantity for {id}"),
            Self::InvalidTradeQuantity(id) => write!(f, "invalid trade quantity for {id}"),
            Self::UnsupportedTradeInstrument(id) => {
                write!(f, "spot execution requested for non-spot instrument {id}")
            }
            Self::InvalidFutureTrade(id) => write!(f, "invalid futures trade for {id}"),
            Self::NotFutureInstrument(id) => write!(f, "instrument {id} is not a future"),
            Self::FutureNotExpired(id) => write!(f, "future {id} has not reached expiry"),
            Self::NotOptionInstrument(id) => write!(f, "instrument {id} is not an option"),
            Self::InvalidOptionTrade(id) => write!(f, "invalid option trade for {id}"),
            Self::NotEuropeanCashOption(id) => {
                write!(f, "option {id} is not a European cash-settled contract")
            }
            Self::OptionNotExpired(id) => write!(f, "option {id} has not reached expiry"),
            Self::InvalidAmericanSettlement(id) => {
                write!(f, "option {id} is not American physical-settled")
            }
            Self::SettlementEventMismatch(id) => {
                write!(f, "settlement event does not match position {id}")
            }
            Self::DuplicateSettlementEvent(id) => {
                write!(f, "settlement event {id} was already applied")
            }
            Self::ExcessSettlementQuantity(id) => {
                write!(f, "settlement event exceeds open contracts for {id}")
            }
            Self::MissingPhysicalSettlement(id) => {
                write!(
                    f,
                    "in-the-money American option {id} lacks a settlement event"
                )
            }
            Self::InvalidExecutionPrice => f.write_str("execution price must be positive"),
            Self::MissingPosition(id) => write!(f, "posting references unknown instrument {id}"),
            Self::InsufficientInitialMargin => {
                f.write_str("initial cash does not cover initial margin")
            }
            Self::InsufficientDeliveryCash => {
                f.write_str("cash does not cover physical-delivery collateral")
            }
            Self::InsufficientUnderlyingCollateral(id) => {
                write!(f, "insufficient unreserved underlying for instrument {id}")
            }
            Self::InvalidDeliveryUnderlying(id) => {
                write!(
                    f,
                    "physical option underlying {id} is not a configured spot"
                )
            }
            Self::InsufficientCash => f.write_str("cash constraint would be violated"),
            Self::ArithmeticOverflow(context) => {
                write!(f, "decimal arithmetic overflow: {context}")
            }
            Self::FixtureTimeline(message) => {
                write!(f, "initial fixture marks are invalid: {message}")
            }
        }
    }
}

impl Error for EngineError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentPosition {
    pub quantity: Decimal,
    /// Last valid valuation mark. Futures also use `settlement_basis` below.
    pub mark: Decimal,
    /// Previous variation settlement price; initialized to the start mark.
    pub settlement_basis: Option<Decimal>,
    pub mark_observation_id: String,
    pub mark_revision: u64,
    pub mark_source: Option<super::marks::MarkSource>,
    pub mark_provenance: Vec<super::marks::MarkProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerPosting {
    pub cash_before: Decimal,
    /// Effective cash delta after cent rounding of the resulting ledger balance.
    pub cash_delta: Decimal,
    pub cash_after: Decimal,
    pub quantity_before: Decimal,
    pub quantity_delta: Decimal,
    pub quantity_after: Decimal,
}

/// Mutable state owned by one run/scenario. Constructing another value creates
/// an independent portfolio, even when both runs share an immutable fixture.
#[derive(Debug, Clone)]
pub struct PortfolioState {
    cash: Decimal,
    instruments: HashMap<String, Instrument>,
    positions: HashMap<String, InstrumentPosition>,
    initial_margin_requirement: Decimal,
    maintenance_margin_requirement: Decimal,
    reserved_cash: Decimal,
    reserved_underlying: HashMap<String, Decimal>,
    cash_encumbrance: Decimal,
    maintenance_cash_encumbrance: Decimal,
    pub(super) consumed_settlement_event_ids: HashSet<String>,
    start_at: UtcTimestamp,
}

impl PortfolioState {
    pub fn initialize(
        configuration: &RunConfiguration,
        fixture: &Fixture,
    ) -> Result<Self, EngineError> {
        let marks =
            initial_marks_available_at(&fixture.dataset, &configuration.simulation.start_at)
                .map_err(|error| EngineError::FixtureTimeline(error.to_string()))?;
        let mut available_marks = HashMap::new();
        for mark in marks {
            let value = decimal(mark.mark.as_str())?;
            if value <= Decimal::ZERO {
                return Err(EngineError::InvalidInitialMark(
                    mark.instrument_id.as_str().to_owned(),
                ));
            }
            available_marks.insert(
                mark.instrument_id.as_str().to_owned(),
                (
                    value,
                    mark.observation_id.as_str().to_owned(),
                    mark.source_revision,
                ),
            );
        }

        let cash = decimal(configuration.initial_portfolio.cash.as_str())?
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
        if cash < Decimal::ZERO {
            return Err(EngineError::InvalidInitialCash);
        }

        let mut instruments = HashMap::new();
        let mut positions = HashMap::new();
        for instrument in &configuration.instruments {
            let id = instrument_id(instrument).as_str().to_owned();
            if instruments.insert(id.clone(), instrument.clone()).is_some() {
                return Err(EngineError::DuplicateInstrument(id));
            }
            let (mark, observation_id, revision) = available_marks
                .get(&id)
                .cloned()
                .ok_or_else(|| EngineError::MissingInitialMark(id.clone()))?;
            let basis = matches!(instrument, Instrument::LinearFuture(_)).then_some(mark);
            positions.insert(
                id,
                InstrumentPosition {
                    quantity: Decimal::ZERO,
                    mark,
                    settlement_basis: basis,
                    mark_observation_id: observation_id,
                    mark_revision: revision,
                    mark_source: None,
                    mark_provenance: Vec::new(),
                },
            );
        }

        let mut seen_positions = HashMap::new();
        for initial in &configuration.initial_portfolio.positions {
            let id = initial.instrument_id.as_str();
            let instrument = instruments
                .get(id)
                .ok_or_else(|| EngineError::UnknownPosition(id.to_owned()))?;
            if seen_positions.insert(id.to_owned(), ()).is_some() {
                return Err(EngineError::DuplicatePosition(id.to_owned()));
            }
            let quantity = decimal(initial.quantity.as_str())?;
            validate_quantity(instrument, quantity)
                .map_err(|()| EngineError::InvalidInitialPosition(id.to_owned()))?;
            positions
                .get_mut(id)
                .expect("instrument was inserted")
                .quantity = quantity;
        }

        let (initial_margin_requirement, maintenance_margin_requirement) =
            calculate_margin_requirements(&instruments, &positions, None)?;
        let (reserved_cash, reserved_underlying, cash_encumbrance, maintenance_cash_encumbrance) =
            calculate_delivery_reservations(
                &instruments,
                &positions,
                initial_margin_requirement,
                maintenance_margin_requirement,
                None,
            )?;
        validate_underlying_collateral(&instruments, &positions, &reserved_underlying, None)?;
        if cash < initial_margin_requirement {
            return Err(EngineError::InsufficientInitialMargin);
        }
        if cash < cash_encumbrance {
            return Err(EngineError::InsufficientDeliveryCash);
        }

        Ok(Self {
            cash,
            instruments,
            positions,
            initial_margin_requirement,
            maintenance_margin_requirement,
            reserved_cash,
            reserved_underlying,
            cash_encumbrance,
            maintenance_cash_encumbrance,
            consumed_settlement_event_ids: HashSet::new(),
            start_at: configuration.simulation.start_at.clone(),
        })
    }

    pub fn cash(&self) -> Decimal {
        self.cash
    }

    /// Marked net asset value excludes settled futures notional; daily futures
    /// P&L belongs to the cash ledger after variation settlement.
    pub fn net_asset_value(&self) -> Result<Decimal, EngineError> {
        let mut ids = self.positions.keys().collect::<Vec<_>>();
        ids.sort();
        let mut nav = self.cash;
        for id in ids {
            let instrument = self
                .instruments
                .get(id)
                .expect("each configured position has an instrument");
            if matches!(instrument, Instrument::LinearFuture(_)) {
                continue;
            }
            let position = self.positions.get(id).expect("position key exists");
            let multiplier = decimal(instrument_multiplier(instrument).as_str())?;
            let market_value = position
                .quantity
                .checked_mul(position.mark)
                .and_then(|value| value.checked_mul(multiplier))
                .ok_or(EngineError::ArithmeticOverflow("position market value"))?;
            nav = nav
                .checked_add(market_value)
                .ok_or(EngineError::ArithmeticOverflow("net asset value"))?;
        }
        Ok(nav)
    }

    pub fn start_at(&self) -> &UtcTimestamp {
        &self.start_at
    }

    pub fn position(&self, instrument_id: &str) -> Option<&InstrumentPosition> {
        self.positions.get(instrument_id)
    }

    pub fn instruments(&self) -> &HashMap<String, Instrument> {
        &self.instruments
    }

    pub fn positions(&self) -> &HashMap<String, InstrumentPosition> {
        &self.positions
    }

    /// Atomically replace marks and preserve every input revision used to
    /// calculate them. A batch with an unknown or repeated instrument changes
    /// no position.
    pub fn apply_valuation_marks(
        &mut self,
        marks: &[super::marks::ValuationMark],
    ) -> Result<(), EngineError> {
        let mut seen = HashSet::new();
        for mark in marks {
            let id = mark.instrument_id.as_str();
            if !self.positions.contains_key(id) {
                return Err(EngineError::MissingPosition(id.to_owned()));
            }
            if !seen.insert(id) {
                return Err(EngineError::DuplicateInstrument(id.to_owned()));
            }
        }
        for mark in marks {
            let position = self
                .positions
                .get_mut(mark.instrument_id.as_str())
                .expect("all mark instruments were validated");
            position.mark = mark.mark;
            position.mark_source = Some(mark.source);
            position.mark_observation_id = mark
                .provenance
                .first()
                .map(|input| input.observation_id.clone())
                .unwrap_or_default();
            position.mark_revision = mark
                .provenance
                .iter()
                .map(|input| input.source_revision)
                .max()
                .unwrap_or_default();
            position.mark_provenance = mark.provenance.clone();
        }
        Ok(())
    }

    /// Build a feasible, isolated projection for a complete target-position
    /// group. Cash flows are valued at the decision snapshot mark; later fills
    /// must recheck capacity at their execution opens.
    pub fn project_target_positions(
        &self,
        targets: &[(String, Decimal)],
    ) -> Result<Self, EngineError> {
        let mut candidate = self.clone();
        let mut seen = HashSet::new();
        let mut ordered = targets.iter().collect::<Vec<_>>();
        ordered.sort_by(|left, right| left.0.cmp(&right.0));
        let mut cash_delta = Decimal::ZERO;

        for (id, target_quantity) in ordered {
            if !seen.insert(id.as_str()) {
                return Err(EngineError::DuplicateInstrument(id.clone()));
            }
            let instrument = self
                .instruments
                .get(id)
                .ok_or_else(|| EngineError::MissingPosition(id.clone()))?;
            let position = self
                .positions
                .get(id)
                .ok_or_else(|| EngineError::MissingPosition(id.clone()))?;
            validate_quantity(instrument, *target_quantity)
                .map_err(|()| EngineError::InvalidPostingQuantity(id.clone()))?;
            let delta = target_quantity
                .checked_sub(position.quantity)
                .ok_or(EngineError::ArithmeticOverflow("target quantity delta"))?;
            let multiplier = decimal(instrument_multiplier(instrument).as_str())?;
            let event_cash_delta = match instrument {
                Instrument::Spot(_) | Instrument::Option(_) => delta
                    .checked_mul(position.mark)
                    .and_then(|value| value.checked_mul(multiplier))
                    .and_then(|value| Decimal::ZERO.checked_sub(value))
                    .ok_or(EngineError::ArithmeticOverflow("target cash flow"))?,
                Instrument::LinearFuture(_) => {
                    let close_quantity = if position.quantity.is_zero()
                        || target_quantity.is_zero()
                        || target_quantity.is_sign_positive()
                            != position.quantity.is_sign_positive()
                    {
                        position.quantity
                    } else if target_quantity.abs() < position.quantity.abs() {
                        position
                            .quantity
                            .checked_sub(*target_quantity)
                            .ok_or(EngineError::ArithmeticOverflow("future close quantity"))?
                    } else {
                        Decimal::ZERO
                    };
                    close_quantity
                        .checked_mul(
                            position
                                .mark
                                .checked_sub(position.settlement_basis.unwrap_or(position.mark))
                                .ok_or(EngineError::ArithmeticOverflow("future close variation"))?,
                        )
                        .and_then(|value| value.checked_mul(multiplier))
                        .ok_or(EngineError::ArithmeticOverflow("future close variation"))?
                }
            };
            cash_delta = cash_delta
                .checked_add(event_cash_delta)
                .ok_or(EngineError::ArithmeticOverflow("group cash flow"))?;
            candidate
                .positions
                .get_mut(id)
                .expect("position was validated")
                .quantity = *target_quantity;
        }

        candidate.cash = candidate
            .cash
            .checked_add(cash_delta)
            .ok_or(EngineError::ArithmeticOverflow("group cash posting"))?
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
        let (initial, maintenance) =
            calculate_margin_requirements(&candidate.instruments, &candidate.positions, None)?;
        let (reserved_cash, reserved_underlying, cash_encumbrance, maintenance_cash_encumbrance) =
            calculate_delivery_reservations(
                &candidate.instruments,
                &candidate.positions,
                initial,
                maintenance,
                None,
            )?;
        validate_underlying_collateral(
            &candidate.instruments,
            &candidate.positions,
            &reserved_underlying,
            None,
        )?;
        if candidate.cash < Decimal::ZERO || candidate.cash < cash_encumbrance {
            return Err(EngineError::InsufficientCash);
        }
        candidate.initial_margin_requirement = initial;
        candidate.maintenance_margin_requirement = maintenance;
        candidate.reserved_cash = reserved_cash;
        candidate.reserved_underlying = reserved_underlying;
        candidate.cash_encumbrance = cash_encumbrance;
        candidate.maintenance_cash_encumbrance = maintenance_cash_encumbrance;
        Ok(candidate)
    }

    pub(super) fn position_mut(&mut self, instrument_id: &str) -> Option<&mut InstrumentPosition> {
        self.positions.get_mut(instrument_id)
    }

    pub(super) fn refresh_margin_requirements(&mut self) -> Result<(), EngineError> {
        let (initial, maintenance) =
            calculate_margin_requirements(&self.instruments, &self.positions, None)?;
        let (reserved_cash, reserved_underlying, cash_encumbrance, maintenance_cash_encumbrance) =
            calculate_delivery_reservations(
                &self.instruments,
                &self.positions,
                initial,
                maintenance,
                None,
            )?;
        validate_underlying_collateral(
            &self.instruments,
            &self.positions,
            &reserved_underlying,
            None,
        )?;
        self.initial_margin_requirement = initial;
        self.maintenance_margin_requirement = maintenance;
        self.reserved_cash = reserved_cash;
        self.reserved_underlying = reserved_underlying;
        self.cash_encumbrance = cash_encumbrance;
        self.maintenance_cash_encumbrance = maintenance_cash_encumbrance;
        Ok(())
    }

    pub fn initial_margin_requirement(&self) -> Decimal {
        self.initial_margin_requirement
    }

    pub fn maintenance_margin_requirement(&self) -> Decimal {
        self.maintenance_margin_requirement
    }

    pub fn reserved_cash(&self) -> Decimal {
        self.reserved_cash
    }

    pub fn reserved_underlying(&self) -> &HashMap<String, Decimal> {
        &self.reserved_underlying
    }

    /// Cash required by margin and delivery obligations after avoiding a
    /// second reservation for the same short-put obligation.
    pub fn cash_encumbrance(&self) -> Decimal {
        self.cash_encumbrance
    }

    pub fn maintenance_cash_encumbrance(&self) -> Decimal {
        self.maintenance_cash_encumbrance
    }

    /// Apply a cash and optional position posting atomically. Cash is rounded
    /// half-even to cents; invalid quantities or arithmetic overflow leave the
    /// state untouched.
    pub fn apply_ledger_posting(
        &mut self,
        instrument_id: Option<&str>,
        cash_delta: Decimal,
        quantity_delta: Decimal,
    ) -> Result<LedgerPosting, EngineError> {
        let position = match instrument_id {
            Some(id) => Some((
                id,
                self.instruments
                    .get(id)
                    .ok_or_else(|| EngineError::MissingPosition(id.to_owned()))?,
                self.positions
                    .get(id)
                    .ok_or_else(|| EngineError::MissingPosition(id.to_owned()))?,
            )),
            None if quantity_delta.is_zero() => None,
            None => return Err(EngineError::MissingPosition("<none>".to_owned())),
        };

        let cash_before = self.cash;
        let cash_after = cash_before
            .checked_add(cash_delta)
            .ok_or(EngineError::ArithmeticOverflow("cash posting"))?
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
        let (quantity_before, quantity_after) = if let Some((id, instrument, current)) = position {
            let next = current
                .quantity
                .checked_add(quantity_delta)
                .ok_or(EngineError::ArithmeticOverflow("quantity posting"))?;
            validate_quantity(instrument, next)
                .map_err(|()| EngineError::InvalidPostingQuantity(id.to_owned()))?;
            (current.quantity, next)
        } else {
            (Decimal::ZERO, Decimal::ZERO)
        };
        let margin_override = instrument_id.map(|id| (id, quantity_after));
        let (initial_margin_requirement, maintenance_margin_requirement) =
            calculate_margin_requirements(&self.instruments, &self.positions, margin_override)?;
        let (reserved_cash, reserved_underlying, cash_encumbrance, maintenance_cash_encumbrance) =
            calculate_delivery_reservations(
                &self.instruments,
                &self.positions,
                initial_margin_requirement,
                maintenance_margin_requirement,
                margin_override,
            )?;
        validate_underlying_collateral(
            &self.instruments,
            &self.positions,
            &reserved_underlying,
            margin_override,
        )?;

        if let Some((id, _, _)) = position {
            self.positions
                .get_mut(id)
                .expect("position was validated")
                .quantity = quantity_after;
        }
        self.cash = cash_after;
        self.initial_margin_requirement = initial_margin_requirement;
        self.maintenance_margin_requirement = maintenance_margin_requirement;
        self.reserved_cash = reserved_cash;
        self.reserved_underlying = reserved_underlying;
        self.cash_encumbrance = cash_encumbrance;
        self.maintenance_cash_encumbrance = maintenance_cash_encumbrance;

        Ok(LedgerPosting {
            cash_before,
            cash_delta: cash_after - cash_before,
            cash_after,
            quantity_before,
            quantity_delta,
            quantity_after,
        })
    }
}

pub(super) fn decimal(value: &str) -> Result<Decimal, EngineError> {
    Decimal::from_str(value).map_err(|_| EngineError::InvalidDecimal(value.to_owned()))
}

fn instrument_id(instrument: &Instrument) -> &InstrumentId {
    match instrument {
        Instrument::Spot(value) => &value.instrument_id,
        Instrument::LinearFuture(value) => &value.instrument_id,
        Instrument::Option(value) => &value.instrument_id,
    }
}

fn instrument_multiplier(instrument: &Instrument) -> &factorsbt_contracts::DecimalString {
    match instrument {
        Instrument::Spot(value) => &value.contract_multiplier,
        Instrument::LinearFuture(value) => &value.contract_multiplier,
        Instrument::Option(value) => &value.contract_multiplier,
    }
}

pub(super) fn validate_quantity(instrument: &Instrument, quantity: Decimal) -> Result<(), ()> {
    validate_lot_quantity(instrument, quantity)?;
    if matches!(instrument, Instrument::Spot(_)) && quantity < Decimal::ZERO {
        return Err(());
    }
    Ok(())
}

pub(super) fn validate_lot_quantity(instrument: &Instrument, quantity: Decimal) -> Result<(), ()> {
    let step = decimal(instrument_fields(instrument).1.as_str()).map_err(|_| ())?;
    if step <= Decimal::ZERO || quantity % step != Decimal::ZERO {
        return Err(());
    }
    if !matches!(instrument, Instrument::Spot(_)) && quantity.fract() != Decimal::ZERO {
        return Err(());
    }
    Ok(())
}

fn instrument_fields(
    instrument: &Instrument,
) -> (&InstrumentId, &factorsbt_contracts::DecimalString) {
    match instrument {
        Instrument::Spot(value) => (&value.instrument_id, &value.quantity_step),
        Instrument::LinearFuture(value) => (&value.instrument_id, &value.quantity_step),
        Instrument::Option(value) => (&value.instrument_id, &value.quantity_step),
    }
}

fn margin_per_unit(instrument: &Instrument) -> Result<(Decimal, Decimal), EngineError> {
    match instrument {
        Instrument::Spot(_) => Ok((Decimal::ZERO, Decimal::ZERO)),
        Instrument::LinearFuture(value) => margin(value),
        Instrument::Option(value) => margin(value),
    }
}

fn margin_for_position(
    instrument: &Instrument,
    quantity: Decimal,
) -> Result<(Decimal, Decimal), EngineError> {
    if matches!(instrument, Instrument::Option(_)) && quantity >= Decimal::ZERO {
        return Ok((Decimal::ZERO, Decimal::ZERO));
    }
    let (initial_per_unit, maintenance_per_unit) = margin_per_unit(instrument)?;
    let absolute_quantity =
        checked_magnitude(quantity).ok_or(EngineError::ArithmeticOverflow("position magnitude"))?;
    Ok((
        absolute_quantity
            .checked_mul(initial_per_unit)
            .ok_or(EngineError::ArithmeticOverflow("initial margin"))?,
        absolute_quantity
            .checked_mul(maintenance_per_unit)
            .ok_or(EngineError::ArithmeticOverflow("maintenance margin"))?,
    ))
}

trait MarginFields {
    fn initial(&self) -> &factorsbt_contracts::DecimalString;
    fn maintenance(&self) -> &factorsbt_contracts::DecimalString;
}

impl MarginFields for LinearFutureInstrument {
    fn initial(&self) -> &factorsbt_contracts::DecimalString {
        &self.initial_margin_per_contract
    }

    fn maintenance(&self) -> &factorsbt_contracts::DecimalString {
        &self.maintenance_margin_per_contract
    }
}

impl MarginFields for OptionInstrument {
    fn initial(&self) -> &factorsbt_contracts::DecimalString {
        &self.initial_margin_per_contract
    }

    fn maintenance(&self) -> &factorsbt_contracts::DecimalString {
        &self.maintenance_margin_per_contract
    }
}

fn margin(value: &impl MarginFields) -> Result<(Decimal, Decimal), EngineError> {
    Ok((
        decimal(value.initial().as_str())?,
        decimal(value.maintenance().as_str())?,
    ))
}

fn calculate_margin_requirements(
    instruments: &HashMap<String, Instrument>,
    positions: &HashMap<String, InstrumentPosition>,
    override_position: Option<(&str, Decimal)>,
) -> Result<(Decimal, Decimal), EngineError> {
    let mut initial_total = Decimal::ZERO;
    let mut maintenance_total = Decimal::ZERO;
    for (id, instrument) in instruments {
        let quantity = match override_position {
            Some((override_id, quantity)) if override_id == id => quantity,
            _ => {
                positions
                    .get(id)
                    .expect("configured position exists")
                    .quantity
            }
        };
        let (position_initial, position_maintenance) = margin_for_position(instrument, quantity)?;
        initial_total = initial_total
            .checked_add(position_initial)
            .ok_or(EngineError::ArithmeticOverflow("initial margin"))?;
        maintenance_total = maintenance_total
            .checked_add(position_maintenance)
            .ok_or(EngineError::ArithmeticOverflow("maintenance margin"))?;
    }
    Ok((initial_total, maintenance_total))
}

fn calculate_delivery_reservations(
    instruments: &HashMap<String, Instrument>,
    positions: &HashMap<String, InstrumentPosition>,
    initial_margin_requirement: Decimal,
    maintenance_margin_requirement: Decimal,
    override_position: Option<(&str, Decimal)>,
) -> Result<(Decimal, HashMap<String, Decimal>, Decimal, Decimal), EngineError> {
    let mut reserved_cash = Decimal::ZERO;
    let mut put_initial_margin = Decimal::ZERO;
    let mut put_maintenance_margin = Decimal::ZERO;
    let mut reserved_underlying = HashMap::new();

    for (id, configured) in instruments {
        let Instrument::Option(option) = configured else {
            continue;
        };
        let quantity = match override_position {
            Some((override_id, quantity)) if override_id == id => quantity,
            _ => {
                positions
                    .get(id)
                    .expect("configured position exists")
                    .quantity
            }
        };
        if quantity >= Decimal::ZERO
            || option.exercise_style != ExerciseStyle::American
            || option.settlement_style != SettlementStyle::Physical
        {
            continue;
        }

        let contracts = checked_magnitude(quantity)
            .ok_or(EngineError::ArithmeticOverflow("delivery contracts"))?;
        let multiplier = decimal(option.contract_multiplier.as_str())?;
        match option.option_type {
            ContractOptionType::Call => {
                let shares =
                    contracts
                        .checked_mul(multiplier)
                        .ok_or(EngineError::ArithmeticOverflow(
                            "underlying delivery reserve",
                        ))?;
                let previous = reserved_underlying
                    .get(option.underlying_id.as_str())
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                reserved_underlying.insert(
                    option.underlying_id.as_str().to_owned(),
                    previous
                        .checked_add(shares)
                        .ok_or(EngineError::ArithmeticOverflow(
                            "underlying delivery reserve",
                        ))?,
                );
            }
            ContractOptionType::Put => {
                let strike = decimal(option.strike.as_str())?;
                let delivery_cash = contracts
                    .checked_mul(strike)
                    .and_then(|value| value.checked_mul(multiplier))
                    .ok_or(EngineError::ArithmeticOverflow("cash delivery reserve"))?;
                reserved_cash = reserved_cash
                    .checked_add(delivery_cash)
                    .ok_or(EngineError::ArithmeticOverflow("cash delivery reserve"))?;
                let (initial_per_contract, maintenance_per_contract) = margin_per_unit(configured)?;
                let initial_margin = contracts
                    .checked_mul(initial_per_contract)
                    .ok_or(EngineError::ArithmeticOverflow("short put margin"))?;
                put_initial_margin = put_initial_margin
                    .checked_add(initial_margin)
                    .ok_or(EngineError::ArithmeticOverflow("short put margin"))?;
                let maintenance_margin = contracts.checked_mul(maintenance_per_contract).ok_or(
                    EngineError::ArithmeticOverflow("short put maintenance margin"),
                )?;
                put_maintenance_margin = put_maintenance_margin
                    .checked_add(maintenance_margin)
                    .ok_or(EngineError::ArithmeticOverflow(
                        "short put maintenance margin",
                    ))?;
            }
        }
    }

    let cash_encumbrance = cash_requirement_without_double_count(
        initial_margin_requirement,
        put_initial_margin,
        reserved_cash,
        "initial cash encumbrance",
    )?;
    let maintenance_cash_encumbrance = cash_requirement_without_double_count(
        maintenance_margin_requirement,
        put_maintenance_margin,
        reserved_cash,
        "maintenance cash encumbrance",
    )?;
    Ok((
        reserved_cash,
        reserved_underlying,
        cash_encumbrance,
        maintenance_cash_encumbrance,
    ))
}

fn cash_requirement_without_double_count(
    margin_requirement: Decimal,
    put_margin: Decimal,
    delivery_cash: Decimal,
    context: &'static str,
) -> Result<Decimal, EngineError> {
    margin_requirement
        .checked_sub(put_margin)
        .and_then(|other_margin| other_margin.checked_add(delivery_cash.max(put_margin)))
        .ok_or(EngineError::ArithmeticOverflow(context))
}

fn validate_underlying_collateral(
    instruments: &HashMap<String, Instrument>,
    positions: &HashMap<String, InstrumentPosition>,
    reservations: &HashMap<String, Decimal>,
    override_position: Option<(&str, Decimal)>,
) -> Result<(), EngineError> {
    for (underlying_id, required) in reservations {
        if !matches!(instruments.get(underlying_id), Some(Instrument::Spot(_))) {
            return Err(EngineError::InvalidDeliveryUnderlying(
                underlying_id.clone(),
            ));
        }
        let available = match override_position {
            Some((override_id, quantity)) if override_id == underlying_id => quantity,
            _ => {
                positions
                    .get(underlying_id)
                    .expect("configured underlying position exists")
                    .quantity
            }
        };
        if available < *required {
            return Err(EngineError::InsufficientUnderlyingCollateral(
                underlying_id.clone(),
            ));
        }
    }
    Ok(())
}

fn checked_magnitude(value: Decimal) -> Option<Decimal> {
    if value < Decimal::ZERO {
        Decimal::ZERO.checked_sub(value)
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::run_configuration::RunConfiguration;
    use factorsbt_data::FixtureRegistry;
    use std::path::Path;

    fn baseline() -> (RunConfiguration, Fixture) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        let fixture = (*registry.get_by("mvp-synthetic", "v1").unwrap()).clone();
        let configuration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        (configuration, fixture)
    }

    #[test]
    fn initializes_zero_positions_at_available_start_marks() {
        let (configuration, fixture) = baseline();
        let state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        let spot = state.position("SPOT_A").unwrap();
        assert_eq!(state.cash(), Decimal::from(100000));
        assert_eq!(spot.quantity, Decimal::ZERO);
        assert_eq!(spot.mark, Decimal::from(100));
        assert_eq!(spot.settlement_basis, None);
        assert_eq!(spot.mark_observation_id, "BAR-SPOT_A-20260105");
    }

    #[test]
    fn initial_marks_are_not_visible_before_their_availability_time() {
        let (mut configuration, fixture) = baseline();
        configuration.simulation.start_at = UtcTimestamp::new("2026-01-05T15:59:59Z").unwrap();
        assert!(matches!(
            PortfolioState::initialize(&configuration, &fixture),
            Err(EngineError::MissingInitialMark(_))
        ));
    }

    #[test]
    fn futures_start_mark_is_the_settlement_basis_without_entry_cashflow() {
        let (mut configuration, fixture) = baseline();
        let future: Instrument = serde_json::from_str(
            r#"{"instrument_type":"linear_future","instrument_id":"FUT_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","expiry_at":"2026-01-09T16:00:00Z","initial_margin_per_contract":"1000","maintenance_margin_per_contract":"800"}"#,
        )
        .unwrap();
        configuration.instruments.push(future);
        configuration
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"FUT_A","quantity":"1"}"#).unwrap());
        configuration.initial_portfolio.cash =
            factorsbt_contracts::DecimalString::new("1000").unwrap();

        let state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        let future = state.position("FUT_A").unwrap();
        assert_eq!(state.cash(), Decimal::from(1000));
        assert_eq!(future.mark, Decimal::from(100));
        assert_eq!(future.settlement_basis, Some(Decimal::from(100)));
        assert_eq!(state.initial_margin_requirement(), Decimal::from(1000));
        assert_eq!(state.maintenance_margin_requirement(), Decimal::from(800));
    }

    #[test]
    fn initial_portfolio_below_margin_is_rejected_before_run_admission() {
        let (mut configuration, fixture) = baseline();
        let future: Instrument = serde_json::from_str(
            r#"{"instrument_type":"linear_future","instrument_id":"FUT_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","expiry_at":"2026-01-09T16:00:00Z","initial_margin_per_contract":"1000","maintenance_margin_per_contract":"800"}"#,
        )
        .unwrap();
        configuration.instruments.push(future);
        configuration
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"FUT_A","quantity":"1"}"#).unwrap());
        configuration.initial_portfolio.cash =
            factorsbt_contracts::DecimalString::new("999.99").unwrap();

        assert!(matches!(
            PortfolioState::initialize(&configuration, &fixture),
            Err(EngineError::InsufficientInitialMargin)
        ));
    }

    #[test]
    fn ledger_postings_round_cash_half_even_and_apply_positions_together() {
        let (configuration, fixture) = baseline();
        let mut state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        let posting = state
            .apply_ledger_posting(Some("SPOT_A"), Decimal::new(-100005, 3), Decimal::ONE)
            .unwrap();
        assert_eq!(posting.cash_before, Decimal::from(100000));
        assert_eq!(posting.cash_delta, Decimal::from(-100));
        assert_eq!(posting.cash_after, Decimal::from(99900));
        assert_eq!(posting.quantity_after, Decimal::ONE);

        let half_even = state
            .apply_ledger_posting(None, Decimal::new(5, 3), Decimal::ZERO)
            .unwrap();
        assert_eq!(half_even.cash_after, Decimal::from(99900));
        assert_eq!(half_even.cash_delta, Decimal::ZERO);
    }

    #[test]
    fn invalid_position_posting_is_atomic() {
        let (configuration, fixture) = baseline();
        let mut state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        let result =
            state.apply_ledger_posting(Some("SPOT_A"), Decimal::from(100), Decimal::from(-1));
        assert!(matches!(
            result,
            Err(EngineError::InvalidPostingQuantity(_))
        ));
        assert_eq!(state.cash(), Decimal::from(100000));
        assert_eq!(state.position("SPOT_A").unwrap().quantity, Decimal::ZERO);
    }

    #[test]
    fn cash_overflow_leaves_the_ledger_unchanged() {
        let (configuration, fixture) = baseline();
        let mut state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        assert!(matches!(
            state.apply_ledger_posting(None, Decimal::MAX, Decimal::ZERO),
            Err(EngineError::ArithmeticOverflow("cash posting"))
        ));
        assert_eq!(state.cash(), Decimal::from(100000));
    }

    #[test]
    fn physical_short_call_requires_distinct_underlying_shares() {
        let (mut configuration, fixture) = baseline();
        configuration.instruments.push(
            serde_json::from_str(r#"{"instrument_type":"option","instrument_id":"AM_CALL_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"100","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","option_type":"call","strike":"100","expiry_at":"2026-01-09T16:00:00Z","exercise_style":"american","settlement_style":"physical","valuation_mode":"quote","initial_margin_per_contract":"600","maintenance_margin_per_contract":"600"}"#)
                .unwrap(),
        );
        configuration.initial_portfolio.positions.push(
            serde_json::from_str(r#"{"instrument_id":"AM_CALL_A","quantity":"-1"}"#).unwrap(),
        );
        assert!(matches!(
            PortfolioState::initialize(&configuration, &fixture),
            Err(EngineError::InsufficientUnderlyingCollateral(_))
        ));

        configuration
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"SPOT_A","quantity":"100"}"#).unwrap());
        let mut state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        assert_eq!(state.reserved_underlying()["SPOT_A"], Decimal::from(100));
        assert_eq!(state.initial_margin_requirement(), Decimal::from(600));
        assert_eq!(state.cash_encumbrance(), Decimal::from(600));

        let result =
            state.apply_ledger_posting(Some("SPOT_A"), Decimal::from(10000), Decimal::from(-100));
        assert!(matches!(
            result,
            Err(EngineError::InsufficientUnderlyingCollateral(_))
        ));
        assert_eq!(
            state.position("SPOT_A").unwrap().quantity,
            Decimal::from(100)
        );
        assert_eq!(state.cash(), Decimal::from(100000));
    }

    #[test]
    fn short_put_cash_collateral_is_not_double_counted_with_margin() {
        let (mut configuration, fixture) = baseline();
        configuration.instruments.push(
            serde_json::from_str(r#"{"instrument_type":"option","instrument_id":"AM_PUT_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"100","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","option_type":"put","strike":"100","expiry_at":"2026-01-09T16:00:00Z","exercise_style":"american","settlement_style":"physical","valuation_mode":"quote","initial_margin_per_contract":"600","maintenance_margin_per_contract":"500"}"#)
                .unwrap(),
        );
        configuration
            .initial_portfolio
            .positions
            .push(serde_json::from_str(r#"{"instrument_id":"AM_PUT_A","quantity":"-1"}"#).unwrap());
        let state = PortfolioState::initialize(&configuration, &fixture).unwrap();
        assert_eq!(state.initial_margin_requirement(), Decimal::from(600));
        assert_eq!(state.maintenance_margin_requirement(), Decimal::from(500));
        assert_eq!(state.reserved_cash(), Decimal::from(10000));
        assert_eq!(state.cash_encumbrance(), Decimal::from(10000));
        assert_eq!(state.maintenance_cash_encumbrance(), Decimal::from(10000));

        configuration.initial_portfolio.cash =
            factorsbt_contracts::DecimalString::new("9999.99").unwrap();
        assert!(matches!(
            PortfolioState::initialize(&configuration, &fixture),
            Err(EngineError::InsufficientDeliveryCash)
        ));
    }
}
