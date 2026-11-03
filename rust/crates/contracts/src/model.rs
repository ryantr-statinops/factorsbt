//! Correlated model request and response payloads.

use crate::instruments::Instrument;
use crate::temporal::DecisionSnapshot;
use crate::{DecimalString, Identifier, InstrumentId, UtcTimestamp, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Scores,
    Targets,
    OrderIntents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionPolicy {
    pub output_mode: OutputMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_policy: Option<ScorePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorePolicy {
    pub top_k: u32,
    pub derivative_score_targets: Vec<DerivativeScoreTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivativeScoreTarget {
    pub instrument_id: InstrumentId,
    pub contracts: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRequest {
    pub contract_version: String,
    pub inference_id: Uuid,
    pub run_id: Uuid,
    pub scenario_id: Identifier,
    pub step_id: Identifier,
    pub snapshot_id: Uuid,
    pub model_id: Identifier,
    pub model_version: Identifier,
    pub requested_at: UtcTimestamp,
    pub simulation_time: UtcTimestamp,
    pub decision_policy: DecisionPolicy,
    pub eligible_instrument_ids: Vec<InstrumentId>,
    pub instruments: Vec<Instrument>,
    pub snapshot: DecisionSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response_type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelResponse {
    Scores {
        contract_version: String,
        inference_id: Uuid,
        run_id: Uuid,
        scenario_id: Identifier,
        step_id: Identifier,
        snapshot_id: Uuid,
        model_id: Identifier,
        model_version: Identifier,
        responded_at: UtcTimestamp,
        outputs: ScoresOutput,
    },
    Targets {
        contract_version: String,
        inference_id: Uuid,
        run_id: Uuid,
        scenario_id: Identifier,
        step_id: Identifier,
        snapshot_id: Uuid,
        model_id: Identifier,
        model_version: Identifier,
        responded_at: UtcTimestamp,
        outputs: TargetsOutput,
    },
    OrderIntents {
        contract_version: String,
        inference_id: Uuid,
        run_id: Uuid,
        scenario_id: Identifier,
        step_id: Identifier,
        snapshot_id: Uuid,
        model_id: Identifier,
        model_version: Identifier,
        responded_at: UtcTimestamp,
        outputs: OrderIntentsOutput,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoresOutput {
    pub scores: Vec<InstrumentScore>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentScore {
    pub instrument_id: InstrumentId,
    pub score: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetsOutput {
    pub spot_weights: Vec<SpotWeight>,
    pub derivative_contracts: Vec<DerivativeTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpotWeight {
    pub instrument_id: InstrumentId,
    pub weight: DecimalString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivativeTarget {
    pub instrument_id: InstrumentId,
    pub contracts: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderIntentsOutput {
    pub orders: Vec<OrderIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderIntent {
    pub instrument_id: InstrumentId,
    pub order_type: MarketOrderType,
    pub quantity: DecimalString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketOrderType {
    #[serde(rename = "market")]
    Market,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_response_keeps_snapshot_correlation_and_decimal_string() {
        let json = r#"{
            "contract_version":"1.0.0","inference_id":"de87564a-ae4a-4768-bb05-97bc1d816d8f",
            "run_id":"c827fcbc-c5fc-48a4-b4aa-21a63f64727e","scenario_id":"baseline",
            "step_id":"step-0001","snapshot_id":"4fe68f00-0ddb-4e72-ad9b-7bc09d84d5aa",
            "model_id":"synthetic-model","model_version":"v1","responded_at":"2026-01-05T16:00:02Z",
            "response_type":"scores","outputs":{"scores":[{"instrument_id":"SPOT_A","score":"2.5"}]}
        }"#;
        let response: ModelResponse = serde_json::from_str(json).unwrap();
        match response {
            ModelResponse::Scores {
                step_id, outputs, ..
            } => {
                assert_eq!(step_id.as_str(), "step-0001");
                assert_eq!(outputs.scores[0].score.as_str(), "2.5");
            }
            _ => panic!("expected scores response"),
        }
    }

    #[test]
    fn each_output_mode_has_a_distinct_shape() {
        let valid = r#"{"contract_version":"1.0.0","inference_id":"de87564a-ae4a-4768-bb05-97bc1d816d8f","run_id":"c827fcbc-c5fc-48a4-b4aa-21a63f64727e","scenario_id":"baseline","step_id":"step-0001","snapshot_id":"4fe68f00-0ddb-4e72-ad9b-7bc09d84d5aa","model_id":"synthetic-model","model_version":"v1","responded_at":"2026-01-05T16:00:02Z","response_type":"targets","outputs":{"spot_weights":[{"instrument_id":"SPOT_A","weight":"1"}],"derivative_contracts":[]}}"#;
        assert!(serde_json::from_str::<ModelResponse>(valid).is_ok());
        let invalid = valid.replace("spot_weights", "orders");
        assert!(serde_json::from_str::<ModelResponse>(&invalid).is_err());
    }
}
