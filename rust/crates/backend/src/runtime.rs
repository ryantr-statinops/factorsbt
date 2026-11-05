//! Local service runtime: fixture registry, bounded workers, and evidence.

use crate::{
    backend_config::BackendConfig,
    dispatcher::RunDispatcher,
    evidence::{write_failed_run, write_successful_run},
    model_client::ModelHttpClient,
    orchestrator::{run_simulation_with_progress, system_utc_timestamp, RunIdentity},
    results::build_run_result,
    run_store::{JobId, RunStore},
};
use factorsbt_contracts::{
    outcomes::{EvidenceKind, RunFailure},
    Identifier, UtcTimestamp, Uuid,
};
use factorsbt_data::FixtureRegistry;
use std::{error::Error, fmt, fs, io, path::PathBuf, sync::Arc, time::Duration};

pub struct BackendRuntime {
    store: RunStore,
    fixtures: Arc<FixtureRegistry>,
    dispatcher: Option<RunDispatcher>,
}

#[derive(Debug)]
pub enum BackendRuntimeError {
    FixtureRegistry(factorsbt_data::FixtureLoadError),
    EvidenceDirectory(io::Error),
    ModelClient(crate::model_client::ModelServiceError),
}

impl fmt::Display for BackendRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixtureRegistry(error) => {
                write!(formatter, "fixture registry startup failed: {error}")
            }
            Self::EvidenceDirectory(_) => formatter.write_str("evidence directory startup failed"),
            Self::ModelClient(error) => {
                write!(formatter, "model HTTP client startup failed: {error}")
            }
        }
    }
}

impl Error for BackendRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FixtureRegistry(error) => Some(error),
            Self::EvidenceDirectory(error) => Some(error),
            Self::ModelClient(error) => Some(error),
        }
    }
}

impl BackendRuntime {
    /// Load immutable local resources once and start the configured worker pool.
    pub fn start(config: &BackendConfig) -> Result<Self, BackendRuntimeError> {
        let fixtures = Arc::new(
            FixtureRegistry::load(&config.fixture_directory)
                .map_err(BackendRuntimeError::FixtureRegistry)?,
        );
        fs::create_dir_all(&config.evidence_directory)
            .map_err(BackendRuntimeError::EvidenceDirectory)?;
        let model_client = ModelHttpClient::new().map_err(BackendRuntimeError::ModelClient)?;
        let store = RunStore::new(config.queue_limits);
        let worker_store = store.clone();
        let worker_fixtures = Arc::clone(&fixtures);
        let worker_client = model_client.clone();
        let worker_evidence_root = config.evidence_directory.clone();
        let panic_store = store.clone();

        let dispatcher = RunDispatcher::start_with_failure_handler(
            store.clone(),
            move |job_id| {
                let store = worker_store.clone();
                let fixtures = Arc::clone(&worker_fixtures);
                let client = worker_client.clone();
                let evidence_root = worker_evidence_root.clone();
                async move {
                    match job_id {
                        JobId::Run(run_id) => {
                            execute_run(store, fixtures, client, evidence_root, run_id).await
                        }
                        JobId::Replay(replay_id) => {
                            execute_replay(store, client, evidence_root, replay_id).await
                        }
                        JobId::Research(job_id) => {
                            execute_research(store, evidence_root, job_id).await
                        }
                    }
                }
            },
            move |job_id| match job_id {
                JobId::Run(run_id) => mark_failed(
                    &panic_store,
                    &run_id,
                    "RUN_WORKER_PANICKED",
                    "The run worker stopped unexpectedly.",
                    None,
                    None,
                    None,
                ),
                JobId::Replay(replay_id) => {
                    mark_replay_worker_panicked(&panic_store, &replay_id);
                }
                JobId::Research(job_id) => {
                    let _ = panic_store.mark_research_terminal(
                        &job_id,
                        system_utc_timestamp().unwrap_or_else(|_| {
                            UtcTimestamp::new("1970-01-01T00:00:00Z").expect("static timestamp")
                        }),
                        None,
                        Some(crate::research::jobs::ResearchJobFailure {
                            code: "RESEARCH_WORKER_PANICKED".into(),
                            message: "The research job worker stopped unexpectedly.".into(),
                        }),
                    );
                }
            },
        );
        Ok(Self {
            store,
            fixtures,
            dispatcher: Some(dispatcher),
        })
    }

    pub fn store(&self) -> RunStore {
        self.store.clone()
    }

    pub fn fixtures(&self) -> Arc<FixtureRegistry> {
        Arc::clone(&self.fixtures)
    }

    pub async fn shutdown(mut self) {
        self.store.stop_accepting();
        if let Some(dispatcher) = self.dispatcher.take() {
            dispatcher.shutdown().await;
        }
    }
}

async fn execute_research(store: RunStore, evidence_root: PathBuf, job_id: Uuid) {
    let Some(stored) = store.get_research_job(&job_id) else {
        return;
    };
    let started = timestamp_or(&stored.status.submitted_at);
    if store.mark_research_running(&job_id, started).is_err() {
        return;
    }
    let finished =
        system_utc_timestamp().unwrap_or_else(|_| timestamp_or(&stored.status.submitted_at));
    let executed = if stored.status.kind == crate::research::jobs::ResearchJobKind::Replay {
        let source_id = stored.request.source_job_id.as_ref();
        match source_id.and_then(|id| crate::research::evidence::load_job(&evidence_root, id).ok())
        {
            Some(source)
                if source.request.kind != crate::research::jobs::ResearchJobKind::Replay =>
            {
                match crate::research::jobs::execute_with_fixture(&source.request, &source.fixture)
                {
                    Ok(report) => {
                        let matched = report == source.result.report;
                        let first_mismatch = if matched {
                            serde_json::Value::Null
                        } else {
                            serde_json::json!("report")
                        };
                        let replay_report = serde_json::json!({
                            "source_job_id": source.result.job_id,
                            "source_kind": source.request.kind,
                            "matched": matched,
                            "first_mismatch": first_mismatch,
                            "source_report": source.result.report,
                            "replayed_report": report,
                        });
                        Ok((source.fixture, replay_report, !matched))
                    }
                    Err(error) => Err(error.to_string()),
                }
            }
            _ => Err("research replay source is absent, invalid, or itself a replay".to_owned()),
        }
    } else {
        match crate::research::fixtures::resolve_fixture(
            stored.request.fixture_id.as_deref().unwrap_or_default(),
            stored
                .request
                .fixture_version
                .as_deref()
                .unwrap_or_default(),
        ) {
            Ok(fixture) => {
                match crate::research::jobs::execute_with_fixture(&stored.request, &fixture) {
                    Ok(report) => Ok((fixture, report, false)),
                    Err(error) => Err(error.to_string()),
                }
            }
            Err(error) => Err(error.to_string()),
        }
    };
    match executed {
        Ok((fixture, report, diverged)) => {
            let result = crate::research::jobs::ResearchJobResult {
                contract_version: "1.0.0".into(),
                job_id: job_id.clone(),
                kind: stored.status.kind,
                report,
            };
            let persisted = crate::research::evidence::write_job(
                &evidence_root,
                &stored.request,
                &fixture,
                &result,
            );
            let failure = if let Err(error) = persisted {
                Some(crate::research::jobs::ResearchJobFailure {
                    code: "RESEARCH_EVIDENCE_FAILED".into(),
                    message: error.to_string(),
                })
            } else if diverged {
                Some(crate::research::jobs::ResearchJobFailure {
                    code: "RESEARCH_REPLAY_DIVERGED".into(),
                    message: "recorded research transitions diverged".into(),
                })
            } else {
                None
            };
            let _ = store.mark_research_terminal(&job_id, finished, Some(result), failure);
        }
        Err(message) => {
            let _ = store.mark_research_terminal(
                &job_id,
                finished,
                None,
                Some(crate::research::jobs::ResearchJobFailure {
                    code: "RESEARCH_EXECUTION_FAILED".into(),
                    message,
                }),
            );
        }
    }
}

async fn execute_replay(
    store: RunStore,
    _http_client: ModelHttpClient,
    evidence_root: PathBuf,
    replay_id: Uuid,
) {
    let Some(stored) = store.get_replay(&replay_id) else {
        return;
    };
    let started = timestamp_or(&stored.status.submitted_at);
    if store.mark_replay_running(&replay_id, started).is_err() {
        return;
    }
    let loaded = (|| -> Result<_, String> {
        let source =
            crate::replay_source::ReplaySource::load(&evidence_root, &stored.status.source_run_id)
                .map_err(|error| error.to_string())?;
        source
            .verify_replay_eligibility(&evidence_root)
            .map_err(|error| error.to_string())?;
        let responses = source
            .recorded_responses(&evidence_root)
            .map_err(|error| error.to_string())?;
        let summary = match source.artifact(&evidence_root, "evidence/summary.json") {
            Ok(bytes) => Some(
                serde_json::from_slice::<serde_json::Value>(&bytes)
                    .map_err(|_| "source summary is invalid".to_owned())?,
            ),
            Err(crate::replay_source::ReplaySourceError::MissingArtifact(_))
                if source.index.status == "failed" =>
            {
                None
            }
            Err(error) => return Err(error.to_string()),
        };
        let terminal_failure = if source.index.status == "failed" {
            Some(
                serde_json::from_slice::<serde_json::Value>(
                    &source
                        .artifact(&evidence_root, "evidence/failure.json")
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|_| "source failure artifact is invalid".to_owned())?,
            )
        } else {
            None
        };
        let provider = match terminal_failure
            .as_ref()
            .and_then(|failure| failure["reason"].as_str())
        {
            Some(reason) => {
                ModelHttpClient::from_recorded_responses_with_failure(responses, reason.to_owned())
            }
            None => ModelHttpClient::from_recorded_responses(responses),
        }
        .map_err(|error| error.to_string())?;
        Ok((source, summary, terminal_failure, provider))
    })();
    let (source, summary, terminal_failure, provider) = match loaded {
        Ok(value) => value,
        Err(message) => {
            let _ = store.mark_replay_terminal(
                &replay_id,
                timestamp_or(&stored.status.submitted_at),
                None,
                Some(crate::replay_types::ReplayFailure {
                    code: "REPLAY_SOURCE_INVALID".to_owned(),
                    message,
                }),
            );
            return;
        }
    };
    let identity = RunIdentity {
        run_id: replay_id.clone(),
        scenario_id: Identifier::new("baseline").expect("static ID"),
    };
    let replayed = run_simulation_with_progress(
        &provider,
        &source.configuration.model.endpoint,
        Duration::from_millis(source.configuration.model.timeout_ms),
        &source.configuration,
        &source.fixture,
        identity,
        |_| {},
    )
    .await;
    match replayed {
        Ok(report) => {
            let replay_orders = report.cycles.iter().map(|cycle| serde_json::json!({
                "orders": cycle.proposal.order_intents.iter().map(|item| serde_json::json!({
                    "instrument_id": item.instrument_id.as_str(), "signed_quantity": item.signed_quantity.to_string(), "order_type": "market"
                })).collect::<Vec<_>>(),
                "target_positions": cycle.proposal.target_positions.iter().map(|item| serde_json::json!({
                    "instrument_id": item.instrument_id.as_str(), "target_quantity": item.target_quantity.to_string()
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>();
            let replay_state = serde_json::json!({
                "cash": report.finalization.cash.to_string(),
                "nav": report.finalization.nav.to_string(),
                "positions": report.final_portfolio.positions,
            "admissions": report.cycles.iter().filter_map(|cycle| cycle.admission.as_ref().map(|admission| serde_json::json!({
                "adjustments": admission.adjustments.iter().map(|adjustment| serde_json::json!({
                    "instrument_id": adjustment.instrument_id,
                    "requested_quantity": adjustment.requested_quantity.to_string(),
                    "admitted_quantity": adjustment.admitted_quantity.to_string(),
                    "common_scale": adjustment.common_scale.to_string(),
                    "scaled": adjustment.scaled,
                })).collect::<Vec<_>>(),
            }))).collect::<Vec<_>>(),
                "orders": replay_orders,
                "executions": report.execution_events,
                "settlements": report.settlement_events,
            "ledger": report.execution_events.iter()
                .chain(report.liquidation_events.iter())
                .chain(report.settlement_events.iter())
                .flat_map(collect_ledger_postings).collect::<Vec<_>>(),
            });
            let Some(summary) = summary.as_ref() else {
                let _ = store.mark_replay_terminal(
                    &replay_id,
                    timestamp_or(&stored.status.submitted_at),
                    None,
                    Some(crate::replay_types::ReplayFailure {
                        code: "REPLAY_SOURCE_INVALID".to_owned(),
                        message: "successful source is missing its summary".to_owned(),
                    }),
                );
                return;
            };
            let source_state = serde_json::json!({
                "cash": summary["final_portfolio"]["cash"],
                "nav": summary["final_portfolio"]["nav"],
                "positions": summary["final_portfolio"]["snapshot"]["positions"],
                "orders": source.artifact(&evidence_root, "evidence/orders.json").ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .and_then(|value| value.as_array().map(|rows| rows.iter().map(|row| serde_json::json!({
                        "orders": row["orders"], "target_positions": row["target_positions"]
                    })).collect::<Vec<_>>())).unwrap_or_default(),
                "executions": source.artifact(&evidence_root, "evidence/executions.json").ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()).unwrap_or_default(),
                "settlements": source.artifact(&evidence_root, "evidence/settlements.json").ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()).unwrap_or_default(),
                "admissions": source.artifact(&evidence_root, "evidence/admissions.json").ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()).unwrap_or_default(),
                "ledger": source.artifact(&evidence_root, "evidence/ledger-postings.json").ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()).unwrap_or_default(),
            });
            let first_mismatch = replay_mismatch(&replay_state, &source_state);
            let matched = first_mismatch.is_none();
            let replay_root = evidence_root.join("replays").join(replay_id.as_str());
            let refs = write_successful_run(
                &replay_root,
                &source.configuration,
                &source.fixture,
                &report,
            )
            .map(|bundle| bundle.artifacts)
            .unwrap_or_default();
            let replay_report = crate::replay_types::ReplayReport {
                contract_version: "1.0.0".to_owned(),
                replay_id: replay_id.clone(),
                source_run_id: stored.status.source_run_id,
                source_outcome: source.index.status,
                matched,
                first_mismatch,
                source_transition_count: summary["decision_steps"].as_u64().unwrap_or(0) as usize
                    + summary["execution_events"].as_u64().unwrap_or(0) as usize
                    + summary["settlement_events"].as_u64().unwrap_or(0) as usize,
                replay_transition_count: report.cycles.len()
                    + report.execution_events.len()
                    + report.settlement_events.len(),
                evidence_refs: refs,
            };
            let failure = (!matched).then(|| crate::replay_types::ReplayFailure {
                code: "REPLAY_DIVERGED".to_owned(),
                message: "Replay final portfolio differs from source evidence.".to_owned(),
            });
            let _ = store.mark_replay_terminal(
                &replay_id,
                timestamp_or(&stored.status.submitted_at),
                Some(replay_report),
                failure,
            );
        }
        Err(error) => {
            let matched_source_failure = terminal_failure.as_ref().is_some_and(|failure| {
                failure["stage"] == error.error.stage && failure["reason"] == error.error.reason
            });
            if matched_source_failure {
                let replay_root = evidence_root.join("replays").join(replay_id.as_str());
                let evidence_refs = match write_failed_run(
                    &replay_root,
                    &source.configuration,
                    &source.fixture,
                    &error,
                ) {
                    Ok(bundle) => bundle.artifacts,
                    Err(write_error) => {
                        let _ = store.mark_replay_terminal(
                            &replay_id,
                            timestamp_or(&stored.status.submitted_at),
                            None,
                            Some(crate::replay_types::ReplayFailure {
                                code: "REPLAY_EVIDENCE_FAILED".to_owned(),
                                message: write_error.to_string(),
                            }),
                        );
                        return;
                    }
                };
                let replay_report = crate::replay_types::ReplayReport {
                    contract_version: "1.0.0".to_owned(),
                    replay_id: replay_id.clone(),
                    source_run_id: stored.status.source_run_id,
                    source_outcome: "failed".to_owned(),
                    matched: true,
                    first_mismatch: None,
                    source_transition_count: summary
                        .as_ref()
                        .and_then(|value| value["decision_steps"].as_u64())
                        .unwrap_or(0) as usize,
                    replay_transition_count: error.partial_report.as_ref().map_or(0, |report| {
                        report.cycles.len()
                            + report.execution_events.len()
                            + report.settlement_events.len()
                    }),
                    evidence_refs,
                };
                let _ = store.mark_replay_terminal(
                    &replay_id,
                    timestamp_or(&stored.status.submitted_at),
                    Some(replay_report),
                    None,
                );
                return;
            }
            let _ = store.mark_replay_terminal(
                &replay_id,
                timestamp_or(&stored.status.submitted_at),
                None,
                Some(crate::replay_types::ReplayFailure {
                    code: "REPLAY_EXECUTION_FAILED".to_owned(),
                    message: error.to_string(),
                }),
            );
        }
    }
}

fn normalize_replay_value(value: serde_json::Value) -> serde_json::Value {
    const IGNORED: &[&str] = &[
        "run_id",
        "scenario_id",
        "inference_id",
        "snapshot_id",
        "order_id",
        "fill_id",
        "created_at",
        "requested_at",
        "responded_at",
    ];
    match value {
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .into_iter()
                .filter(|(key, _)| {
                    !IGNORED.contains(&key.as_str())
                        && !(key.ends_with("_id") && key != "instrument_id")
                })
                .map(|(key, value)| (key, normalize_replay_value(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(normalize_replay_value).collect())
        }
        other => other,
    }
}

fn replay_mismatch(replay: &serde_json::Value, source: &serde_json::Value) -> Option<String> {
    [
        "admissions",
        "orders",
        "executions",
        "settlements",
        "ledger",
        "cash",
        "positions",
        "nav",
    ]
    .into_iter()
    .find(|field| {
        normalize_replay_value(replay[*field].clone())
            != normalize_replay_value(source[*field].clone())
    })
    .map(str::to_owned)
}

fn collect_ledger_postings(value: &serde_json::Value) -> Vec<serde_json::Value> {
    fn collect(value: &serde_json::Value, postings: &mut Vec<serde_json::Value>) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, nested) in object {
                    if key.ends_with("_posting") && nested.is_object() {
                        postings.push(nested.clone());
                    }
                    collect(nested, postings);
                }
            }
            serde_json::Value::Array(values) => {
                for nested in values {
                    collect(nested, postings);
                }
            }
            _ => {}
        }
    }

    let mut postings = Vec::new();
    collect(value, &mut postings);
    postings
}

async fn execute_run(
    store: RunStore,
    fixtures: Arc<FixtureRegistry>,
    client: ModelHttpClient,
    evidence_root: PathBuf,
    run_id: Uuid,
) {
    let Some(stored) = store.get_run(&run_id) else {
        return;
    };
    if store
        .mark_running(&run_id, timestamp_or(&stored.status.submitted_at))
        .is_err()
    {
        mark_failed(
            &store,
            &run_id,
            "RUN_WORKER_START_FAILED",
            "The run could not enter the running state.",
            None,
            None,
            None,
        );
        return;
    }

    let configuration = stored.configuration;
    let fixture = match fixtures.get(&configuration.data) {
        Ok(fixture) => fixture,
        Err(_) => {
            mark_failed(
                &store,
                &run_id,
                "FIXTURE_UNAVAILABLE",
                "The registered run fixture is unavailable.",
                None,
                None,
                None,
            );
            return;
        }
    };
    let identity = RunIdentity {
        run_id: run_id.clone(),
        scenario_id: Identifier::new("baseline").expect("baseline is a valid scenario ID"),
    };
    let progress_store = store.clone();
    let progress_run_id = run_id.clone();
    let outcome = run_simulation_with_progress(
        &client,
        &configuration.model.endpoint,
        Duration::from_millis(configuration.model.timeout_ms),
        &configuration,
        &fixture,
        identity,
        move |progress| {
            let _ = progress_store.update_progress(&progress_run_id, progress);
        },
    )
    .await;

    match outcome {
        Ok(report) => {
            let evidence =
                match write_successful_run(&evidence_root, &configuration, &fixture, &report) {
                    Ok(evidence) => evidence,
                    Err(_) => {
                        mark_failed(
                            &store,
                            &run_id,
                            "EVIDENCE_PERSISTENCE_FAILED",
                            "Run evidence could not be verified and stored.",
                            Some(report.identity.scenario_id),
                            None,
                            None,
                        );
                        return;
                    }
                };
            let finished_at = timestamp_or(&stored.status.submitted_at);
            let result = match build_run_result(
                &configuration,
                &fixture,
                &report,
                finished_at.clone(),
                &evidence,
            ) {
                Ok(result) => result,
                Err(_) => {
                    mark_failed(
                        &store,
                        &run_id,
                        "RESULT_VALIDATION_FAILED",
                        "The completed run result failed validation.",
                        Some(report.identity.scenario_id),
                        None,
                        None,
                    );
                    return;
                }
            };
            let _ = store.mark_succeeded(&run_id, finished_at, result);
        }
        Err(failure) => {
            let scenario_id = Some(failure.identity.scenario_id.clone());
            let step_id = failure
                .failed_inference
                .as_ref()
                .map(|exchange| exchange.request.step_id.clone())
                .or_else(|| {
                    failure
                        .partial_report
                        .as_ref()
                        .and_then(|report| report.cycles.last())
                        .map(|cycle| cycle.request.step_id.clone())
                });
            let state_ref = write_failed_run(&evidence_root, &configuration, &fixture, &failure)
                .ok()
                .and_then(|bundle| {
                    bundle
                        .artifacts
                        .iter()
                        .find(|artifact| artifact.kind == EvidenceKind::Summary)
                        .map(|artifact| artifact.relative_path.clone())
                });
            mark_failed(
                &store,
                &run_id,
                "RUN_EXECUTION_FAILED",
                "The run failed during controlled simulation execution.",
                scenario_id,
                step_id,
                state_ref,
            );
        }
    }
}

fn mark_failed(
    store: &RunStore,
    run_id: &Uuid,
    code: &'static str,
    message: &'static str,
    scenario_id: Option<Identifier>,
    step_id: Option<Identifier>,
    last_valid_state_ref: Option<String>,
) {
    let Some(run) = store.get_run(run_id) else {
        return;
    };
    let _ = store.mark_failed(
        run_id,
        timestamp_or(&run.status.submitted_at),
        RunFailure {
            code: code.to_owned(),
            message: message.to_owned(),
            scenario_id,
            step_id,
            last_valid_state_ref,
        },
    );
}

fn mark_replay_worker_panicked(store: &RunStore, replay_id: &Uuid) {
    if let Some(replay) = store.get_replay(replay_id) {
        let at = timestamp_or(&replay.status.submitted_at);
        if replay.status.status == "queued" {
            let _ = store.mark_replay_running(replay_id, at.clone());
        }
        let _ = store.mark_replay_terminal(
            replay_id,
            at,
            None,
            Some(crate::replay_types::ReplayFailure {
                code: "REPLAY_WORKER_PANICKED".to_owned(),
                message: "The replay worker stopped unexpectedly.".to_owned(),
            }),
        );
    }
}

fn timestamp_or(fallback: &UtcTimestamp) -> UtcTimestamp {
    system_utc_timestamp().unwrap_or_else(|_| fallback.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend_config::BackendConfig, backend_config::QueueLimits};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn replay_comparator_reports_first_financial_transition_mismatch() {
        let source = serde_json::json!({
            "admissions": [{"instrument_id": "SPOT_A", "admitted_quantity": "1"}],
            "orders": [{"order_id": "old", "instrument_id": "SPOT_A"}],
            "executions": [],
            "settlements": [],
            "ledger": [{"cash_delta": "-100.00"}],
            "cash": "900.00",
            "positions": [{"instrument_id": "SPOT_A", "quantity": "1"}],
            "nav": "1000.00"
        });
        let mut replay = source.clone();
        replay["orders"][0]["order_id"] = serde_json::json!("regenerated");
        assert_eq!(replay_mismatch(&replay, &source), None);

        replay["ledger"][0]["cash_delta"] = serde_json::json!("-99.00");
        assert_eq!(replay_mismatch(&replay, &source).as_deref(), Some("ledger"));
        replay["admissions"][0]["admitted_quantity"] = serde_json::json!("2");
        assert_eq!(
            replay_mismatch(&replay, &source).as_deref(),
            Some("admissions")
        );
    }

    #[test]
    fn replay_worker_panic_callback_terminalizes_queued_and_running_jobs() {
        let store = RunStore::new(QueueLimits::new(2, 0).unwrap());
        let queued_id = Uuid::new("b1c0e095-c4b1-4a65-a05a-1b5689911ac2").unwrap();
        let running_id = Uuid::new("b1c0e095-c4b1-4a65-a05a-1b5689911ac3").unwrap();
        let source = Uuid::new("b1c0e095-c4b1-4a65-a05a-1b5689911ac4").unwrap();
        let queued = store
            .accept_replay(
                queued_id.clone(),
                source.clone(),
                UtcTimestamp::new("2026-01-05T09:00:00Z").unwrap(),
            )
            .unwrap();
        store
            .accept_replay(
                running_id.clone(),
                source,
                UtcTimestamp::new("2026-01-05T09:00:00Z").unwrap(),
            )
            .unwrap();
        store
            .mark_replay_running(&running_id, queued.acknowledgement.submitted_at.clone())
            .unwrap();

        mark_replay_worker_panicked(&store, &queued_id);
        mark_replay_worker_panicked(&store, &running_id);

        for replay_id in [queued_id, running_id] {
            let replay = store.get_replay(&replay_id).unwrap();
            assert_eq!(replay.status.status, "failed");
            assert_eq!(
                replay.status.failure.unwrap().code,
                "REPLAY_WORKER_PANICKED"
            );
        }
    }

    #[tokio::test]
    async fn startup_loads_local_fixture_registry_and_shutdown_joins_idle_workers() {
        let evidence_directory = std::env::temp_dir().join(format!(
            "factorsbt-runtime-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = BackendConfig {
            queue_limits: QueueLimits::new(1, 0).unwrap(),
            evidence_directory: evidence_directory.clone(),
            ..BackendConfig::default()
        };
        let runtime = BackendRuntime::start(&config).unwrap();
        assert_eq!(runtime.fixtures().len(), 9);
        assert!(evidence_directory.is_dir());
        runtime.shutdown().await;
        fs::remove_dir_all(evidence_directory).unwrap();
    }
}
