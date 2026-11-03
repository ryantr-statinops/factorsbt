//! Versioned API error envelopes and per-request correlation IDs.

use crate::ids::new_uuid_v4;
use axum::{
    body::Body,
    extract::{FromRequest, FromRequestParts, Path, Request},
    http::{header::HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use factorsbt_contracts::outcomes::{ApiError, ApiErrorResponse, ValidationDetail};
use serde::de::DeserializeOwned;
use std::{error::Error, fmt};

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    pub request_id: factorsbt_contracts::Uuid,
}

impl RequestContext {
    pub fn new() -> Self {
        Self {
            request_id: new_uuid_v4(),
        }
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ApiResponseError {
    context: RequestContext,
    status: StatusCode,
    code: String,
    message: String,
    details: Option<Vec<ValidationDetail>>,
}

impl ApiResponseError {
    pub fn new(
        context: &RequestContext,
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<Vec<ValidationDetail>>,
    ) -> Self {
        Self {
            context: context.clone(),
            status,
            code: code.into(),
            message: message.into(),
            details,
        }
    }

    pub fn bad_request(
        context: &RequestContext,
        code: &'static str,
        message: &'static str,
    ) -> Self {
        Self::new(context, StatusCode::BAD_REQUEST, code, message, None)
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl fmt::Display for ApiResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for ApiResponseError {}

impl IntoResponse for ApiResponseError {
    fn into_response(self) -> Response {
        let request_id = self.context.request_id.as_str();
        let header = HeaderValue::from_str(request_id)
            .expect("validated contract UUID is a valid HTTP header value");
        let body = ApiErrorResponse {
            error: ApiError {
                code: self.code,
                message: self.message,
                request_id: self.context.request_id,
                details: self.details,
            },
        };
        (self.status, [(X_REQUEST_ID, header)], Json(body)).into_response()
    }
}

/// Associate every operation with a request ID and expose the same value in a
/// response header, including framework-generated not-found responses.
pub async fn request_id_middleware(mut request: Request<Body>, next: Next) -> Response {
    let context = RequestContext::new();
    request.extensions_mut().insert(context.clone());
    let mut response = next.run(request).await;
    let header = HeaderValue::from_str(context.request_id.as_str())
        .expect("validated contract UUID is a valid HTTP header value");
    response.headers_mut().insert(X_REQUEST_ID, header);
    response
}

/// JSON extractor that keeps parsing failures in the accepted error envelope
/// and never copies request body text into a public response.
#[derive(Debug)]
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiResponseError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let context = request
            .extensions()
            .get::<RequestContext>()
            .cloned()
            .unwrap_or_default();
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) if rejection.status() == StatusCode::UNPROCESSABLE_ENTITY => {
                Err(ApiResponseError::new(
                    &context,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "SCHEMA_VALIDATION_FAILED",
                    "Request body does not match the declared JSON contract.",
                    None,
                ))
            }
            Err(_) => Err(ApiResponseError::bad_request(
                &context,
                "INVALID_JSON",
                "Request body must be valid JSON with an application/json content type.",
            )),
        }
    }
}

#[derive(Debug)]
pub struct ApiPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiResponseError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let context = parts
            .extensions
            .get::<RequestContext>()
            .cloned()
            .unwrap_or_default();
        match Path::<T>::from_request_parts(parts, state).await {
            Ok(Path(value)) => Ok(Self(value)),
            Err(_) => Err(ApiResponseError::bad_request(
                &context,
                "INVALID_RESOURCE_ID",
                "A path resource identifier is malformed.",
            )),
        }
    }
}

pub async fn extract_json<S, T>(request: Request, state: &S) -> Result<ApiJson<T>, ApiResponseError>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    ApiJson::<T>::from_request(request, state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::to_bytes,
        http::{header::CONTENT_TYPE, Request as HttpRequest},
        middleware, Router,
    };
    use serde_json::Value;
    use tower::ServiceExt;

    #[tokio::test]
    async fn error_body_uuid_matches_the_request_id_header() {
        let context = RequestContext {
            request_id: factorsbt_contracts::Uuid::new("f189d119-3f26-4da9-a0a4-04a929bdb2f6")
                .unwrap(),
        };
        let response = ApiResponseError::new(
            &context,
            StatusCode::CONFLICT,
            "SUBMISSION_ID_REUSED",
            "This submission ID is already bound to a different configuration.",
            None,
        )
        .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.headers()[X_REQUEST_ID],
            context.request_id.as_str()
        );
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(body["error"]["request_id"], context.request_id.as_str());
    }

    #[tokio::test]
    async fn malformed_json_uses_safe_envelope_without_echoing_private_body() {
        let context = RequestContext::new();
        let mut request = HttpRequest::builder()
            .method("POST")
            .uri("/v1/runs")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"api_key":"synthetic-secret-value""#))
            .unwrap();
        request.extensions_mut().insert(context.clone());
        let response = extract_json::<(), Value>(request, &()).await.unwrap_err();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = response.into_response();
        assert_eq!(
            response.headers()[X_REQUEST_ID],
            context.request_id.as_str()
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("INVALID_JSON"));
        assert!(!text.contains("synthetic-secret-value"));
    }

    async fn path_resource(ApiPath(_): ApiPath<factorsbt_contracts::Uuid>) -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn malformed_resource_path_uses_the_same_error_envelope_and_request_id() {
        let response = Router::new()
            .route("/{resource_id}", axum::routing::get(path_resource))
            .layer(middleware::from_fn(request_id_middleware))
            .oneshot(
                HttpRequest::builder()
                    .uri("/not-a-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let header_id = response.headers()[X_REQUEST_ID]
            .to_str()
            .unwrap()
            .to_owned();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(body["error"]["code"], "INVALID_RESOURCE_ID");
        assert_eq!(body["error"]["request_id"], header_id);
    }
}
