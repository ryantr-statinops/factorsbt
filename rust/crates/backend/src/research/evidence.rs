//! Local, digest-checked evidence for bounded research jobs.

use super::{fixtures::ResearchFixture, jobs::{ResearchJobRequest, ResearchJobResult}};
use factorsbt_contracts::Uuid;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedArtifact {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchManifest {
    pub format_version: String,
    pub job_id: String,
    pub contract_version: String,
    pub executable_sha256: String,
    pub request: SealedArtifact,
    pub fixture: SealedArtifact,
    pub result: SealedArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchEvidenceError { Io, InvalidManifest, Incompatible, MissingArtifact, DigestMismatch }
impl fmt::Display for ResearchEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { write!(formatter, "research evidence error: {self:?}") }
}
impl Error for ResearchEvidenceError {}

#[derive(Debug, Clone)]
pub struct SealedResearchJob {
    pub request: ResearchJobRequest,
    pub fixture: ResearchFixture,
    pub result: ResearchJobResult,
}

fn executable_digest() -> Result<String, ResearchEvidenceError> {
    let path = std::env::current_exe().map_err(|_| ResearchEvidenceError::Io)?;
    digest(&fs::read(path).map_err(|_| ResearchEvidenceError::Io)?)
}

fn digest(bytes: &[u8]) -> Result<String, ResearchEvidenceError> {
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn write_artifact(directory: &Path, name: &str, bytes: &[u8]) -> Result<SealedArtifact, ResearchEvidenceError> {
    fs::write(directory.join(name), bytes).map_err(|_| ResearchEvidenceError::Io)?;
    Ok(SealedArtifact { relative_path: name.into(), size_bytes: bytes.len() as u64, sha256: digest(bytes)? })
}

pub fn write_job(root: &Path, request: &ResearchJobRequest, fixture: &ResearchFixture, result: &ResearchJobResult) -> Result<ResearchManifest, ResearchEvidenceError> {
    let directory = root.join("research_jobs").join(result.job_id.as_str());
    fs::create_dir_all(&directory).map_err(|_| ResearchEvidenceError::Io)?;
    let request_bytes = serde_json::to_vec(request).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    let fixture_bytes = serde_json::to_vec(fixture).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    let result_bytes = serde_json::to_vec(result).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    let request_ref = write_artifact(&directory, "request.json", &request_bytes)?;
    let fixture_ref = write_artifact(&directory, "fixture.json", &fixture_bytes)?;
    let result_ref = write_artifact(&directory, "result.json", &result_bytes)?;
    let manifest = ResearchManifest {
        format_version: "1.0.0".into(),
        job_id: result.job_id.as_str().into(),
        contract_version: "1.0.0".into(),
        executable_sha256: executable_digest()?,
        request: request_ref,
        fixture: fixture_ref,
        result: result_ref,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    fs::write(directory.join("manifest.json"), manifest_bytes).map_err(|_| ResearchEvidenceError::Io)?;
    Ok(manifest)
}

fn read_artifact(directory: &Path, reference: &SealedArtifact) -> Result<Vec<u8>, ResearchEvidenceError> {
    if reference.relative_path.contains('/') || reference.relative_path.contains('\\') || reference.relative_path.starts_with('.') {
        return Err(ResearchEvidenceError::InvalidManifest);
    }
    let bytes = fs::read(directory.join(&reference.relative_path)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound { ResearchEvidenceError::MissingArtifact } else { ResearchEvidenceError::Io }
    })?;
    if bytes.len() as u64 != reference.size_bytes || digest(&bytes)? != reference.sha256 { return Err(ResearchEvidenceError::DigestMismatch); }
    Ok(bytes)
}

pub fn load_job(root: &Path, job_id: &Uuid) -> Result<SealedResearchJob, ResearchEvidenceError> {
    let directory: PathBuf = root.join("research_jobs").join(job_id.as_str());
    let manifest_bytes = fs::read(directory.join("manifest.json")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound { ResearchEvidenceError::MissingArtifact } else { ResearchEvidenceError::Io }
    })?;
    let manifest: ResearchManifest = serde_json::from_slice(&manifest_bytes).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    if manifest.format_version != "1.0.0" || manifest.contract_version != "1.0.0" || manifest.job_id != job_id.as_str() {
        return Err(ResearchEvidenceError::Incompatible);
    }
    if executable_digest()? != manifest.executable_sha256 { return Err(ResearchEvidenceError::Incompatible); }
    let request: ResearchJobRequest = serde_json::from_slice(&read_artifact(&directory, &manifest.request)?).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    let fixture: ResearchFixture = serde_json::from_slice(&read_artifact(&directory, &manifest.fixture)?).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    let result: ResearchJobResult = serde_json::from_slice(&read_artifact(&directory, &manifest.result)?).map_err(|_| ResearchEvidenceError::InvalidManifest)?;
    Ok(SealedResearchJob { request, fixture, result })
}
