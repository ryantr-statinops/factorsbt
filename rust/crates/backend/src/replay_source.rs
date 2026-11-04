//! Load a sealed run from local evidence without relying on process metadata.

use crate::evidence::{
    executable_sha256, read_evidence_index, read_verified_artifact, EvidenceError, EvidenceIndex,
};
use factorsbt_contracts::{RunConfiguration, Uuid};
use factorsbt_data::Fixture;
use serde_json::Value;
use std::{error::Error, fmt, path::Path};

#[derive(Debug)]
pub enum ReplaySourceError {
    Evidence(EvidenceError),
    MissingArtifact(&'static str),
    InvalidArtifact(&'static str),
    InvalidFixture(String),
    Incompatible(&'static str),
    Incomplete(&'static str),
}

impl fmt::Display for ReplaySourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => error.fmt(f),
            Self::MissingArtifact(name) => write!(f, "replay source is missing {name}"),
            Self::InvalidArtifact(name) => write!(f, "replay source contains invalid {name}"),
            Self::InvalidFixture(reason) => write!(f, "replay source fixture is invalid: {reason}"),
            Self::Incompatible(reason) => write!(f, "replay source is incompatible: {reason}"),
            Self::Incomplete(reason) => write!(f, "replay source is incomplete: {reason}"),
        }
    }
}

impl Error for ReplaySourceError {}

impl From<EvidenceError> for ReplaySourceError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

pub struct ReplaySource {
    pub index: EvidenceIndex,
    pub manifest: Value,
    pub configuration: RunConfiguration,
    pub fixture: Fixture,
}

impl ReplaySource {
    pub fn load(root: &Path, run_id: &Uuid) -> Result<Self, ReplaySourceError> {
        let index = read_evidence_index(root, run_id)?;
        let manifest: Value = serde_json::from_slice(&artifact(
            root,
            run_id,
            &index,
            "evidence/run-manifest.json",
        )?)
        .map_err(|_| ReplaySourceError::InvalidArtifact("run manifest"))?;
        let configuration: RunConfiguration = serde_json::from_slice(&artifact(
            root,
            run_id,
            &index,
            "evidence/configuration.json",
        )?)
        .map_err(|_| ReplaySourceError::InvalidArtifact("configuration"))?;
        let fixture = Fixture::from_evidence_bytes(
            artifact(root, run_id, &index, "evidence/fixture/manifest.json")?,
            artifact(root, run_id, &index, "evidence/fixture/dataset.json")?,
        )
        .map_err(|error| ReplaySourceError::InvalidFixture(error.to_string()))?;
        if configuration.data.fixture_id != fixture.manifest.fixture_id
            || configuration.data.fixture_version != fixture.manifest.fixture_version
            || manifest["fixture"]["dataset_sha256"] != fixture.digest_sha256
        {
            return Err(ReplaySourceError::InvalidArtifact("fixture identity"));
        }
        Ok(Self {
            index,
            manifest,
            configuration,
            fixture,
        })
    }

    pub fn artifact(&self, root: &Path, relative_path: &str) -> Result<Vec<u8>, ReplaySourceError> {
        artifact(root, &self.index.run_id, &self.index, relative_path)
    }

    pub fn recorded_responses(&self, root: &Path) -> Result<Vec<Vec<u8>>, ReplaySourceError> {
        let mut refs = self
            .index
            .artifacts
            .iter()
            .filter(|item| {
                item.kind == factorsbt_contracts::outcomes::EvidenceKind::ModelResponse
                    && item.relative_path.starts_with("evidence/model-responses/")
                    && item.relative_path != "evidence/model-responses/failed-inference.json"
            })
            .collect::<Vec<_>>();
        refs.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        refs.into_iter()
            .map(|item| self.artifact(root, &item.relative_path))
            .collect()
    }

    /// Eligibility is separate from loading so read-only evidence inspection can
    /// still work for older runs that cannot be replayed.
    pub fn verify_replay_eligibility(&self, root: &Path) -> Result<(), ReplaySourceError> {
        if self.manifest["format_version"] != "2.0.0" {
            return Err(ReplaySourceError::Incompatible("evidence format"));
        }
        if self.manifest["contract_version"] != "1.0.0"
            || self.configuration.contract_version != "1.0.0"
        {
            return Err(ReplaySourceError::Incompatible("contract version"));
        }
        if self.manifest["executable_sha256"] != executable_sha256()? {
            return Err(ReplaySourceError::Incompatible("backend executable"));
        }
        if self.manifest["status"] != self.index.status {
            return Err(ReplaySourceError::Incomplete("terminal status"));
        }
        if self.index.status == "succeeded" && !self.manifest["failure"].is_null() {
            return Err(ReplaySourceError::Incomplete(
                "successful source has failure",
            ));
        }
        if self.index.status == "failed" {
            let failure: Value =
                serde_json::from_slice(&self.artifact(root, "evidence/failure.json")?)
                    .map_err(|_| ReplaySourceError::Incomplete("terminal failure"))?;
            if failure["code"] != "RUN_EXECUTION_FAILED"
                || failure["stage"] != self.manifest["failure"]["stage"]
                || failure["reason"] != self.manifest["failure"]["reason"]
            {
                return Err(ReplaySourceError::Incomplete("terminal failure identity"));
            }
        }
        let has_summary = self
            .index
            .artifacts
            .iter()
            .any(|artifact| artifact.relative_path == "evidence/summary.json");
        if self.index.status == "succeeded" || has_summary {
            self.artifact(root, "evidence/summary.json")?;
            for path in [
                "evidence/admissions.json",
                "evidence/orders.json",
                "evidence/executions.json",
                "evidence/settlements.json",
                "evidence/ledger-postings.json",
            ] {
                self.artifact(root, path)?;
            }
        }
        for request in self.index.artifacts.iter().filter(|a| {
            a.relative_path.starts_with("evidence/model-requests/")
                && a.relative_path.ends_with(".json")
        }) {
            if request.relative_path == "evidence/model-requests/failed-inference.json" {
                continue;
            }
            let response_path =
                request
                    .relative_path
                    .replacen("model-requests", "model-responses", 1);
            let decision_path = request
                .relative_path
                .replacen("model-requests", "decisions", 1);
            let snapshot_path = request
                .relative_path
                .replacen("model-requests", "snapshots", 1);
            for path in [response_path, decision_path, snapshot_path] {
                if !self
                    .index
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.relative_path == path)
                {
                    return Err(ReplaySourceError::Incomplete("decision trace"));
                }
                self.artifact(root, &path)?;
            }
        }
        Ok(())
    }
}

fn artifact(
    root: &Path,
    run_id: &Uuid,
    index: &EvidenceIndex,
    path: &str,
) -> Result<Vec<u8>, ReplaySourceError> {
    let reference = index
        .artifacts
        .iter()
        .find(|item| item.relative_path == path)
        .ok_or(ReplaySourceError::MissingArtifact("required artifact"))?;
    let (_, bytes) = read_verified_artifact(root, run_id, &reference.artifact_id)?;
    Ok(bytes)
}
