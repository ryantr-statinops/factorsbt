//! Typed observation, snapshot, and portfolio data used across contracts.

use crate::{DecimalString, Identifier, InstrumentId, UtcTimestamp, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Bar,
    Quote,
    News,
    OptionPricingInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub observation_id: Identifier,
    pub kind: ObservationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<InstrumentId>,
    pub source_id: Identifier,
    pub source_version: Identifier,
    pub revision: u64,
    pub event_time: UtcTimestamp,
    pub available_at: UtcTimestamp,
    pub payload: ObservationPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObservationPayload {
    Bar(BarPayload),
    Quote(QuotePayload),
    News(NewsPayload),
    OptionPricingInput(OptionPricingInputPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarPayload {
    pub interval_start: UtcTimestamp,
    pub interval_end: UtcTimestamp,
    pub open: DecimalString,
    pub high: DecimalString,
    pub low: DecimalString,
    pub close: DecimalString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<DecimalString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotePayload {
    pub bid: DecimalString,
    pub ask: DecimalString,
    pub mark: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewsPayload {
    pub headline: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionPricingInputPayload {
    pub risk_free_rate: DecimalString,
    pub annualized_volatility: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionSnapshot {
    pub snapshot_id: Uuid,
    pub run_id: Uuid,
    pub scenario_id: Identifier,
    pub step_id: Identifier,
    pub simulation_time: UtcTimestamp,
    pub assembled_at: UtcTimestamp,
    pub observations: Vec<Observation>,
    pub portfolio: PortfolioSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioSnapshot {
    pub cash: DecimalString,
    pub nav: DecimalString,
    pub positions: Vec<PositionSnapshot>,
    pub reserved_cash: DecimalString,
    pub reserved_underlying: Vec<UnderlyingReservation>,
    pub initial_margin_requirement: DecimalString,
    pub maintenance_margin_requirement: DecimalString,
    pub free_buying_power: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionSnapshot {
    pub instrument_id: InstrumentId,
    pub quantity: DecimalString,
    pub mark: DecimalString,
    pub market_value: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnderlyingReservation {
    pub instrument_id: InstrumentId,
    pub quantity: DecimalString,
}
