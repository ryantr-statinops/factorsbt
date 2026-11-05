use axum::{body::{to_bytes, Body}, middleware, http::{Request, StatusCode}, Router};
use factorsbt_backend::{
    api::{run_routes, ApiState}, api_errors::request_id_middleware,
    backend_config::{BackendConfig, QueueLimits}, runtime::BackendRuntime,
};
use factorsbt_contracts::Uuid;
use serde_json::{json, Value};
use std::{fs, time::{Duration, SystemTime, UNIX_EPOCH}};
use tower::ServiceExt;

fn app(runtime: &BackendRuntime, evidence_root: &std::path::Path) -> Router {
    run_routes(ApiState::new(runtime.store(), runtime.fixtures(), evidence_root))
        .layer(middleware::from_fn(request_id_middleware))
}

fn request(method: &str, uri: &str, key: Option<&str>, value: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri).header("accept", "application/json");
    if let Some(key) = key { builder = builder.header("idempotency-key", key); }
    if value.is_some() { builder = builder.header("content-type", "application/json"); }
    builder.body(value.map(|body| Body::from(serde_json::to_vec(&body).unwrap())).unwrap_or_else(Body::empty)).unwrap()
}

async fn read_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn wait_for_terminal(runtime: &BackendRuntime, raw_id: &str) {
    let id = Uuid::new(raw_id).unwrap();
    for _ in 0..100 {
        if runtime.store().get_research_job(&id).is_some_and(|job| matches!(job.status.status.as_str(), "succeeded" | "failed")) { return; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("research job did not reach a terminal state");
}

#[tokio::test]
async fn python_contract_routes_execute_jobs_seal_evidence_and_replay_without_mutating_source() {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let evidence_root = std::env::temp_dir().join(format!("factorsbt-research-api-{}-{nonce}", std::process::id()));
    let mut config = BackendConfig::default();
    config.evidence_directory = evidence_root.clone();
    config.queue_limits = QueueLimits::new(1, 2).unwrap();
    let runtime = BackendRuntime::start(&config).unwrap();
    let app = app(&runtime, &evidence_root);

    let source_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let episode = json!({"contract_version":"1.0.0","kind":"episode","fixture_id":"episode-eval","fixture_version":"1.0.0","actions":["0.5","0.5"]});
    let (status, accepted) = read_json(app.clone().oneshot(request("POST", "/v1/research/jobs", Some(source_id), Some(episode.clone()))).await.unwrap()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(accepted["job_id"], source_id);
    wait_for_terminal(&runtime, source_id).await;
    let record = runtime.store().get_research_job(&Uuid::new(source_id).unwrap()).unwrap();
    assert_eq!(record.status.status, "succeeded");
    let source_bytes = fs::read(evidence_root.join("research_jobs").join(source_id).join("manifest.json")).unwrap();

    let (status, _) = read_json(app.clone().oneshot(request("POST", "/v1/research/jobs", Some(source_id), Some(episode))).await.unwrap()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let changed = json!({"contract_version":"1.0.0","kind":"episode","fixture_id":"episode-eval","fixture_version":"1.0.0","actions":["0","0"]});
    let (status, _) = read_json(app.clone().oneshot(request("POST", "/v1/research/jobs", Some(source_id), Some(changed))).await.unwrap()).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let replay_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let replay = json!({"contract_version":"1.0.0","kind":"replay","source_job_id":source_id});
    let (status, _) = read_json(app.clone().oneshot(request("POST", "/v1/research/jobs", Some(replay_id), Some(replay))).await.unwrap()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    wait_for_terminal(&runtime, replay_id).await;
    let (status, result) = read_json(app.clone().oneshot(request("GET", &format!("/v1/research/jobs/{replay_id}/result"), None, None)).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["report"]["matched"], true);
    assert_eq!(fs::read(evidence_root.join("research_jobs").join(source_id).join("manifest.json")).unwrap(), source_bytes);

    runtime.shutdown().await;
    let _ = fs::remove_dir_all(evidence_root);
}
