use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/system/health", get(health_check))
        .route("/api/v1/system/metrics", get(metrics))
}

#[utoipa::path(
    get,
    path = "/api/v1/system/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/system/metrics",
    responses(
        (status = 200, description = "Prometheus metrics")
    )
)]
pub async fn metrics() -> String {
    // Placeholder: implement prometheus-client metrics collection
    "# LingShu Metrics\n# TODO: implement prometheus metrics\n".to_string()
}
