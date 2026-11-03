//! Configurable, stateless loopback HTTP model fixture for adapter tests.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use factorsbt_contracts::{model::ModelRequest, UtcTimestamp};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpModelFixture {
    response_type: String,
    outputs: Value,
    responded_at: UtcTimestamp,
    status: StatusCode,
    delay: Duration,
    response_body_override: Option<String>,
}

impl HttpModelFixture {
    pub fn new(
        response_type: impl Into<String>,
        outputs: Value,
        responded_at: UtcTimestamp,
    ) -> Self {
        Self {
            response_type: response_type.into(),
            outputs,
            responded_at,
            status: StatusCode::OK,
            delay: Duration::ZERO,
            response_body_override: None,
        }
    }

    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Replace JSON serialization with an exact body for malformed-response tests.
    pub fn with_raw_response_body(mut self, body: impl Into<String>) -> Self {
        self.response_body_override = Some(body.into());
        self
    }

    pub fn router(self) -> Router {
        Router::new().route("/infer", post(infer)).with_state(self)
    }

    pub fn response_document(&self, request: &ModelRequest) -> Value {
        json!({
            "contract_version": request.contract_version,
            "inference_id": request.inference_id,
            "run_id": request.run_id,
            "scenario_id": request.scenario_id,
            "step_id": request.step_id,
            "snapshot_id": request.snapshot_id,
            "model_id": request.model_id,
            "model_version": request.model_version,
            "responded_at": self.responded_at,
            "response_type": self.response_type,
            "outputs": self.outputs,
        })
    }
}

async fn infer(
    State(fixture): State<HttpModelFixture>,
    Json(request): Json<ModelRequest>,
) -> Response {
    if !fixture.delay.is_zero() {
        tokio::time::sleep(fixture.delay).await;
    }
    if let Some(body) = fixture.response_body_override {
        return (fixture.status, body).into_response();
    }
    if !fixture.status.is_success() {
        return (
            fixture.status,
            Json(json!({"error":"controlled model fixture response"})),
        )
            .into_response();
    }
    (fixture.status, Json(fixture.response_document(&request))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use factorsbt_contracts::model::ModelResponse;
    use tower::ServiceExt;

    fn request() -> ModelRequest {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
        ))
        .unwrap()
    }

    #[test]
    fn fixture_echoes_all_request_correlation_and_keeps_outputs_stateless() {
        let request = request();
        let fixture = HttpModelFixture::new(
            "scores",
            json!({"scores":[{"instrument_id":"SPOT_A","score":"1"}]}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        );
        let first = fixture.response_document(&request);
        let second = fixture.response_document(&request);
        assert_eq!(first, second);
        assert_eq!(first["run_id"], request.run_id.as_str());
        assert_eq!(first["scenario_id"], request.scenario_id.as_str());
        assert_eq!(first["step_id"], request.step_id.as_str());
        assert_eq!(first["snapshot_id"], request.snapshot_id.as_str());
        assert_eq!(first["inference_id"], request.inference_id.as_str());
        assert!(matches!(
            serde_json::from_value::<ModelResponse>(first),
            Ok(ModelResponse::Scores { .. })
        ));
    }

    #[test]
    fn fixture_can_produce_controlled_non_success_and_malformed_bodies() {
        let unavailable = HttpModelFixture::new(
            "scores",
            json!({"scores":[]}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        )
        .with_status(StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(unavailable.status, StatusCode::SERVICE_UNAVAILABLE);
        let malformed = HttpModelFixture::new(
            "scores",
            json!({}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        )
        .with_raw_response_body("not-json");
        assert_eq!(
            malformed.response_body_override.as_deref(),
            Some("not-json")
        );
    }

    #[tokio::test]
    async fn loopback_router_serves_correlated_responses_and_controlled_failures() {
        let request = request();
        let success = HttpModelFixture::new(
            "scores",
            json!({"scores":[{"instrument_id":"SPOT_A","score":"1"}]}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        );
        let response = success
            .router()
            .oneshot(json_request(&request))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let decoded: ModelResponse = serde_json::from_slice(&bytes).unwrap();
        match decoded {
            ModelResponse::Scores {
                run_id,
                snapshot_id,
                ..
            } => {
                assert_eq!(run_id.as_str(), request.run_id.as_str());
                assert_eq!(snapshot_id.as_str(), request.snapshot_id.as_str());
            }
            _ => panic!("expected scores response"),
        }

        let unavailable = HttpModelFixture::new(
            "scores",
            json!({"scores":[]}),
            UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        )
        .with_status(StatusCode::SERVICE_UNAVAILABLE);
        let response = unavailable
            .router()
            .oneshot(json_request(&request))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    fn json_request(request: &ModelRequest) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/infer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(request).unwrap()))
            .unwrap()
    }
}
