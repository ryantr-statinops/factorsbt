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
                        JobId::Replay(_) => {}
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
                    if let Some(replay) = panic_store.get_replay(&replay_id) {
                        let _ = panic_store.mark_replay_terminal(
                            &replay_id,
                            timestamp_or(&replay.status.submitted_at),
                            None,
                            Some(crate::replay_types::ReplayFailure {
                                code: "REPLAY_WORKER_PANICKED".to_owned(),
                                message: "The replay worker stopped unexpectedly.".to_owned(),
                            }),
                        );
                    }
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

fn timestamp_or(fallback: &UtcTimestamp) -> UtcTimestamp {
    system_utc_timestamp().unwrap_or_else(|_| fallback.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend_config::BackendConfig, backend_config::QueueLimits};
    use std::time::{SystemTime, UNIX_EPOCH};

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
        assert_eq!(runtime.fixtures().len(), 7);
        assert!(evidence_directory.is_dir());
        runtime.shutdown().await;
        fs::remove_dir_all(evidence_directory).unwrap();
    }
}
