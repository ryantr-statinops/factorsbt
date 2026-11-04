//! Wire payloads for independently tracked replay verification jobs.

use factorsbt_contracts::{outcomes::EvidenceArtifactRef, UtcTimestamp, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRequest {
    pub contract_version: String,
    pub source_run_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayAcknowledgement {
    pub contract_version: String,
    pub replay_id: Uuid,
    pub source_run_id: Uuid,
    pub status: &'static str,
    pub submitted_at: UtcTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayFailure {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayStatus {
    pub contract_version: String,
    pub replay_id: Uuid,
    pub source_run_id: Uuid,
    pub status: String,
    pub submitted_at: UtcTimestamp,
    pub started_at: Option<UtcTimestamp>,
    pub finished_at: Option<UtcTimestamp>,
    pub failure: Option<ReplayFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayReport {
    pub contract_version: String,
    pub replay_id: Uuid,
    pub source_run_id: Uuid,
    pub source_outcome: String,
    pub matched: bool,
    pub first_mismatch: Option<String>,
    pub source_transition_count: usize,
    pub replay_transition_count: usize,
    pub evidence_refs: Vec<EvidenceArtifactRef>,
}
