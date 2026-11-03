//! Async HTTP adapter for the approved loopback model-service boundary.

use factorsbt_contracts::model::{ModelRequest, ModelResponse};
use reqwest::{redirect::Policy, Client as HttpClient, StatusCode, Url};
use std::{error::Error, fmt, net::IpAddr, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelServiceErrorKind {
    InvalidEndpoint,
    Timeout,
    Transport,
    HttpStatus,
    InvalidResponse,
}

#[derive(Debug)]
pub struct ModelServiceError {
    kind: ModelServiceErrorKind,
    status: Option<StatusCode>,
    response_body: Option<Vec<u8>>,
}

impl ModelServiceError {
    fn new(kind: ModelServiceErrorKind, status: Option<StatusCode>) -> Self {
        Self {
            kind,
            status,
            response_body: None,
        }
    }

    fn with_response_body(
        kind: ModelServiceErrorKind,
        status: Option<StatusCode>,
        response_body: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            status,
            response_body: Some(response_body),
        }
    }

    pub fn kind(&self) -> &ModelServiceErrorKind {
        &self.kind
    }

    pub fn status(&self) -> Option<StatusCode> {
        self.status
    }

    pub fn response_body(&self) -> Option<&[u8]> {
        self.response_body.as_deref()
    }
}

impl fmt::Display for ModelServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.kind, self.status) {
            (&ModelServiceErrorKind::HttpStatus, Some(status)) => {
                write!(formatter, "model service returned HTTP {status}")
            }
            (kind, _) => write!(formatter, "model service request failed: {kind:?}"),
        }
    }
}

impl Error for ModelServiceError {}

#[derive(Debug)]
pub struct ModelExchange {
    pub response: ModelResponse,
    pub response_body: Vec<u8>,
}

#[derive(Clone)]
pub struct ModelHttpClient {
    client: HttpClient,
}

impl ModelHttpClient {
    pub fn new() -> Result<Self, ModelServiceError> {
        let client = HttpClient::builder()
            .redirect(Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| ModelServiceError::new(ModelServiceErrorKind::Transport, None))?;
        Ok(Self { client })
    }

    /// Send exactly one request. Redirects and automatic retries are disabled;
    /// timeout applies to this inference only and never advances simulation time.
    pub async fn infer(
        &self,
        endpoint: &str,
        timeout: Duration,
        request: &ModelRequest,
    ) -> Result<ModelExchange, ModelServiceError> {
        let endpoint = parse_loopback_endpoint(endpoint)?;
        let response = self
            .client
            .post(endpoint)
            .timeout(timeout)
            .json(request)
            .send()
            .await
            .map_err(|error| {
                ModelServiceError::new(
                    if error.is_timeout() {
                        ModelServiceErrorKind::Timeout
                    } else {
                        ModelServiceErrorKind::Transport
                    },
                    None,
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.bytes().await.map_err(|error| {
                ModelServiceError::new(
                    if error.is_timeout() {
                        ModelServiceErrorKind::Timeout
                    } else {
                        ModelServiceErrorKind::Transport
                    },
                    Some(status),
                )
            })?;
            return Err(ModelServiceError::with_response_body(
                ModelServiceErrorKind::HttpStatus,
                Some(status),
                body.to_vec(),
            ));
        }
        let response_body = response.bytes().await.map_err(|error| {
            ModelServiceError::new(
                if error.is_timeout() {
                    ModelServiceErrorKind::Timeout
                } else {
                    ModelServiceErrorKind::Transport
                },
                None,
            )
        })?;
        let response = serde_json::from_slice(&response_body).map_err(|_| {
            ModelServiceError::with_response_body(
                ModelServiceErrorKind::InvalidResponse,
                Some(status),
                response_body.to_vec(),
            )
        })?;
        Ok(ModelExchange {
            response,
            response_body: response_body.to_vec(),
        })
    }
}

fn parse_loopback_endpoint(value: &str) -> Result<Url, ModelServiceError> {
    let endpoint = Url::parse(value)
        .map_err(|_| ModelServiceError::new(ModelServiceErrorKind::InvalidEndpoint, None))?;
    let host_is_loopback = endpoint.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    let has_http_scheme = endpoint.scheme() == "http";
    let has_no_user_info = endpoint.username().is_empty() && endpoint.password().is_none();
    if !has_http_scheme || !host_is_loopback || !has_no_user_info {
        return Err(ModelServiceError::new(
            ModelServiceErrorKind::InvalidEndpoint,
            None,
        ));
    }
    Ok(endpoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_fixture::HttpModelFixture;
    use axum::{http::StatusCode, routing::post, Router};
    use serde_json::json;
    use std::{
        net::SocketAddr,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    use tokio::net::TcpListener;

    async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{address}/infer"), task)
    }

    fn request() -> ModelRequest {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
        ))
        .unwrap()
    }

    fn fixture() -> HttpModelFixture {
        HttpModelFixture::new(
            "scores",
            json!({"scores":[{"instrument_id":"SPOT_A","score":"1"}]}),
            factorsbt_contracts::UtcTimestamp::new("2026-01-05T16:00:02Z").unwrap(),
        )
    }

    #[tokio::test]
    async fn posts_once_to_loopback_fixture_and_retains_response_bytes() {
        let (endpoint, server) = serve(fixture().router()).await;
        let exchange = ModelHttpClient::new()
            .unwrap()
            .infer(&endpoint, Duration::from_secs(1), &request())
            .await
            .unwrap();
        assert!(!exchange.response_body.is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn timeout_and_http_errors_are_typed_without_retrying() {
        let delayed = fixture().with_delay(Duration::from_millis(100)).router();
        let (endpoint, server) = serve(delayed).await;
        let error = ModelHttpClient::new()
            .unwrap()
            .infer(&endpoint, Duration::from_millis(10), &request())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), &ModelServiceErrorKind::Timeout);
        server.abort();

        let attempts = Arc::new(AtomicUsize::new(0));
        let handler_attempts = Arc::clone(&attempts);
        let unavailable = Router::new().route(
            "/infer",
            post(move || {
                let attempts = Arc::clone(&handler_attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::SERVICE_UNAVAILABLE, "controlled failure")
                }
            }),
        );
        let (endpoint, server) = serve(unavailable).await;
        let error = ModelHttpClient::new()
            .unwrap()
            .infer(&endpoint, Duration::from_secs(1), &request())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), &ModelServiceErrorKind::HttpStatus);
        assert_eq!(error.status(), Some(StatusCode::SERVICE_UNAVAILABLE));
        assert_eq!(
            error.response_body(),
            Some(b"controlled failure".as_slice())
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn malformed_success_response_retains_exact_bytes_for_failure_evidence() {
        let router = Router::new().route(
            "/infer",
            post(|| async { (StatusCode::OK, "{malformed-json") }),
        );
        let (endpoint, server) = serve(router).await;
        let error = ModelHttpClient::new()
            .unwrap()
            .infer(&endpoint, Duration::from_secs(1), &request())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), &ModelServiceErrorKind::InvalidResponse);
        assert_eq!(error.response_body(), Some(b"{malformed-json".as_slice()));
        server.abort();
    }

    #[tokio::test]
    async fn redirect_responses_are_returned_as_http_failures_without_following() {
        let router = Router::new()
            .route(
                "/infer",
                post(|| async { (StatusCode::OK, "unexpected follow") }),
            )
            .route(
                "/redirect",
                post(|| async {
                    (
                        StatusCode::TEMPORARY_REDIRECT,
                        [(axum::http::header::LOCATION, "/infer")],
                    )
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let error = ModelHttpClient::new()
            .unwrap()
            .infer(
                &format!("http://{address}/redirect"),
                Duration::from_secs(1),
                &request(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind(), &ModelServiceErrorKind::HttpStatus);
        assert_eq!(error.status(), Some(StatusCode::TEMPORARY_REDIRECT));
        server.abort();
    }

    #[tokio::test]
    async fn rejects_non_loopback_and_non_http_endpoints_before_sending() {
        let client = ModelHttpClient::new().unwrap();
        for endpoint in ["https://localhost/infer", "http://example.com/infer"] {
            let error = client
                .infer(endpoint, Duration::from_secs(1), &request())
                .await
                .unwrap_err();
            assert_eq!(error.kind(), &ModelServiceErrorKind::InvalidEndpoint);
        }
    }
}
