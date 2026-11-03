//! Loopback service entry point. The run operations are registered in Stage 06.

use axum::{routing::get, Json, Router};
use factorsbt_backend::{backend_config::BackendConfig, runtime::BackendRuntime};
use factorsbt_contracts::outcomes::{BackendService, HealthResponse, HealthStatus};
use std::error::Error;
use tokio::net::TcpListener;

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: HealthStatus::Ok,
        service: BackendService::FactorsbtBackend,
        contract_version: "1.0.0".to_owned(),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = BackendConfig::from_env()?;
    let listener = TcpListener::bind(config.bind_address).await?;
    let runtime = BackendRuntime::start(&config)?;
    let app = Router::new().route("/healthz", get(healthz));

    println!("FactorsBT backend listening on {}", config.bind_address);
    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
    runtime.shutdown().await;
    serve_result?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("failed to listen for shutdown signal: {error}");
    }
}
