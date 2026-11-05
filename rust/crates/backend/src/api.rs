//! MVP run operations over the accepted local HTTP contract.

use crate::{
    admission::{validate_run, RunAdmissionError},
    api_errors::{ApiJson, ApiPath, ApiResponseError, RequestContext},
    evidence::{read_evidence_index, read_verified_artifact, EvidenceError, EvidenceIndex},
    replay_source::ReplaySource,
    replay_types::{ReplayAcknowledgement, ReplayReport, ReplayRequest, ReplayStatus},
    research::jobs::{
        validate_request, ResearchJobAcknowledgement, ResearchJobRequest, ResearchJobResult,
        ResearchJobStatus,
    },
    results::verify_result_evidence,
    run_store::{RunResultLookupError, RunStore, SubmissionStoreError},
};
use axum::{
    body::Body,
    extract::{Extension, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use factorsbt_contracts::{
    outcomes::{RunResult, RunStatus, SubmissionAccepted, SubmissionStatus, ValidationDetail},
    Uuid,
};
use factorsbt_data::FixtureRegistry;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

const IDEMPOTENCY_KEY: &str = "idempotency-key";

#[derive(Clone)]
pub struct ApiState {
    store: RunStore,
    fixtures: Arc<FixtureRegistry>,
    evidence_root: PathBuf,
}

impl ApiState {
    pub fn new(
        store: RunStore,
        fixtures: Arc<FixtureRegistry>,
        evidence_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            store,
            fixtures,
            evidence_root: evidence_root.into(),
        }
    }
}

/// Routes are merged with `/healthz` by the binary and share its request-ID
/// middleware. The state is owned by the backend runtime for this process.
pub fn run_routes(state: ApiState) -> Router {
    Router::new()
        .route("/v1/runs", post(submit_run))
        .route("/v1/replays", post(submit_replay))
        .route("/v1/replays/{replay_id}", get(get_replay))
        .route("/v1/replays/{replay_id}/report", get(get_replay_report))
        .route("/v1/research/jobs", post(submit_research_job))
        .route("/v1/research/jobs/{job_id}", get(get_research_job))
        .route(
            "/v1/research/jobs/{job_id}/result",
            get(get_research_result),
        )
        .route("/v1/submissions/{submission_id}", get(get_submission))
        .route("/v1/runs/{run_id}", get(get_run))
        .route("/v1/runs/{run_id}/results", get(get_results))
        .route("/v1/runs/{run_id}/evidence", get(get_evidence))
        .route(
            "/v1/runs/{run_id}/artifacts/{artifact_id}",
            get(get_artifact),
        )
        .with_state(state)
}

async fn submit_research_job(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    ApiJson(value): ApiJson<Value>,
) -> Result<(StatusCode, Json<ResearchJobAcknowledgement>), ApiResponseError> {
    let job_id = headers
        .get(IDEMPOTENCY_KEY)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::new(value).ok())
        .ok_or_else(|| {
            ApiResponseError::bad_request(
                &context,
                "INVALID_IDEMPOTENCY_KEY",
                "Idempotency-Key must contain a lowercase UUID.",
            )
        })?;
    let request: ResearchJobRequest = serde_json::from_value(value).map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::UNPROCESSABLE_ENTITY,
            "RESEARCH_REQUEST_INVALID",
            "Research job request is invalid.",
            None,
        )
    })?;
    if state.store.get_research_job(&job_id).is_some() {
        let submitted_at = crate::orchestrator::system_utc_timestamp().map_err(|_| {
            ApiResponseError::new(
                &context,
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Could not record research job submission time.",
                None,
            )
        })?;
        let outcome = state
            .store
            .accept_research_job(job_id, request, submitted_at)
            .map_err(|error| submission_store_error(&context, error))?;
        return Ok((StatusCode::ACCEPTED, Json(outcome.acknowledgement)));
    }
    validate_request(&request).map_err(|error| {
        ApiResponseError::new(
            &context,
            StatusCode::UNPROCESSABLE_ENTITY,
            "RESEARCH_REQUEST_INVALID",
            error.to_string(),
            None,
        )
    })?;
    match request.kind {
        crate::research::jobs::ResearchJobKind::Reranking
        | crate::research::jobs::ResearchJobKind::Episode => {
            let fixture = crate::research::fixtures::resolve_fixture(
                request.fixture_id.as_deref().unwrap_or_default(),
                request.fixture_version.as_deref().unwrap_or_default(),
            )
            .map_err(|_| {
                ApiResponseError::new(
                    &context,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "RESEARCH_FIXTURE_INVALID",
                    "Research fixture is unavailable or incompatible.",
                    None,
                )
            })?;
            if (request.kind == crate::research::jobs::ResearchJobKind::Reranking
                && fixture.kind != crate::research::fixtures::ResearchFixtureKind::Reranking)
                || (request.kind == crate::research::jobs::ResearchJobKind::Episode
                    && fixture.kind != crate::research::fixtures::ResearchFixtureKind::Episode)
            {
                return Err(ApiResponseError::new(
                    &context,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "RESEARCH_FIXTURE_INVALID",
                    "Research fixture kind does not match the request.",
                    None,
                ));
            }
        }
        crate::research::jobs::ResearchJobKind::Replay => {
            let source_id = request
                .source_job_id
                .as_ref()
                .expect("validated replay source");
            let source = state
                .store
                .get_research_job(source_id)
                .filter(|job| {
                    job.status.status == "succeeded"
                        && job.status.kind != crate::research::jobs::ResearchJobKind::Replay
                })
                .ok_or_else(|| {
                    ApiResponseError::new(
                        &context,
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "RESEARCH_SOURCE_INELIGIBLE",
                        "Research replay source is not a completed original job.",
                        None,
                    )
                })?;
            drop(source);
            crate::research::evidence::load_job(&state.evidence_root, source_id).map_err(|_| {
                ApiResponseError::new(
                    &context,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "RESEARCH_SOURCE_INCOMPATIBLE",
                    "Research replay source evidence is missing, altered, or incompatible.",
                    None,
                )
            })?;
        }
    }
    let submitted_at = crate::orchestrator::system_utc_timestamp().map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Could not record research job submission time.",
            None,
        )
    })?;
    let outcome = state
        .store
        .accept_research_job(job_id, request, submitted_at)
        .map_err(|error| submission_store_error(&context, error))?;
    Ok((StatusCode::ACCEPTED, Json(outcome.acknowledgement)))
}

async fn get_research_job(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(job_id): ApiPath<Uuid>,
) -> Result<Json<ResearchJobStatus>, ApiResponseError> {
    state
        .store
        .get_research_job(&job_id)
        .map(|item| Json(item.status))
        .ok_or_else(|| {
            ApiResponseError::new(
                &context,
                StatusCode::NOT_FOUND,
                "RESEARCH_JOB_NOT_FOUND",
                "No research job has this ID.",
                None,
            )
        })
}

async fn get_research_result(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(job_id): ApiPath<Uuid>,
) -> Result<Json<ResearchJobResult>, ApiResponseError> {
    let item = state.store.get_research_job(&job_id).ok_or_else(|| {
        ApiResponseError::new(
            &context,
            StatusCode::NOT_FOUND,
            "RESEARCH_JOB_NOT_FOUND",
            "No research job has this ID.",
            None,
        )
    })?;
    item.result.map(Json).ok_or_else(|| {
        ApiResponseError::new(
            &context,
            StatusCode::CONFLICT,
            "RESEARCH_RESULT_UNAVAILABLE",
            "Research result is available after successful execution.",
            None,
        )
    })
}

async fn submit_replay(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    ApiJson(value): ApiJson<Value>,
) -> Result<(StatusCode, Json<ReplayAcknowledgement>), ApiResponseError> {
    let replay_id = headers
        .get(IDEMPOTENCY_KEY)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::new(value).ok())
        .ok_or_else(|| {
            ApiResponseError::bad_request(
                &context,
                "INVALID_IDEMPOTENCY_KEY",
                "Idempotency-Key must contain a lowercase UUID.",
            )
        })?;
    let request: ReplayRequest = serde_json::from_value(value).map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::UNPROCESSABLE_ENTITY,
            "REPLAY_REQUEST_INVALID",
            "Replay request is invalid.",
            None,
        )
    })?;
    if request.contract_version != "1.0.0" {
        return Err(ApiResponseError::new(
            &context,
            StatusCode::UNPROCESSABLE_ENTITY,
            "CONTRACT_VERSION_UNSUPPORTED",
            "The backend supports contract version 1.0.0.",
            None,
        ));
    }
    if state.store.get_replay(&replay_id).is_none() {
        let source = ReplaySource::load(&state.evidence_root, &request.source_run_id)
            .and_then(|source| {
                source.verify_replay_eligibility(&state.evidence_root)?;
                Ok(source)
            })
            .map_err(|_error| {
                ApiResponseError::new(
                    &context,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "REPLAY_SOURCE_INCOMPATIBLE",
                    "Replay source is missing, incomplete, or incompatible.",
                    None,
                )
            })?;
        drop(source);
    }
    let submitted_at = crate::orchestrator::system_utc_timestamp().map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Could not record replay submission time.",
            None,
        )
    })?;
    let outcome = state
        .store
        .accept_replay(replay_id, request.source_run_id, submitted_at)
        .map_err(|error| submission_store_error(&context, error))?;
    Ok((StatusCode::ACCEPTED, Json(outcome.acknowledgement)))
}

async fn get_replay(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(replay_id): ApiPath<Uuid>,
) -> Result<Json<ReplayStatus>, ApiResponseError> {
    state
        .store
        .get_replay(&replay_id)
        .map(|item| Json(item.status))
        .ok_or_else(|| {
            ApiResponseError::new(
                &context,
                StatusCode::NOT_FOUND,
                "REPLAY_NOT_FOUND",
                "No retained replay has this ID.",
                None,
            )
        })
}

async fn get_replay_report(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(replay_id): ApiPath<Uuid>,
) -> Result<Json<ReplayReport>, ApiResponseError> {
    let item = state.store.get_replay(&replay_id).ok_or_else(|| {
        ApiResponseError::new(
            &context,
            StatusCode::NOT_FOUND,
            "REPLAY_NOT_FOUND",
            "No retained replay has this ID.",
            None,
        )
    })?;
    item.report.map(Json).ok_or_else(|| {
        ApiResponseError::new(
            &context,
            StatusCode::CONFLICT,
            "REPLAY_NOT_COMPLETE",
            "Replay report is available after verification completes.",
            None,
        )
    })
}

async fn submit_run(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    ApiJson(value): ApiJson<Value>,
) -> Result<(StatusCode, Json<SubmissionAccepted>), ApiResponseError> {
    let submission_id = headers
        .get(IDEMPOTENCY_KEY)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::new(value).ok())
        .ok_or_else(|| {
            ApiResponseError::bad_request(
                &context,
                "INVALID_IDEMPOTENCY_KEY",
                "Idempotency-Key must contain a lowercase UUID.",
            )
        })?;

    if value
        .get("contract_version")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "1.0.0")
    {
        return Err(ApiResponseError::new(
            &context,
            StatusCode::UNPROCESSABLE_ENTITY,
            "CONTRACT_VERSION_UNSUPPORTED",
            "The backend supports contract version 1.0.0.",
            None,
        ));
    }

    let validated =
        validate_run(&value, &state.fixtures).map_err(|error| admission_error(&context, error))?;
    let submitted_at = crate::orchestrator::system_utc_timestamp().map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "The backend could not record the submission time.",
            None,
        )
    })?;
    let outcome = state
        .store
        .accept_submission(submission_id, validated.configuration, submitted_at)
        .map_err(|error| submission_store_error(&context, error))?;
    Ok((StatusCode::ACCEPTED, Json(outcome.acknowledgement)))
}

async fn get_submission(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(submission_id): ApiPath<Uuid>,
) -> Result<Json<SubmissionStatus>, ApiResponseError> {
    state
        .store
        .get_submission(&submission_id)
        .map(Json)
        .ok_or_else(|| {
            ApiResponseError::new(
                &context,
                StatusCode::NOT_FOUND,
                "SUBMISSION_NOT_FOUND",
                "No retained submission has this ID.",
                None,
            )
        })
}

async fn get_run(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(run_id): ApiPath<Uuid>,
) -> Result<Json<RunStatus>, ApiResponseError> {
    state
        .store
        .get_run(&run_id)
        .map(|run| Json(run.status))
        .ok_or_else(|| {
            ApiResponseError::new(
                &context,
                StatusCode::NOT_FOUND,
                "RUN_NOT_FOUND",
                "No retained run has this ID.",
                None,
            )
        })
}

async fn get_results(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(run_id): ApiPath<Uuid>,
) -> Result<Json<RunResult>, ApiResponseError> {
    let result = state
        .store
        .get_result(&run_id)
        .map_err(|error| match error {
            RunResultLookupError::NotFound => ApiResponseError::new(
                &context,
                StatusCode::NOT_FOUND,
                "RUN_NOT_FOUND",
                "No retained run has this ID.",
                None,
            ),
            RunResultLookupError::NotSucceeded => ApiResponseError::new(
                &context,
                StatusCode::CONFLICT,
                "RUN_NOT_SUCCEEDED",
                "Run results are available after successful completion.",
                None,
            ),
            RunResultLookupError::Failed => ApiResponseError::new(
                &context,
                StatusCode::CONFLICT,
                "RUN_FAILED",
                "The run failed; inspect its status resource for details.",
                None,
            ),
            RunResultLookupError::ResultUnavailable => ApiResponseError::new(
                &context,
                StatusCode::SERVICE_UNAVAILABLE,
                "BACKEND_UNAVAILABLE",
                "The completed run result is temporarily unavailable.",
                None,
            ),
        })?;
    let run_directory = state.evidence_root.join("runs").join(run_id.as_str());
    verify_result_evidence(&run_directory, &result).map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Run evidence failed integrity verification.",
            None,
        )
    })?;
    Ok(Json(result))
}

async fn get_evidence(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath(run_id): ApiPath<Uuid>,
) -> Result<Json<EvidenceIndex>, ApiResponseError> {
    read_evidence_index(&state.evidence_root, &run_id)
        .map(Json)
        .map_err(|error| evidence_error(&state, &context, &run_id, error))
}

async fn get_artifact(
    State(state): State<ApiState>,
    Extension(context): Extension<RequestContext>,
    ApiPath((run_id, artifact_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<Response, ApiResponseError> {
    let (reference, bytes) = read_verified_artifact(&state.evidence_root, &run_id, &artifact_id)
        .map_err(|error| evidence_error(&state, &context, &run_id, error))?;
    let content_type = HeaderValue::from_str(&reference.content_type).map_err(|_| {
        ApiResponseError::new(
            &context,
            StatusCode::INTERNAL_SERVER_ERROR,
            "EVIDENCE_REFERENCE_INVALID",
            "Stored artifact media type is invalid.",
            None,
        )
    })?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .expect("verified response headers are valid"))
}

fn evidence_error(
    state: &ApiState,
    context: &RequestContext,
    run_id: &Uuid,
    error: EvidenceError,
) -> ApiResponseError {
    let (status, code, message) = match error {
        EvidenceError::RunNotFound => {
            match state.store.get_run(run_id).map(|run| run.status.status) {
                Some(factorsbt_contracts::outcomes::RunLifecycle::Queued)
                | Some(factorsbt_contracts::outcomes::RunLifecycle::Running) => (
                    StatusCode::CONFLICT,
                    "RUN_NOT_SUCCEEDED",
                    "Run evidence is available after terminal completion.",
                ),
                Some(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "EVIDENCE_REFERENCE_INVALID",
                    "Terminal run evidence is unavailable.",
                ),
                None => (
                    StatusCode::NOT_FOUND,
                    "RUN_NOT_FOUND",
                    "Run evidence was not found.",
                ),
            }
        }
        EvidenceError::ArtifactNotFound => (
            StatusCode::NOT_FOUND,
            "EVIDENCE_ARTIFACT_NOT_FOUND",
            "Artifact ID was not found for this run.",
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "EVIDENCE_REFERENCE_INVALID",
            "Run evidence failed integrity verification.",
        ),
    };
    ApiResponseError::new(context, status, code, message, None)
}

fn submission_store_error(
    context: &RequestContext,
    error: SubmissionStoreError,
) -> ApiResponseError {
    match error {
        SubmissionStoreError::SubmissionIdReused => ApiResponseError::new(
            context,
            StatusCode::CONFLICT,
            "SUBMISSION_ID_REUSED",
            "This submission ID is already bound to a different configuration.",
            None,
        ),
        SubmissionStoreError::QueueFull | SubmissionStoreError::BackendUnavailable => {
            ApiResponseError::new(
                context,
                StatusCode::SERVICE_UNAVAILABLE,
                "BACKEND_UNAVAILABLE",
                "The backend cannot accept a new run at this time.",
                None,
            )
        }
    }
}

fn admission_error(context: &RequestContext, error: RunAdmissionError) -> ApiResponseError {
    match error {
        RunAdmissionError::SchemaInvalid(issues) => {
            let details = issues
                .iter()
                .map(|issue| ValidationDetail {
                    path: schema_error_path(issue),
                    reason: "Value does not match the declared run configuration schema.".into(),
                })
                .collect();
            ApiResponseError::new(
                context,
                StatusCode::UNPROCESSABLE_ENTITY,
                "SCHEMA_VALIDATION_FAILED",
                "Run configuration does not match the declared JSON schema.",
                Some(details),
            )
        }
        RunAdmissionError::InvalidWireConfiguration(_)
        | RunAdmissionError::FixtureNotRegistered { .. }
        | RunAdmissionError::InvalidModelEndpoint
        | RunAdmissionError::InvalidSimulation(_)
        | RunAdmissionError::InvalidFixtureCoverage(_)
        | RunAdmissionError::InvalidInitialPortfolio(_) => ApiResponseError::new(
            context,
            StatusCode::UNPROCESSABLE_ENTITY,
            "RUN_CONFIGURATION_INVALID",
            "Run configuration violates an MVP domain constraint.",
            None,
        ),
    }
}

fn schema_error_path(issue: &str) -> String {
    let pointer = issue.rsplit_once(" at ").map_or("", |(_, path)| path);
    if pointer.is_empty() {
        return "$".to_owned();
    }
    let mut result = "$".to_owned();
    for part in pointer.trim_start_matches('/').split('/') {
        let part = part.replace("~1", "/").replace("~0", "~");
        if part.parse::<usize>().is_ok() {
            result.push_str(&format!("[{part}]"));
        } else {
            result.push('.');
            result.push_str(&part);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api_errors::request_id_middleware,
        backend_config::{BackendConfig, QueueLimits},
        model_fixture::HttpModelFixture,
        runtime::BackendRuntime,
    };
    use axum::{
        body::{to_bytes, Body},
        extract::State as AxumState,
        http::Request,
        middleware,
        routing::post as axum_post,
    };
    use factorsbt_contracts::{model::ModelRequest, outcomes::RunLifecycle, UtcTimestamp};
    use serde_json::json;
    use std::{
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tokio::{
        net::TcpListener,
        sync::{Barrier, Notify},
    };
    use tower::ServiceExt;

    fn state_with_queue(active: usize, queued: usize) -> (ApiState, RunStore) {
        let fixture_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let fixtures = Arc::new(FixtureRegistry::load(fixture_root).unwrap());
        let store = RunStore::new(QueueLimits::new(active, queued).unwrap());
        let evidence_root = std::env::temp_dir().join(format!(
            "factorsbt-api-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (ApiState::new(store.clone(), fixtures, evidence_root), store)
    }

    fn app(state: ApiState) -> Router {
        run_routes(state).layer(middleware::from_fn(request_id_middleware))
    }

    fn config_value() -> Value {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap()
    }

    fn json_request(key: &str, value: Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/v1/runs")
            .header("content-type", "application/json")
            .header(IDEMPOTENCY_KEY, key)
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap()
    }

    fn valid_key(number: u8) -> String {
        format!("00000000-0000-4000-8000-{number:012x}")
    }

    fn unique_temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "factorsbt-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    async fn spawn_server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{address}"), task)
    }

    fn runtime_config(evidence_root: PathBuf) -> crate::backend_config::BackendConfig {
        crate::backend_config::BackendConfig {
            queue_limits: QueueLimits::new(2, 16).unwrap(),
            evidence_directory: evidence_root,
            ..crate::backend_config::BackendConfig::default()
        }
    }

    fn config_for_endpoint(endpoint: &str, model_id: &str) -> Value {
        let mut value = config_value();
        value["model"]["endpoint"] = Value::String(endpoint.to_owned());
        value["model"]["model_id"] = Value::String(model_id.to_owned());
        value
    }

    #[derive(Clone)]
    struct PairedModelState {
        fixture: HttpModelFixture,
        barrier: Arc<Barrier>,
        requests: Arc<AtomicUsize>,
        first_pair_ready: Arc<Notify>,
        in_flight: Arc<AtomicUsize>,
        max_in_flight: Arc<AtomicUsize>,
        completed: Arc<Mutex<Vec<(String, String)>>>,
    }

    async fn paired_model_response(
        AxumState(state): AxumState<PairedModelState>,
        Json(request): Json<ModelRequest>,
    ) -> Json<Value> {
        let current = state.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        state.max_in_flight.fetch_max(current, Ordering::SeqCst);
        if state.requests.fetch_add(1, Ordering::SeqCst) + 1 == 2 {
            state.first_pair_ready.notify_one();
        }
        state.barrier.wait().await;
        if request.model_id.as_str() == "slow-model" {
            tokio::time::sleep(Duration::from_millis(35)).await;
        }
        state.completed.lock().unwrap().push((
            request.model_id.as_str().to_owned(),
            request.step_id.as_str().to_owned(),
        ));
        state.in_flight.fetch_sub(1, Ordering::SeqCst);
        Json(state.fixture.response_document(&request))
    }

    async fn await_terminal_status(
        client: &reqwest::Client,
        base: &str,
        run_id: &Uuid,
    ) -> RunStatus {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = client
                    .get(format!("{base}/v1/runs/{}", run_id.as_str()))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let status: RunStatus = response.json().await.unwrap();
                if matches!(
                    status.status,
                    RunLifecycle::Succeeded | RunLifecycle::Failed
                ) {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("run should reach a terminal lifecycle state")
    }

    #[tokio::test]
    async fn concurrent_identical_posts_share_one_ack_and_changed_config_conflicts() {
        let (state, store) = state_with_queue(1, 2);
        let app = app(state);
        let key = valid_key(1);
        let config = config_value();
        let (left, right) = tokio::join!(
            app.clone().oneshot(json_request(&key, config.clone())),
            app.clone().oneshot(json_request(&key, config.clone()))
        );
        let left = left.unwrap();
        let right = right.unwrap();
        assert_eq!(left.status(), StatusCode::ACCEPTED);
        assert_eq!(right.status(), StatusCode::ACCEPTED);
        let left_body: SubmissionAccepted = serde_json::from_slice(
            &axum::body::to_bytes(left.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let right_body: SubmissionAccepted = serde_json::from_slice(
            &axum::body::to_bytes(right.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(left_body, right_body);
        assert_eq!(store.record_counts().runs, 1);

        let mut changed = config;
        changed["model"]["model_version"] = Value::String("v2".to_owned());
        let conflict = app.oneshot(json_request(&key, changed)).await.unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(conflict.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "SUBMISSION_ID_REUSED");
        assert_eq!(store.record_counts().runs, 1);
    }

    #[tokio::test]
    async fn queue_overflow_and_invalid_admission_create_no_extra_records() {
        let (state, store) = state_with_queue(1, 1);
        let app = app(state);
        let mut queued_ack = None;
        for number in 2..=4 {
            let response = app
                .clone()
                .oneshot(json_request(&valid_key(number), config_value()))
                .await
                .unwrap();
            if number < 4 {
                assert_eq!(response.status(), StatusCode::ACCEPTED);
                assert_eq!(store.record_counts().runs, usize::from(number - 1));
                if number == 3 {
                    queued_ack = Some(
                        serde_json::from_slice::<SubmissionAccepted>(
                            &axum::body::to_bytes(response.into_body(), usize::MAX)
                                .await
                                .unwrap(),
                        )
                        .unwrap(),
                    );
                }
            } else {
                assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
                let body: Value = serde_json::from_slice(
                    &axum::body::to_bytes(response.into_body(), usize::MAX)
                        .await
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(body["error"]["code"], "BACKEND_UNAVAILABLE");
                assert_eq!(store.record_counts().runs, 2);
            }
        }
        let retried_full_queue = app
            .clone()
            .oneshot(json_request(&valid_key(3), config_value()))
            .await
            .unwrap();
        assert_eq!(retried_full_queue.status(), StatusCode::ACCEPTED);
        let retry_ack: SubmissionAccepted = serde_json::from_slice(
            &axum::body::to_bytes(retried_full_queue.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(Some(retry_ack), queued_ack);

        let before_invalid = store.record_counts();
        let mut invalid = config_value();
        invalid["unexpected"] = Value::Bool(true);
        let response = app
            .clone()
            .oneshot(json_request(&valid_key(5), invalid))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "SCHEMA_VALIDATION_FAILED");
        assert_eq!(store.record_counts(), before_invalid);

        let missing_key = Request::builder()
            .method("POST")
            .uri("/v1/runs")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&config_value()).unwrap()))
            .unwrap();
        let response = app.clone().oneshot(missing_key).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "INVALID_IDEMPOTENCY_KEY");

        let mut unsupported = config_value();
        unsupported["contract_version"] = Value::String("2.0.0".to_owned());
        let response = app
            .oneshot(json_request(&valid_key(6), unsupported))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "CONTRACT_VERSION_UNSUPPORTED");
        assert_eq!(store.record_counts(), before_invalid);
    }

    #[tokio::test]
    async fn lookup_paths_return_structured_not_found_and_invalid_id_errors() {
        let (state, _) = state_with_queue(1, 0);
        let response = app(state)
            .oneshot(
                Request::builder()
                    .uri("/v1/runs/not-a-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let request_id = response.headers()["x-request-id"].clone();
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "INVALID_RESOURCE_ID");
        assert_eq!(
            body["error"]["request_id"].as_str(),
            request_id.to_str().ok()
        );
    }

    #[tokio::test]
    async fn replay_rejects_missing_source_before_queue_admission() {
        let (state, store) = state_with_queue(1, 1);
        let key = valid_key(77);
        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/replays")
                    .header("content-type", "application/json")
                    .header(IDEMPOTENCY_KEY, &key)
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "contract_version": "1.0.0",
                            "source_run_id": "a4ab8716-3d8c-42d7-857c-83e94df28ff6"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["error"]["code"], "REPLAY_SOURCE_INCOMPATIBLE");
        assert_eq!(store.queue_snapshot().active_runs, 0);
        assert_eq!(store.queue_snapshot().queued_runs, 0);
        assert!(store.get_replay(&Uuid::new(&key).unwrap()).is_none());
    }

    #[tokio::test]
    async fn loopback_api_runs_two_isolated_simulations_and_verifies_result_evidence() {
        let fixture = HttpModelFixture::new(
            "scores",
            json!({"scores":[{"instrument_id":"SPOT_A","score":"1"}]}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        );
        let model_state = PairedModelState {
            fixture,
            barrier: Arc::new(Barrier::new(2)),
            requests: Arc::new(AtomicUsize::new(0)),
            first_pair_ready: Arc::new(Notify::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            max_in_flight: Arc::new(AtomicUsize::new(0)),
            completed: Arc::new(Mutex::new(Vec::new())),
        };
        let model_state_check = model_state.clone();
        let model_router = Router::new()
            .route("/infer", axum_post(paired_model_response))
            .with_state(model_state);
        let (model_base, model_server) = spawn_server(model_router).await;

        let evidence_root = unique_temp_path("api-evidence");
        let runtime = BackendRuntime::start(&runtime_config(evidence_root.clone())).unwrap();
        let api_state = ApiState::new(runtime.store(), runtime.fixtures(), evidence_root.clone());
        let (api_base, api_server) =
            spawn_server(run_routes(api_state).layer(middleware::from_fn(request_id_middleware)))
                .await;
        let client = reqwest::Client::new();
        let slow_config = config_for_endpoint(&format!("{model_base}/infer"), "slow-model");
        let fast_config = config_for_endpoint(&format!("{model_base}/infer"), "fast-model");
        let (slow_response, fast_response) = tokio::join!(
            client
                .post(format!("{api_base}/v1/runs"))
                .header(IDEMPOTENCY_KEY, valid_key(31))
                .json(&slow_config)
                .send(),
            client
                .post(format!("{api_base}/v1/runs"))
                .header(IDEMPOTENCY_KEY, valid_key(32))
                .json(&fast_config)
                .send()
        );
        let slow_response = slow_response.unwrap();
        let fast_response = fast_response.unwrap();
        assert_eq!(slow_response.status(), StatusCode::ACCEPTED);
        assert_eq!(fast_response.status(), StatusCode::ACCEPTED);
        let slow_ack: SubmissionAccepted = slow_response.json().await.unwrap();
        let fast_ack: SubmissionAccepted = fast_response.json().await.unwrap();
        assert_ne!(slow_ack.run_id, fast_ack.run_id);

        tokio::time::timeout(
            Duration::from_secs(5),
            model_state_check.first_pair_ready.notified(),
        )
        .await
        .expect("both model requests should be in flight concurrently");
        for accepted in [&slow_ack, &fast_ack] {
            let submission: SubmissionStatus = client
                .get(format!(
                    "{api_base}/v1/submissions/{}",
                    accepted.submission_id.as_str()
                ))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(submission.run_id, accepted.run_id);
            assert_eq!(submission.status, RunLifecycle::Running);
            let premature = client
                .get(format!(
                    "{api_base}/v1/runs/{}/results",
                    accepted.run_id.as_str()
                ))
                .send()
                .await
                .unwrap();
            assert_eq!(premature.status(), StatusCode::CONFLICT);
        }

        let slow_status = await_terminal_status(&client, &api_base, &slow_ack.run_id).await;
        let fast_status = await_terminal_status(&client, &api_base, &fast_ack.run_id).await;
        assert_eq!(slow_status.status, RunLifecycle::Succeeded);
        assert_eq!(fast_status.status, RunLifecycle::Succeeded);
        assert!(slow_status.progress.unwrap().completed_steps > 0);
        assert!(fast_status.progress.unwrap().completed_steps > 0);

        let slow_result: RunResult = client
            .get(format!(
                "{api_base}/v1/runs/{}/results",
                slow_ack.run_id.as_str()
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let fast_result: RunResult = client
            .get(format!(
                "{api_base}/v1/runs/{}/results",
                fast_ack.run_id.as_str()
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(slow_result.run_id, slow_ack.run_id);
        assert_eq!(fast_result.run_id, fast_ack.run_id);
        assert_eq!(
            slow_result.provenance.model_invocation.model_id.as_str(),
            "slow-model"
        );
        assert_eq!(
            fast_result.provenance.model_invocation.model_id.as_str(),
            "fast-model"
        );
        assert_eq!(slow_result.counts.decision_steps, 5);
        assert_eq!(fast_result.counts.decision_steps, 5);
        assert_eq!(model_state_check.max_in_flight.load(Ordering::SeqCst), 2);
        let completed = model_state_check.completed.lock().unwrap().clone();
        let steps = completed
            .iter()
            .map(|(_, step)| step.clone())
            .collect::<std::collections::HashSet<_>>();
        for step in steps {
            let fast_index = completed
                .iter()
                .position(|(model, current_step)| model == "fast-model" && current_step == &step)
                .unwrap();
            let slow_index = completed
                .iter()
                .position(|(model, current_step)| model == "slow-model" && current_step == &step)
                .unwrap();
            assert!(
                fast_index < slow_index,
                "responses should complete out of order per step"
            );
        }
        let checked_artifact = slow_result.evidence.first().unwrap();
        let artifact_path = evidence_root
            .join("runs")
            .join(slow_ack.run_id.as_str())
            .join(&checked_artifact.relative_path);
        std::fs::write(&artifact_path, b"mutated evidence").unwrap();
        let corrupt_result = client
            .get(format!(
                "{api_base}/v1/runs/{}/results",
                slow_ack.run_id.as_str()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(corrupt_result.status(), StatusCode::INTERNAL_SERVER_ERROR);

        runtime.shutdown().await;
        api_server.abort();
        model_server.abort();
        std::fs::remove_dir_all(evidence_root).unwrap();
    }

    #[tokio::test]
    async fn loopback_api_exposes_failed_run_and_preserves_failure_evidence() {
        let model_router = HttpModelFixture::new(
            "scores",
            json!({"scores": []}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        )
        .with_raw_response_body("malformed controlled response")
        .router();
        let (model_base, model_server) = spawn_server(model_router).await;
        let evidence_root = unique_temp_path("api-failure-evidence");
        let runtime = BackendRuntime::start(&runtime_config(evidence_root.clone())).unwrap();
        let api_state = ApiState::new(runtime.store(), runtime.fixtures(), evidence_root.clone());
        let (api_base, api_server) =
            spawn_server(run_routes(api_state).layer(middleware::from_fn(request_id_middleware)))
                .await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{api_base}/v1/runs"))
            .header(IDEMPOTENCY_KEY, valid_key(41))
            .json(&config_for_endpoint(
                &format!("{model_base}/infer"),
                "failure-model",
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let accepted: SubmissionAccepted = response.json().await.unwrap();
        let status = await_terminal_status(&client, &api_base, &accepted.run_id).await;
        assert_eq!(status.status, RunLifecycle::Failed);
        let failure = status.failure.unwrap();
        assert_eq!(failure.code, "RUN_EXECUTION_FAILED");
        assert_eq!(failure.step_id.as_ref().unwrap().as_str(), "step-0001");
        let state_ref = failure.last_valid_state_ref.unwrap();
        assert!(evidence_root
            .join("runs")
            .join(accepted.run_id.as_str())
            .join(state_ref)
            .is_file());
        let result_response = client
            .get(format!(
                "{api_base}/v1/runs/{}/results",
                accepted.run_id.as_str()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(result_response.status(), StatusCode::CONFLICT);
        let body: Value = result_response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "RUN_FAILED");

        runtime.shutdown().await;
        api_server.abort();
        model_server.abort();
        std::fs::remove_dir_all(evidence_root).unwrap();
    }

    #[tokio::test]
    async fn research_api_seals_and_replays_job_without_mutating_source() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let evidence_root = std::env::temp_dir().join(format!(
            "factorsbt-research-api-{}-{nonce}",
            std::process::id()
        ));
        let mut config = BackendConfig::default();
        config.evidence_directory = evidence_root.clone();
        config.queue_limits = QueueLimits::new(1, 2).unwrap();
        let runtime = BackendRuntime::start(&config).unwrap();
        let router = run_routes(ApiState::new(
            runtime.store(),
            runtime.fixtures(),
            &evidence_root,
        ))
        .layer(middleware::from_fn(request_id_middleware));
        let source_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let episode = json!({"contract_version":"1.0.0","kind":"episode","fixture_id":"episode-eval","fixture_version":"1.0.0","actions":["0.5","0.5"]});
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/research/jobs")
                    .header("content-type", "application/json")
                    .header(IDEMPOTENCY_KEY, source_id)
                    .body(Body::from(serde_json::to_vec(&episode).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        for _ in 0..100 {
            if runtime
                .store()
                .get_research_job(&Uuid::new(source_id).unwrap())
                .is_some_and(|job| job.status.status == "succeeded")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let source = runtime
            .store()
            .get_research_job(&Uuid::new(source_id).unwrap())
            .unwrap();
        assert_eq!(source.status.status, "succeeded");
        let manifest_path = evidence_root
            .join("research_jobs")
            .join(source_id)
            .join("manifest.json");
        let source_manifest = std::fs::read(&manifest_path).unwrap();
        let replay_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let replay = json!({"contract_version":"1.0.0","kind":"replay","source_job_id":source_id});
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/research/jobs")
                    .header("content-type", "application/json")
                    .header(IDEMPOTENCY_KEY, replay_id)
                    .body(Body::from(serde_json::to_vec(&replay).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        for _ in 0..100 {
            if runtime
                .store()
                .get_research_job(&Uuid::new(replay_id).unwrap())
                .is_some_and(|job| matches!(job.status.status.as_str(), "succeeded" | "failed"))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/research/jobs/{replay_id}/result"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["report"]["matched"], true);
        assert_eq!(std::fs::read(manifest_path).unwrap(), source_manifest);
        runtime.shutdown().await;
        std::fs::remove_dir_all(evidence_root).unwrap();
    }
}
