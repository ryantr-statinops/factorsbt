//! Load a sealed run from local evidence without relying on process metadata.

use crate::evidence::{read_evidence_index, read_verified_artifact, EvidenceError, EvidenceIndex};
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
}

impl fmt::Display for ReplaySourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => error.fmt(f),
            Self::MissingArtifact(name) => write!(f, "replay source is missing {name}"),
            Self::InvalidArtifact(name) => write!(f, "replay source contains invalid {name}"),
            Self::InvalidFixture(reason) => write!(f, "replay source fixture is invalid: {reason}"),
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
