//! Run operations, engine outcome, result, and evidence wire payloads.

use crate::model::OutputMode;
use crate::{DecimalString, Identifier, InstrumentId, UtcTimestamp, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLifecycle {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub service: BackendService,
    pub contract_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    #[serde(rename = "ok")]
    Ok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendService {
    #[serde(rename = "factorsbt-backend")]
    FactorsbtBackend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionAccepted {
    pub submission_id: Uuid,
    pub run_id: Uuid,
    pub status: QueuedStatus,
    pub submitted_at: UtcTimestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueuedStatus {
    #[serde(rename = "queued")]
    Queued,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionStatus {
    pub submission_id: Uuid,
    pub run_id: Uuid,
    pub status: RunLifecycle,
    pub submitted_at: UtcTimestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStatus {
    pub run_id: Uuid,
    pub status: RunLifecycle,
    pub submitted_at: UtcTimestamp,
    pub started_at: Option<UtcTimestamp>,
    pub finished_at: Option<UtcTimestamp>,
    pub progress: Option<RunProgress>,
    pub failure: Option<RunFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunProgress {
    pub scenario_id: Identifier,
    pub step_id: Identifier,
    pub simulation_time: UtcTimestamp,
    pub completed_steps: u64,
    pub total_steps: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunFailure {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_valid_state_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiErrorResponse {
    pub error: ApiError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub request_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ValidationDetail>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationDetail {
    pub path: String,
    pub reason: String,
}

macro_rules! engine_event {
    ($name:ident { $($fields:tt)* }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub event_id: Uuid,
            pub run_id: Uuid,
            pub scenario_id: Identifier,
            pub step_id: Identifier,
            pub sequence: u64,
            pub simulation_time: UtcTimestamp,
            $($fields)*
        }
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EngineEvent {
    DecisionAdmission(DecisionAdmissionEvent),
    OrderCreated(OrderCreatedEvent),
    OrderCanceled(OrderCanceledEvent),
    Fill(FillEvent),
    LedgerPosting(LedgerPostingEvent),
    Settlement(SettlementEvent),
    Risk(RiskEvent),
}

engine_event!(DecisionAdmissionEvent {
    pub event_type: DecisionAdmissionType,
    pub decision_id: Uuid,
    pub inference_id: Uuid,
    pub snapshot_id: Uuid,
    pub response_type: OutputMode,
    pub status: AdmissionStatus,
    pub scale_factor: DecimalString,
    pub funding_constraint: FundingConstraint,
    pub entries: Vec<AdmissionEntry>,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionAdmissionType {
    #[serde(rename = "decision_admission")]
    DecisionAdmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionStatus {
    Accepted,
    Adjusted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FundingConstraint {
    None,
    SpotCash,
    InitialMargin,
    OptionMargin,
    DeliveryCash,
    DeliveryUnderlying,
    Fees,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionEntry {
    pub instrument_id: InstrumentId,
    pub unit: AdmissionUnit,
    pub requested_value: DecimalString,
    pub admitted_value: DecimalString,
    pub adjustment: AdmissionAdjustment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionUnit {
    SpotWeight,
    Quantity,
    Contracts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionAdjustment {
    None,
    LotRounding,
    CapitalScale,
    PolicyRejection,
}

engine_event!(OrderCreatedEvent {
    pub event_type: OrderCreatedType,
    pub order_id: Uuid,
    pub decision_id: Uuid,
    pub instrument_id: InstrumentId,
    pub order_type: MarketOrderType,
    pub signed_quantity: DecimalString,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderCreatedType {
    #[serde(rename = "order_created")]
    OrderCreated,
}

engine_event!(OrderCanceledEvent {
    pub event_type: OrderCanceledType,
    pub order_id: Uuid,
    pub cancel_reason: CancelReason,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderCanceledType {
    #[serde(rename = "order_canceled")]
    OrderCanceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelReason {
    EndOfRun,
    MaintenanceBreach,
    LiquidationReplaced,
}

engine_event!(FillEvent {
    pub event_type: FillType,
    pub fill_id: Uuid,
    pub order_id: Uuid,
    pub instrument_id: InstrumentId,
    pub signed_quantity: DecimalString,
    pub fill_price: DecimalString,
    pub fee: DecimalString,
    pub source_observation_id: Identifier,
    pub source_revision: u64,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillType {
    #[serde(rename = "fill")]
    Fill,
}

engine_event!(LedgerPostingEvent {
    pub event_type: LedgerPostingType,
    pub posting_id: Uuid,
    pub source_event_id: Uuid,
    pub posting_type: PostingKind,
    pub instrument_id: InstrumentId,
    pub cash_before: DecimalString,
    pub cash_delta: DecimalString,
    pub cash_after: DecimalString,
    pub position_quantity_before: DecimalString,
    pub position_quantity_delta: DecimalString,
    pub position_quantity_after: DecimalString,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerPostingType {
    #[serde(rename = "ledger_posting")]
    LedgerPosting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostingKind {
    SpotTrade,
    OptionTrade,
    FuturesVariation,
    FuturesCloseRealizedVariation,
    FuturesExpirySettlement,
    OptionCashExpiry,
    OptionDelivery,
    Fee,
}

engine_event!(SettlementEvent {
    pub event_type: SettlementType,
    pub settlement_id: Uuid,
    pub instrument_id: InstrumentId,
    pub settlement_kind: SettlementKind,
    pub contracts: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_event_id: Option<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_id: Option<InstrumentId>,
    pub settlement_cash_amount: DecimalString,
    pub underlying_delivery_quantity: DecimalString,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementType {
    #[serde(rename = "settlement")]
    Settlement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementKind {
    FuturesVariation,
    FuturesExpiry,
    EuropeanCashExpiry,
    AmericanExercise,
    AmericanAssignment,
    AmericanExpiryWorthless,
}

engine_event!(RiskEvent {
    pub event_type: RiskEventType,
    pub risk_code: RiskCode,
    pub severity: RiskSeverity,
    pub action: RiskAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<UtcTimestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash: Option<DecimalString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav: Option<DecimalString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_requirement: Option<DecimalString>,
    pub causing_event_ids: Vec<Uuid>,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskEventType {
    #[serde(rename = "risk")]
    Risk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCode {
    MaintenanceBreach,
    ForcedLiquidationScheduled,
    ForcedLiquidationCompleted,
    ForcedLiquidationFailed,
    CollateralShortfall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskAction {
    RejectExposure,
    CancelOrdersAndLiquidate,
    ContinueRun,
    FailRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketOrderType {
    #[serde(rename = "market")]
    Market,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResult {
    pub contract_version: String,
    pub run_id: Uuid,
    pub status: SuccessfulStatus,
    pub scenario_id: Identifier,
    pub output_mode: OutputMode,
    pub simulation_start: UtcTimestamp,
    pub simulation_end: UtcTimestamp,
    pub finished_at: UtcTimestamp,
    pub initial_cash: DecimalString,
    pub total_fees: DecimalString,
    pub counts: ResultCounts,
    pub portfolio: serde_json::Value,
    pub provenance: ResultProvenance,
    pub evidence: Vec<EvidenceArtifactRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuccessfulStatus {
    #[serde(rename = "succeeded")]
    Succeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultCounts {
    pub decision_steps: u64,
    pub admissions: u64,
    pub orders: u64,
    pub fills: u64,
    pub settlements: u64,
    pub ledger_postings: u64,
    pub risk_events: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultProvenance {
    pub data_fixtures: Vec<DataFixtureRef>,
    pub model_invocation: ModelInvocation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataFixtureRef {
    pub fixture_id: Identifier,
    pub fixture_version: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelInvocation {
    pub mode: ModelInvocationMode,
    pub model_id: Identifier,
    pub model_version: Identifier,
    pub inference_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelInvocationMode {
    FreshInference,
    RecordedResponseReplay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceArtifactRef {
    pub artifact_id: Uuid,
    pub kind: EvidenceKind,
    pub relative_path: String,
    pub content_type: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    RunManifest,
    Configuration,
    Observation,
    Snapshot,
    ModelRequest,
    ModelResponse,
    DecisionAdmission,
    Order,
    Fill,
    Settlement,
    Ledger,
    Risk,
    Summary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_and_lifecycle_payloads_round_trip() {
        let health: HealthResponse = serde_json::from_str(
            r#"{"status":"ok","service":"factorsbt-backend","contract_version":"1.0.0"}"#,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(health).unwrap()["status"], "ok");
        let status: RunStatus = serde_json::from_str(r#"{"run_id":"c827fcbc-c5fc-48a4-b4aa-21a63f64727e","status":"queued","submitted_at":"2026-01-05T16:00:00Z","started_at":null,"finished_at":null,"progress":null,"failure":null}"#).unwrap();
        assert_eq!(status.status, RunLifecycle::Queued);
    }

    #[test]
    fn fill_event_keeps_source_revision_and_decimal_strings() {
        let event: EngineEvent = serde_json::from_str(r#"{"event_id":"9f2f32ac-5a39-4cc6-a183-b849c07518ca","run_id":"c827fcbc-c5fc-48a4-b4aa-21a63f64727e","scenario_id":"baseline","step_id":"step-0001","sequence":3,"simulation_time":"2026-01-06T09:00:00Z","event_type":"fill","fill_id":"701f9c31-39a4-4709-9885-f680aa73d145","order_id":"dc8795c0-d0df-4afe-b6dc-a806de048e5c","instrument_id":"SPOT_A","signed_quantity":"1.25","fill_price":"100.10","fee":"0","source_observation_id":"bar-a-1","source_revision":1}"#).unwrap();
        assert!(matches!(
            event,
            EngineEvent::Fill(FillEvent {
                source_revision: 1,
                ..
            })
        ));
    }
}
