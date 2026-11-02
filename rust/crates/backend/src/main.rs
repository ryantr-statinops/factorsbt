//! Local HTTP boundary for the Phase A health operation.

use axum::{routing::get, Json, Router};
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
    let bind_address =
        std::env::var("FACTORSBT_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let listener = TcpListener::bind(&bind_address).await?;
    let app = Router::new().route("/healthz", get(healthz));

    println!("FactorsBT backend listening on {bind_address}");
    axum::serve(listener, app).await?;
    Ok(())
}
