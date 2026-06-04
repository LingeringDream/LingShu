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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_response_has_required_fields() {
        let resp = HealthResponse {
            status: "ok".to_string(),
            version: "0.1.0".to_string(),
            uptime_seconds: 42,
        };
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.version, "0.1.0");
        assert_eq!(resp.uptime_seconds, 42);
    }

    #[test]
    fn metrics_returns_placeholder() {
        let output = tokio_test::block_on(async {
            // This handler doesn't need state, but axum handlers return impl IntoResponse
            // Just verify it returns a non-empty string
            let body = metrics().await;
            assert!(body.contains("LingShu Metrics"));
            body
        });
        assert!(!output.is_empty());
    }
}
