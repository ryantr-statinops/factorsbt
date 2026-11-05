//! Wire types and deterministic execution for bounded research requests.

use super::{
    episode::{run_episode, EpisodeAction, EpisodeReport},
    fixtures::resolve_fixture,
    reranking::{compare_reranking, RerankingReport},
};
use factorsbt_contracts::{UtcTimestamp, Uuid};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchJobKind { Reranking, Episode, Replay }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchJobRequest {
    pub contract_version: String,
    pub kind: ResearchJobKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_job_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchJobFailure { pub code: String, pub message: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchJobStatus {
    pub contract_version: String,
    pub job_id: Uuid,
    pub kind: ResearchJobKind,
    pub status: String,
    pub submitted_at: UtcTimestamp,
    pub started_at: Option<UtcTimestamp>,
    pub finished_at: Option<UtcTimestamp>,
    pub failure: Option<ResearchJobFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchJobAcknowledgement {
    pub contract_version: String,
    pub job_id: Uuid,
    pub kind: ResearchJobKind,
    pub status: String,
    pub submitted_at: UtcTimestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchJobResult {
    pub contract_version: String,
    pub job_id: Uuid,
    pub kind: ResearchJobKind,
    pub report: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchJobError { InvalidContract, InvalidRequest(&'static str), Fixture(String), Episode(String), ReplayUnsupported }

impl fmt::Display for ResearchJobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract => formatter.write_str("research contract version is unsupported"),
            Self::InvalidRequest(reason) => formatter.write_str(reason),
            Self::Fixture(reason) | Self::Episode(reason) => formatter.write_str(reason),
            Self::ReplayUnsupported => formatter.write_str("research replay source is unavailable"),
        }
    }
}
impl Error for ResearchJobError {}

/// Validate a request before admitting it to the backend queue.
pub fn validate_request(request: &ResearchJobRequest) -> Result<(), ResearchJobError> {
    if request.contract_version != "1.0.0" { return Err(ResearchJobError::InvalidContract); }
    match request.kind {
        ResearchJobKind::Reranking => {
            if request.fixture_id.is_none() || request.fixture_version.is_none() || request.actions.is_some() || request.source_job_id.is_some() {
                return Err(ResearchJobError::InvalidRequest("reranking requires a fixture reference only"));
            }
        }
        ResearchJobKind::Episode => {
            if request.fixture_id.is_none() || request.fixture_version.is_none() || request.actions.as_ref().is_none_or(|actions| actions.len() != 2) || request.source_job_id.is_some() {
                return Err(ResearchJobError::InvalidRequest("episode requires a fixture and exactly two actions"));
            }
            if request.actions.as_ref().is_some_and(|actions| actions.iter().any(|action| !matches!(action.as_str(), "0" | "0.0" | "0.00" | "0.5" | "0.50" | "1" | "1.0" | "1.00"))) {
                return Err(ResearchJobError::InvalidRequest("episode actions must be 0, 0.5, or 1"));
            }
        }
        ResearchJobKind::Replay => {
            if request.source_job_id.is_none() || request.fixture_id.is_some() || request.fixture_version.is_some() || request.actions.is_some() {
                return Err(ResearchJobError::InvalidRequest("replay requires a source job ID only"));
            }
        }
    }
    Ok(())
}

/// Execute a validated research request without model or provider I/O.
pub fn execute_request(request: &ResearchJobRequest) -> Result<Value, ResearchJobError> {
    validate_request(request)?;
    if request.kind == ResearchJobKind::Replay { return Err(ResearchJobError::ReplayUnsupported); }
    let fixture = resolve_fixture(request.fixture_id.as_deref().unwrap_or_default(), request.fixture_version.as_deref().unwrap_or_default()).map_err(|error| ResearchJobError::Fixture(error.to_string()))?;
    execute_with_fixture(request, &fixture)
}

pub fn execute_with_fixture(request: &ResearchJobRequest, fixture: &super::fixtures::ResearchFixture) -> Result<Value, ResearchJobError> {
    match request.kind {
        ResearchJobKind::Reranking => {
            let report: RerankingReport = compare_reranking(fixture);
            serde_json::to_value(report).map_err(|_| ResearchJobError::Fixture("could not encode reranking report".into()))
        }
        ResearchJobKind::Episode => {
            let actions = request.actions.as_ref().expect("validated episode actions").iter().map(|raw| {
                Decimal::from_str(raw).map(|target_weight| EpisodeAction { target_weight }).map_err(|_| ResearchJobError::Episode("episode action is not a decimal".into()))
            }).collect::<Result<Vec<_>, _>>()?;
            let report: EpisodeReport = run_episode(fixture, &actions).map_err(|error| ResearchJobError::Episode(error.to_string()))?;
            serde_json::to_value(report).map_err(|_| ResearchJobError::Episode("could not encode episode report".into()))
        }
        ResearchJobKind::Replay => Err(ResearchJobError::ReplayUnsupported),
    }
}
