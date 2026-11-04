//! Local per-run evidence writing and integrity verification.

use crate::orchestrator::{
    DecisionCycleRecord, FailedInferenceExchange, OrchestrationError, OrchestrationReport,
    RunExecutionFailure,
};
use factorsbt_contracts::{
    outcomes::{EvidenceArtifactRef, EvidenceKind},
    RunConfiguration, UtcTimestamp, Uuid,
};
use factorsbt_data::Fixture;
use factorsbt_engine::{FinalRunStatus, PortfolioState};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub enum EvidenceError {
    Io(&'static str),
    ExistingRun,
    InvalidRelativePath,
    PathEscapesRunRoot,
    InvalidRunIdentity,
    RunNotSuccessful,
    RunNotFailed,
    FixtureIdentityMismatch,
    FixtureDigestMismatch,
    InvalidInitialPortfolio,
    Serialization(&'static str),
    ArtifactIntegrity(String),
    TimestampUnavailable,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(context) => write!(formatter, "evidence {context} failed"),
            Self::ExistingRun => formatter.write_str("evidence already exists for this run ID"),
            Self::InvalidRelativePath => {
                formatter.write_str("evidence path is not relative and normalized")
            }
            Self::PathEscapesRunRoot => {
                formatter.write_str("evidence path escapes its run directory")
            }
            Self::InvalidRunIdentity => {
                formatter.write_str("run ID is invalid for a local evidence directory")
            }
            Self::RunNotSuccessful => {
                formatter.write_str("successful evidence writer requires a succeeded run")
            }
            Self::RunNotFailed => {
                formatter.write_str("failure evidence writer requires a failed run")
            }
            Self::FixtureIdentityMismatch => {
                formatter.write_str("run report and fixture identity differ")
            }
            Self::FixtureDigestMismatch => {
                formatter.write_str("fixture source bytes do not match the registered digest")
            }
            Self::InvalidInitialPortfolio => {
                formatter.write_str("initial portfolio cannot be valued")
            }
            Self::Serialization(context) => {
                write!(formatter, "evidence {context} serialization failed")
            }
            Self::ArtifactIntegrity(path) => write!(
                formatter,
                "evidence artifact failed integrity verification: {path}"
            ),
            Self::TimestampUnavailable => {
                formatter.write_str("evidence creation timestamp is unavailable")
            }
        }
    }
}

impl Error for EvidenceError {}

#[derive(Debug, Clone)]
pub struct EvidenceBundle {
    pub run_directory: PathBuf,
    pub artifacts: Vec<EvidenceArtifactRef>,
}

/// Write immutable artifacts for a successful run. The caller should publish
/// `succeeded` only after this function and `verify_artifacts` both succeed.
pub fn write_successful_run(
    root: impl AsRef<Path>,
    configuration: &RunConfiguration,
    fixture: &Fixture,
    report: &OrchestrationReport,
) -> Result<EvidenceBundle, EvidenceError> {
    if report.finalization.status != FinalRunStatus::Succeeded {
        return Err(EvidenceError::RunNotSuccessful);
    }
    write_report_artifacts(root, configuration, fixture, report, None, None)
}

/// Persist a failed run's last valid portfolio, trace, and in-flight model
/// exchange. Failures before engine initialization still retain config, source
/// fixtures, the error, and any request/response bytes that were available.
pub fn write_failed_run(
    root: impl AsRef<Path>,
    configuration: &RunConfiguration,
    fixture: &Fixture,
    failure: &RunExecutionFailure,
) -> Result<EvidenceBundle, EvidenceError> {
    if let Some(report) = failure.partial_report.as_deref() {
        if report.finalization.status != FinalRunStatus::Failed {
            return Err(EvidenceError::RunNotFailed);
        }
        write_report_artifacts(
            root,
            configuration,
            fixture,
            report,
            Some(&failure.error),
            failure.failed_inference.as_deref(),
        )
    } else {
        write_uninitialized_failure(root, configuration, fixture, failure)
    }
}

fn write_report_artifacts(
    root: impl AsRef<Path>,
    configuration: &RunConfiguration,
    fixture: &Fixture,
    report: &OrchestrationReport,
    failure: Option<&OrchestrationError>,
    failed_inference: Option<&FailedInferenceExchange>,
) -> Result<EvidenceBundle, EvidenceError> {
    if failure.is_none() && report.finalization.status != FinalRunStatus::Succeeded {
        return Err(EvidenceError::RunNotSuccessful);
    }
    if failure.is_some() && report.finalization.status != FinalRunStatus::Failed {
        return Err(EvidenceError::RunNotFailed);
    }
    if report.fixture_id != fixture.manifest.fixture_id.as_str()
        || report.fixture_version != fixture.manifest.fixture_version.as_str()
        || report.fixture_digest_sha256 != fixture.digest_sha256
        || configuration.data.fixture_id != fixture.manifest.fixture_id
        || configuration.data.fixture_version != fixture.manifest.fixture_version
    {
        return Err(EvidenceError::FixtureIdentityMismatch);
    }
    if sha256(&fixture.dataset_bytes) != fixture.digest_sha256 {
        return Err(EvidenceError::FixtureDigestMismatch);
    }
    let initial_nav = PortfolioState::initialize(configuration, fixture)
        .and_then(|portfolio| portfolio.net_asset_value())
        .map_err(|_| EvidenceError::InvalidInitialPortfolio)?
        .to_string();
    let run_id = report.identity.run_id.as_str();
    if run_id.len() != 36
        || run_id
            .bytes()
            .any(|byte| !(byte.is_ascii_hexdigit() || byte == b'-'))
    {
        return Err(EvidenceError::InvalidRunIdentity);
    }
    let run_directory = create_run_directory(root, run_id)?;

    let created_at = system_timestamp()?;
    let mut artifacts = Vec::new();
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Configuration,
        "evidence/configuration.json",
        json_bytes(configuration, "configuration")?,
        &mut artifacts,
    )?;
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Observation,
        "evidence/fixture/manifest.json",
        fixture.manifest_bytes.to_vec(),
        &mut artifacts,
    )?;
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Observation,
        "evidence/fixture/dataset.json",
        fixture.dataset_bytes.to_vec(),
        &mut artifacts,
    )?;

    let mut order_records = Vec::new();
    let mut admission_records = Vec::new();
    for cycle in &report.cycles {
        write_cycle_artifacts(&run_directory, &created_at, cycle, &mut artifacts)?;
        order_records.push(order_record(cycle));
        if let Some(admission) = &cycle.admission {
            admission_records.push(json!({
                "step_id":cycle.request.step_id.as_str(),
                "adjustments":admission.adjustments.iter().map(|adjustment| json!({
                    "instrument_id":adjustment.instrument_id,
                    "requested_quantity":adjustment.requested_quantity.to_string(),
                    "admitted_quantity":adjustment.admitted_quantity.to_string(),
                    "common_scale":adjustment.common_scale.to_string(),
                    "scaled":adjustment.scaled,
                })).collect::<Vec<_>>(),
            }));
        }
    }
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::DecisionAdmission,
        "evidence/admissions.json",
        &json!(admission_records),
        &mut artifacts,
    )?;
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Order,
        "evidence/orders.json",
        &json!(order_records),
        &mut artifacts,
    )?;
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Fill,
        "evidence/executions.json",
        &json!(report.execution_events),
        &mut artifacts,
    )?;
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Fill,
        "evidence/liquidations.json",
        &json!(report.liquidation_events),
        &mut artifacts,
    )?;
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Settlement,
        "evidence/settlements.json",
        &json!(report.settlement_events),
        &mut artifacts,
    )?;
    let ledger = report
        .execution_events
        .iter()
        .chain(report.liquidation_events.iter())
        .chain(report.settlement_events.iter())
        .flat_map(collect_ledger_postings)
        .collect::<Vec<_>>();
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Ledger,
        "evidence/ledger-postings.json",
        &json!(ledger),
        &mut artifacts,
    )?;
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Risk,
        "evidence/risk-events.json",
        &json!({
            "maintenance":report.risk_events,
            "liquidation":report.liquidation_events,
        }),
        &mut artifacts,
    )?;
    write_json_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Summary,
        "evidence/summary.json",
        &summary_value(report, failure, configuration, &initial_nav),
        &mut artifacts,
    )?;

    if let Some(failure) = failure {
        write_failure_artifacts(
            &run_directory,
            &created_at,
            failure,
            failed_inference,
            true,
            &mut artifacts,
        )?;
    }

    let manifest_bytes = serde_json::to_vec_pretty(&json!({
        "format_version":"1.0.0",
        "run_id":report.identity.run_id.as_str(),
        "scenario_id":report.identity.scenario_id.as_str(),
        "status":status_name(report.finalization.status),
        "failure":failure.map(|error| json!({"stage":error.stage,"reason":error.reason})),
        "fixture":{
            "fixture_id":report.fixture_id,
            "fixture_version":report.fixture_version,
            "dataset_sha256":report.fixture_digest_sha256,
        },
        "artifacts":artifacts,
    }))
    .map_err(|_| EvidenceError::Serialization("run manifest"))?;
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::RunManifest,
        "evidence/run-manifest.json",
        manifest_bytes,
        &mut artifacts,
    )?;

    verify_artifacts(&run_directory, &artifacts)?;
    Ok(EvidenceBundle {
        run_directory,
        artifacts,
    })
}

fn write_uninitialized_failure(
    root: impl AsRef<Path>,
    configuration: &RunConfiguration,
    fixture: &Fixture,
    failure: &RunExecutionFailure,
) -> Result<EvidenceBundle, EvidenceError> {
    if configuration.data.fixture_id != fixture.manifest.fixture_id
        || configuration.data.fixture_version != fixture.manifest.fixture_version
    {
        return Err(EvidenceError::FixtureIdentityMismatch);
    }
    if sha256(&fixture.dataset_bytes) != fixture.digest_sha256 {
        return Err(EvidenceError::FixtureDigestMismatch);
    }
    let run_directory = create_run_directory(root, failure.identity.run_id.as_str())?;
    let created_at = system_timestamp()?;
    let mut artifacts = Vec::new();
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Configuration,
        "evidence/configuration.json",
        json_bytes(configuration, "configuration")?,
        &mut artifacts,
    )?;
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Observation,
        "evidence/fixture/manifest.json",
        fixture.manifest_bytes.to_vec(),
        &mut artifacts,
    )?;
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::Observation,
        "evidence/fixture/dataset.json",
        fixture.dataset_bytes.to_vec(),
        &mut artifacts,
    )?;
    write_failure_artifacts(
        &run_directory,
        &created_at,
        &failure.error,
        failure.failed_inference.as_deref(),
        false,
        &mut artifacts,
    )?;
    let manifest_bytes = serde_json::to_vec_pretty(&json!({
        "format_version":"1.0.0",
        "run_id":failure.identity.run_id.as_str(),
        "scenario_id":failure.identity.scenario_id.as_str(),
        "status":"failed",
        "failure":{"stage":failure.error.stage,"reason":failure.error.reason},
        "fixture":{
            "fixture_id":fixture.manifest.fixture_id,
            "fixture_version":fixture.manifest.fixture_version,
            "dataset_sha256":fixture.digest_sha256,
        },
        "artifacts":artifacts,
    }))
    .map_err(|_| EvidenceError::Serialization("run manifest"))?;
    write_artifact(
        &run_directory,
        &created_at,
        EvidenceKind::RunManifest,
        "evidence/run-manifest.json",
        manifest_bytes,
        &mut artifacts,
    )?;
    verify_artifacts(&run_directory, &artifacts)?;
    Ok(EvidenceBundle {
        run_directory,
        artifacts,
    })
}

fn write_failure_artifacts(
    run_directory: &Path,
    created_at: &UtcTimestamp,
    failure: &OrchestrationError,
    failed_inference: Option<&FailedInferenceExchange>,
    last_valid_portfolio_present: bool,
    artifacts: &mut Vec<EvidenceArtifactRef>,
) -> Result<(), EvidenceError> {
    if let Some(failed_inference) = failed_inference {
        write_artifact(
            run_directory,
            created_at,
            EvidenceKind::ModelRequest,
            "evidence/model-requests/failed-inference.json",
            failed_inference.request_body.clone(),
            artifacts,
        )?;
        if let Some(response_body) = &failed_inference.response_body {
            write_artifact(
                run_directory,
                created_at,
                EvidenceKind::ModelResponse,
                "evidence/model-responses/failed-inference.json",
                response_body.clone(),
                artifacts,
            )?;
        }
    }
    write_json_artifact(
        run_directory,
        created_at,
        EvidenceKind::Risk,
        "evidence/failure.json",
        &json!({
            "stage":failure.stage,
            "reason":failure.reason,
            "last_valid_portfolio_present":last_valid_portfolio_present,
            "failed_inference_id":failed_inference.map(|exchange| exchange.request.inference_id.as_str()),
        }),
        artifacts,
    )
}

fn create_run_directory(root: impl AsRef<Path>, run_id: &str) -> Result<PathBuf, EvidenceError> {
    if run_id.len() != 36
        || run_id
            .bytes()
            .any(|byte| !(byte.is_ascii_hexdigit() || byte == b'-'))
    {
        return Err(EvidenceError::InvalidRunIdentity);
    }
    let root = root.as_ref();
    fs::create_dir_all(root).map_err(|_| EvidenceError::Io("root creation"))?;
    let root = fs::canonicalize(root).map_err(|_| EvidenceError::Io("root resolution"))?;
    let runs = root.join("runs");
    fs::create_dir_all(&runs).map_err(|_| EvidenceError::Io("runs directory creation"))?;
    let runs =
        fs::canonicalize(&runs).map_err(|_| EvidenceError::Io("runs directory resolution"))?;
    if !runs.starts_with(&root) {
        return Err(EvidenceError::PathEscapesRunRoot);
    }
    let run_directory = runs.join(run_id);
    match fs::create_dir(&run_directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(EvidenceError::ExistingRun)
        }
        Err(_) => return Err(EvidenceError::Io("run directory creation")),
    }
    let evidence_directory = run_directory.join("evidence");
    fs::create_dir(&evidence_directory)
        .map_err(|_| EvidenceError::Io("evidence directory creation"))?;
    let evidence_directory = fs::canonicalize(&evidence_directory)
        .map_err(|_| EvidenceError::Io("evidence directory resolution"))?;
    if !evidence_directory.starts_with(&run_directory) {
        return Err(EvidenceError::PathEscapesRunRoot);
    }
    Ok(run_directory)
}

/// Recheck every local reference's normalized path, exact byte size, and SHA-256.
pub fn verify_artifacts(
    run_directory: impl AsRef<Path>,
    artifacts: &[EvidenceArtifactRef],
) -> Result<(), EvidenceError> {
    let root = fs::canonicalize(run_directory.as_ref())
        .map_err(|_| EvidenceError::Io("run root resolution"))?;
    for artifact in artifacts {
        let relative = safe_relative_path(&artifact.relative_path)?;
        let path = root.join(relative);
        if !path.starts_with(&root) {
            return Err(EvidenceError::PathEscapesRunRoot);
        }
        let canonical = fs::canonicalize(&path)
            .map_err(|_| EvidenceError::ArtifactIntegrity(artifact.relative_path.clone()))?;
        if !canonical.starts_with(root.join("evidence")) || !canonical.is_file() {
            return Err(EvidenceError::PathEscapesRunRoot);
        }
        let bytes = fs::read(&canonical)
            .map_err(|_| EvidenceError::ArtifactIntegrity(artifact.relative_path.clone()))?;
        if u64::try_from(bytes.len()).ok() != Some(artifact.size_bytes)
            || sha256(&bytes) != artifact.sha256
        {
            return Err(EvidenceError::ArtifactIntegrity(
                artifact.relative_path.clone(),
            ));
        }
    }
    Ok(())
}

fn write_cycle_artifacts(
    run_directory: &Path,
    created_at: &UtcTimestamp,
    cycle: &DecisionCycleRecord,
    artifacts: &mut Vec<EvidenceArtifactRef>,
) -> Result<(), EvidenceError> {
    let step = cycle.request.step_id.as_str();
    write_json_artifact(
        run_directory,
        created_at,
        EvidenceKind::Observation,
        &format!("evidence/observations/{step}.json"),
        &json!(cycle.snapshot.observations),
        artifacts,
    )?;
    write_json_artifact(
        run_directory,
        created_at,
        EvidenceKind::Snapshot,
        &format!("evidence/snapshots/{step}.json"),
        &cycle.snapshot,
        artifacts,
    )?;
    write_artifact(
        run_directory,
        created_at,
        EvidenceKind::ModelRequest,
        &format!("evidence/model-requests/{step}.json"),
        cycle.request_body.clone(),
        artifacts,
    )?;
    write_artifact(
        run_directory,
        created_at,
        EvidenceKind::ModelResponse,
        &format!("evidence/model-responses/{step}.json"),
        cycle.response_body.clone(),
        artifacts,
    )?;
    write_json_artifact(
        run_directory,
        created_at,
        EvidenceKind::DecisionAdmission,
        &format!("evidence/decisions/{step}.json"),
        &decision_record(cycle),
        artifacts,
    )?;
    Ok(())
}

fn decision_record(cycle: &DecisionCycleRecord) -> Value {
    json!({
        "run_id":cycle.request.run_id.as_str(),
        "scenario_id":cycle.request.scenario_id.as_str(),
        "step_id":cycle.request.step_id.as_str(),
        "snapshot_id":cycle.request.snapshot_id.as_str(),
        "inference_id":cycle.request.inference_id.as_str(),
        "proposal":{
            "target_positions":cycle.proposal.target_positions.iter().map(|target| json!({
                "instrument_id":target.instrument_id.as_str(),
                "target_quantity":target.target_quantity.to_string(),
            })).collect::<Vec<_>>(),
            "order_intents":cycle.proposal.order_intents.iter().map(|intent| json!({
                "instrument_id":intent.instrument_id.as_str(),
                "signed_quantity":intent.signed_quantity.to_string(),
            })).collect::<Vec<_>>(),
        },
        "queued_order_ids":cycle.queued_order_ids,
    })
}

fn order_record(cycle: &DecisionCycleRecord) -> Value {
    json!({
        "step_id":cycle.request.step_id.as_str(),
        "orders":cycle.proposal.order_intents.iter().map(|intent| json!({
            "instrument_id":intent.instrument_id.as_str(),
            "signed_quantity":intent.signed_quantity.to_string(),
            "order_type":"market",
        })).collect::<Vec<_>>(),
        "target_positions":cycle.proposal.target_positions.iter().map(|target| json!({
            "instrument_id":target.instrument_id.as_str(),
            "target_quantity":target.target_quantity.to_string(),
        })).collect::<Vec<_>>(),
        "queued_order_ids":cycle.queued_order_ids,
    })
}

fn summary_value(
    report: &OrchestrationReport,
    failure: Option<&OrchestrationError>,
    configuration: &RunConfiguration,
    initial_nav: &str,
) -> Value {
    json!({
        "run_id":report.identity.run_id.as_str(),
        "scenario_id":report.identity.scenario_id.as_str(),
        "status":status_name(report.finalization.status),
        "failure":failure.map(|error| json!({"stage":error.stage,"reason":error.reason})),
        "fixture_id":report.fixture_id,
        "fixture_version":report.fixture_version,
        "fixture_digest_sha256":report.fixture_digest_sha256,
        "simulation_start":configuration.simulation.start_at.as_str(),
        "simulation_end":configuration.simulation.end_at.as_str(),
        "initial_nav":initial_nav,
        "decision_steps":report.cycles.len(),
        "execution_events":report.execution_events.len(),
        "settlement_events":report.settlement_events.len(),
        "risk_events":report.risk_events,
        "liquidation_events":report.liquidation_events,
        "final_portfolio":{
            "cash":report.finalization.cash.to_string(),
            "nav":report.finalization.nav.to_string(),
            "snapshot":&report.final_portfolio,
            "open_positions":report.finalization.open_positions.iter().map(|position| json!({
                "instrument_id":position.instrument_id,
                "quantity":position.quantity.to_string(),
                "mark":position.mark.to_string(),
                "market_value":position.market_value.to_string(),
            })).collect::<Vec<_>>(),
            "canceled_order_ids":report.finalization.canceled_order_ids,
        },
    })
}

fn status_name(status: FinalRunStatus) -> &'static str {
    match status {
        FinalRunStatus::Succeeded => "succeeded",
        FinalRunStatus::Failed => "failed",
    }
}

fn collect_ledger_postings(value: &Value) -> Vec<Value> {
    let mut postings = Vec::new();
    collect_postings_recursive(value, &mut postings);
    postings
}

fn collect_postings_recursive(value: &Value, postings: &mut Vec<Value>) {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if key.ends_with("_posting") && nested.is_object() {
                    postings.push(nested.clone());
                }
                collect_postings_recursive(nested, postings);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_postings_recursive(nested, postings);
            }
        }
        _ => {}
    }
}

fn write_json_artifact<T: serde::Serialize>(
    run_directory: &Path,
    created_at: &UtcTimestamp,
    kind: EvidenceKind,
    relative_path: &str,
    value: &T,
    artifacts: &mut Vec<EvidenceArtifactRef>,
) -> Result<(), EvidenceError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| EvidenceError::Serialization("JSON artifact"))?;
    write_artifact(
        run_directory,
        created_at,
        kind,
        relative_path,
        bytes,
        artifacts,
    )
}

fn json_bytes<T: serde::Serialize>(
    value: &T,
    context: &'static str,
) -> Result<Vec<u8>, EvidenceError> {
    serde_json::to_vec_pretty(value).map_err(|_| EvidenceError::Serialization(context))
}

fn write_artifact(
    run_directory: &Path,
    created_at: &UtcTimestamp,
    kind: EvidenceKind,
    relative_path: &str,
    bytes: Vec<u8>,
    artifacts: &mut Vec<EvidenceArtifactRef>,
) -> Result<(), EvidenceError> {
    let relative = safe_relative_path(relative_path)?;
    let target = run_directory.join(relative);
    if !target.starts_with(run_directory) {
        return Err(EvidenceError::PathEscapesRunRoot);
    }
    let parent = target.parent().ok_or(EvidenceError::InvalidRelativePath)?;
    fs::create_dir_all(parent).map_err(|_| EvidenceError::Io("artifact directory creation"))?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| EvidenceError::Io("artifact directory resolution"))?;
    let evidence_root = fs::canonicalize(run_directory.join("evidence"))
        .map_err(|_| EvidenceError::Io("evidence root resolution"))?;
    if !canonical_parent.starts_with(&evidence_root) {
        return Err(EvidenceError::PathEscapesRunRoot);
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&target)
        .map_err(|_| EvidenceError::Io("artifact creation"))?;
    file.write_all(&bytes)
        .map_err(|_| EvidenceError::Io("artifact write"))?;
    file.sync_all()
        .map_err(|_| EvidenceError::Io("artifact sync"))?;
    drop(file);
    let stored = fs::read(&target).map_err(|_| EvidenceError::Io("artifact verification read"))?;
    let digest = sha256(&stored);
    let size_bytes =
        u64::try_from(stored.len()).map_err(|_| EvidenceError::Io("artifact size conversion"))?;
    if sha256(&stored) != digest {
        return Err(EvidenceError::ArtifactIntegrity(relative_path.to_owned()));
    }
    artifacts.push(EvidenceArtifactRef {
        artifact_id: artifact_id(run_directory, relative_path),
        kind,
        relative_path: relative_path.to_owned(),
        content_type: "application/json".to_owned(),
        sha256: digest,
        size_bytes,
        created_at: created_at.clone(),
    });
    Ok(())
}

fn safe_relative_path(value: &str) -> Result<&Path, EvidenceError> {
    if !value.starts_with("evidence/") || value.contains('\\') || value.contains(':') {
        return Err(EvidenceError::InvalidRelativePath);
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(EvidenceError::InvalidRelativePath);
    }
    Ok(path)
}

fn artifact_id(run_directory: &Path, relative_path: &str) -> Uuid {
    let run_id = run_directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("run");
    let digest = Sha256::digest(format!("{run_id}:{relative_path}").as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Uuid::new(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
    .expect("SHA-256-derived artifact identity is a valid UUID")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn system_timestamp() -> Result<UtcTimestamp, EvidenceError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| EvidenceError::TimestampUnavailable)?
        .as_secs();
    let days = i64::try_from(seconds / 86_400).map_err(|_| EvidenceError::TimestampUnavailable)?;
    let seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3_600;
    let minute = seconds % 3_600 / 60;
    let second = seconds % 60;
    UtcTimestamp::new(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
    .map_err(|_| EvidenceError::TimestampUnavailable)
}

fn civil_from_days(day_number: i64) -> (i64, i64, i64) {
    let shifted = day_number + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{RunExecutionFailure, RunIdentity};
    use factorsbt_contracts::Identifier;
    use factorsbt_data::FixtureRegistry;
    use factorsbt_engine::{OpenPositionSummary, RunFinalization};
    use rust_decimal::Decimal;
    use std::path::Path;

    fn test_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("factorsbt-evidence-{}-{nonce}", std::process::id()))
    }

    fn input() -> (RunConfiguration, Fixture, OrchestrationReport) {
        let config: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        let fixture_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let fixture = FixtureRegistry::load(fixture_root)
            .unwrap()
            .get_by("mvp-synthetic", "v1")
            .unwrap()
            .as_ref()
            .clone();
        let report = OrchestrationReport {
            identity: crate::orchestrator::RunIdentity {
                run_id: Uuid::new("f8a3cad4-4da8-4a20-82bc-258cbf03a530").unwrap(),
                scenario_id: Identifier::new("baseline").unwrap(),
            },
            fixture_id: "mvp-synthetic".to_owned(),
            fixture_version: "v1".to_owned(),
            fixture_digest_sha256: fixture.digest_sha256.clone(),
            cycles: Vec::new(),
            execution_events: Vec::new(),
            liquidation_events: Vec::new(),
            settlement_events: Vec::new(),
            risk_events: Vec::new(),
            final_portfolio: factorsbt_contracts::PortfolioSnapshot {
                cash: factorsbt_contracts::DecimalString::new("100000").unwrap(),
                nav: factorsbt_contracts::DecimalString::new("100000").unwrap(),
                positions: vec![factorsbt_contracts::temporal::PositionSnapshot {
                    instrument_id: factorsbt_contracts::InstrumentId::new("SPOT_A").unwrap(),
                    quantity: factorsbt_contracts::DecimalString::new("1").unwrap(),
                    mark: factorsbt_contracts::DecimalString::new("100").unwrap(),
                    market_value: factorsbt_contracts::DecimalString::new("100").unwrap(),
                }],
                reserved_cash: factorsbt_contracts::DecimalString::new("0").unwrap(),
                reserved_underlying: Vec::new(),
                initial_margin_requirement: factorsbt_contracts::DecimalString::new("0").unwrap(),
                maintenance_margin_requirement: factorsbt_contracts::DecimalString::new("0")
                    .unwrap(),
                free_buying_power: factorsbt_contracts::DecimalString::new("100000").unwrap(),
            },
            finalization: RunFinalization {
                status: FinalRunStatus::Succeeded,
                failure_reason: None,
                cash: Decimal::from(100_000),
                nav: Decimal::from(100_000),
                canceled_order_ids: Vec::new(),
                open_positions: vec![OpenPositionSummary {
                    instrument_id: "SPOT_A".to_owned(),
                    quantity: Decimal::ONE,
                    mark: Decimal::from(100),
                    market_value: Decimal::from(100),
                }],
            },
        };
        (config, fixture, report)
    }

    #[test]
    fn persists_exact_fixture_bytes_and_verifies_all_hashes_and_sizes() {
        let root = test_root();
        let (configuration, fixture, report) = input();
        let bundle = write_successful_run(&root, &configuration, &fixture, &report).unwrap();
        assert!(bundle
            .artifacts
            .iter()
            .any(|artifact| { artifact.relative_path == "evidence/run-manifest.json" }));
        let stored_fixture =
            fs::read(bundle.run_directory.join("evidence/fixture/dataset.json")).unwrap();
        assert_eq!(stored_fixture.as_slice(), fixture.dataset_bytes.as_ref());
        assert_eq!(sha256(&stored_fixture), fixture.digest_sha256);
        verify_artifacts(&bundle.run_directory, &bundle.artifacts).unwrap();
        let result = crate::results::build_run_result(
            &configuration,
            &fixture,
            &report,
            UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap(),
            &bundle,
        )
        .unwrap();
        assert_eq!(
            result.status,
            factorsbt_contracts::outcomes::SuccessfulStatus::Succeeded
        );
        assert_eq!(result.counts.decision_steps, 0);
        assert_eq!(result.total_fees.as_str(), "0");
        crate::results::verify_result_evidence(&bundle.run_directory, &result).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_detects_artifact_mutation() {
        assert!(matches!(
            safe_relative_path("evidence/../outside.json"),
            Err(EvidenceError::InvalidRelativePath)
        ));
        assert!(matches!(
            safe_relative_path("evidence\\outside.json"),
            Err(EvidenceError::InvalidRelativePath)
        ));

        let root = test_root();
        let (configuration, fixture, report) = input();
        let bundle = write_successful_run(&root, &configuration, &fixture, &report).unwrap();
        fs::write(
            bundle.run_directory.join("evidence/configuration.json"),
            b"tampered",
        )
        .unwrap();
        assert!(matches!(
            verify_artifacts(&bundle.run_directory, &bundle.artifacts),
            Err(EvidenceError::ArtifactIntegrity(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_an_existing_run_evidence_directory() {
        let root = test_root();
        let (configuration, fixture, report) = input();
        write_successful_run(&root, &configuration, &fixture, &report).unwrap();
        assert!(matches!(
            write_successful_run(&root, &configuration, &fixture, &report),
            Err(EvidenceError::ExistingRun)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persists_partial_failure_and_in_flight_model_exchange() {
        let root = test_root();
        let (configuration, fixture, mut report) = input();
        report.finalization.status = FinalRunStatus::Failed;
        report.finalization.failure_reason = Some("model output rejected".to_owned());
        let request: factorsbt_contracts::ModelRequest = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
        ))
        .unwrap();
        let failure = RunExecutionFailure {
            identity: report.identity.clone(),
            error: OrchestrationError {
                stage: "model response validation",
                reason: "mode mismatch".to_owned(),
            },
            partial_report: Some(Box::new(report)),
            failed_inference: Some(Box::new(FailedInferenceExchange {
                request,
                request_body: br#"{"request":"exact"}"#.to_vec(),
                response_body: Some(br#"{"response":"raw"}"#.to_vec()),
            })),
        };
        let bundle = write_failed_run(&root, &configuration, &fixture, &failure).unwrap();
        let manifest_bytes =
            fs::read(bundle.run_directory.join("evidence/run-manifest.json")).unwrap();
        let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(manifest["status"], "failed");
        let manifest_paths = manifest["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|artifact| artifact["relative_path"].as_str())
            .collect::<Vec<_>>();
        assert!(manifest_paths.contains(&"evidence/failure.json"));
        assert!(manifest_paths.contains(&"evidence/model-requests/failed-inference.json"));
        assert!(manifest_paths.contains(&"evidence/model-responses/failed-inference.json"));
        assert!(bundle
            .artifacts
            .iter()
            .any(|artifact| artifact.relative_path == "evidence/run-manifest.json"));
        let summary: Value = serde_json::from_slice(
            &fs::read(bundle.run_directory.join("evidence/summary.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(summary["status"], "failed");
        let failure_evidence: Value = serde_json::from_slice(
            &fs::read(bundle.run_directory.join("evidence/failure.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(failure_evidence["last_valid_portfolio_present"], true);
        assert_eq!(
            fs::read(
                bundle
                    .run_directory
                    .join("evidence/model-responses/failed-inference.json")
            )
            .unwrap(),
            br#"{"response":"raw"}"#
        );
        verify_artifacts(&bundle.run_directory, &bundle.artifacts).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persists_configuration_and_failure_when_engine_never_initializes() {
        let root = test_root();
        let (configuration, fixture, _) = input();
        let failure = RunExecutionFailure {
            identity: RunIdentity {
                run_id: Uuid::new("a4ab8716-3d8c-42d7-857c-83e94df28ff6").unwrap(),
                scenario_id: Identifier::new("baseline").unwrap(),
            },
            error: OrchestrationError {
                stage: "fixture coverage",
                reason: "required price is missing".to_owned(),
            },
            partial_report: None,
            failed_inference: None,
        };
        let bundle = write_failed_run(&root, &configuration, &fixture, &failure).unwrap();
        let evidence: Value = serde_json::from_slice(
            &fs::read(bundle.run_directory.join("evidence/failure.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(evidence["last_valid_portfolio_present"], false);
        let manifest: Value = serde_json::from_slice(
            &fs::read(bundle.run_directory.join("evidence/run-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["status"], "failed");
        assert_eq!(manifest["failure"]["stage"], "fixture coverage");
        assert!(manifest["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["relative_path"] == "evidence/failure.json"));
        verify_artifacts(&bundle.run_directory, &bundle.artifacts).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
