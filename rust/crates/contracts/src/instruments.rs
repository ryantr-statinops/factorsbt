//! Typed instrument configuration payloads.

use crate::{DecimalString, InstrumentId, UtcTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "instrument_type", rename_all = "snake_case")]
pub enum Instrument {
    Spot(SpotInstrument),
    LinearFuture(LinearFutureInstrument),
    Option(OptionInstrument),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpotInstrument {
    pub instrument_id: InstrumentId,
    pub quote_currency: Currency,
    pub price_tick: DecimalString,
    pub quantity_step: DecimalString,
    pub contract_multiplier: DecimalString,
    pub fee_bps: DecimalString,
    pub fee_per_unit: DecimalString,
    pub slippage_bps: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinearFutureInstrument {
    pub instrument_id: InstrumentId,
    pub quote_currency: Currency,
    pub price_tick: DecimalString,
    pub quantity_step: DecimalString,
    pub contract_multiplier: DecimalString,
    pub fee_bps: DecimalString,
    pub fee_per_unit: DecimalString,
    pub slippage_bps: DecimalString,
    pub underlying_id: InstrumentId,
    pub expiry_at: UtcTimestamp,
    pub initial_margin_per_contract: DecimalString,
    pub maintenance_margin_per_contract: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionInstrument {
    pub instrument_id: InstrumentId,
    pub quote_currency: Currency,
    pub price_tick: DecimalString,
    pub quantity_step: DecimalString,
    pub contract_multiplier: DecimalString,
    pub fee_bps: DecimalString,
    pub fee_per_unit: DecimalString,
    pub slippage_bps: DecimalString,
    pub underlying_id: InstrumentId,
    pub option_type: OptionType,
    pub strike: DecimalString,
    pub expiry_at: UtcTimestamp,
    pub exercise_style: ExerciseStyle,
    pub settlement_style: SettlementStyle,
    pub valuation_mode: ValuationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valuation_inputs: Option<OptionValuationInputs>,
    pub initial_margin_per_contract: DecimalString,
    pub maintenance_margin_per_contract: DecimalString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    #[serde(rename = "USD")]
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStyle {
    European,
    American,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStyle {
    Cash,
    Physical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValuationMode {
    Quote,
    Theoretical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pricer", rename_all = "snake_case")]
pub enum OptionValuationInputs {
    BlackScholes { day_count: DayCount },
    Crr { day_count: DayCount, crr_steps: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DayCount {
    #[serde(rename = "ACT/365F")]
    Act365F,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_instrument_preserves_tagged_wire_shape() {
        let json = r#"{
            "instrument_type":"spot","instrument_id":"SPOT_A","quote_currency":"USD",
            "price_tick":"0.01","quantity_step":"0.001","contract_multiplier":"1",
            "fee_bps":"0","fee_per_unit":"0","slippage_bps":"0"
        }"#;
        let instrument: Instrument = serde_json::from_str(json).unwrap();
        let encoded = serde_json::to_value(instrument).unwrap();
        assert_eq!(encoded["instrument_type"], "spot");
        assert_eq!(encoded["instrument_id"], "SPOT_A");
    }

    #[test]
    fn option_variant_deserializes_pricer_configuration() {
        let json = r#"{
            "instrument_type":"option","instrument_id":"CALL_A","quote_currency":"USD",
            "price_tick":"0.01","quantity_step":"1","contract_multiplier":"100",
            "fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A",
            "option_type":"call","strike":"100","expiry_at":"2026-02-01T16:00:00Z",
            "exercise_style":"european","settlement_style":"cash","valuation_mode":"theoretical",
            "valuation_inputs":{"pricer":"black_scholes","day_count":"ACT/365F"},
            "initial_margin_per_contract":"1000","maintenance_margin_per_contract":"800"
        }"#;
        let instrument: Instrument = serde_json::from_str(json).unwrap();
        assert!(matches!(
            instrument,
            Instrument::Option(OptionInstrument { .. })
        ));
    }

    #[test]
    fn instrument_rejects_unrecognized_fields() {
        let json = r#"{"instrument_type":"spot","instrument_id":"SPOT_A","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","unexpected":true}"#;
        assert!(serde_json::from_str::<Instrument>(json).is_err());
    }
}
