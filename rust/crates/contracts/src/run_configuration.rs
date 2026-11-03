//! Typed public MVP run configuration payloads.

use crate::instruments::Instrument;
use crate::model::DecisionPolicy;
use crate::outcomes::DataFixtureRef;
use crate::{DecimalString, InstrumentId, UtcTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunConfiguration {
    pub contract_version: String,
    pub simulation: SimulationConfiguration,
    pub initial_portfolio: InitialPortfolio,
    pub instruments: Vec<Instrument>,
    pub data: DataFixtureRef,
    pub model: ModelConfiguration,
    pub decision_policy: DecisionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationConfiguration {
    pub start_at: UtcTimestamp,
    pub end_at: UtcTimestamp,
    pub calendar_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialPortfolio {
    pub cash: DecimalString,
    pub positions: Vec<InitialPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialPosition {
    pub instrument_id: InstrumentId,
    pub quantity: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfiguration {
    pub adapter: ModelAdapter,
    pub model_id: crate::Identifier,
    pub model_version: crate::Identifier,
    pub endpoint: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelAdapter {
    #[serde(rename = "http_fixture")]
    HttpFixture,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_the_shared_run_configuration_example() {
        let config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        assert_eq!(config.contract_version, "1.0.0");
        assert_eq!(config.initial_portfolio.cash.as_str(), "100000");
        assert_eq!(config.instruments.len(), 1);
    }
}
