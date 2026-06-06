// Phase 0: allow dead code for future-use fields, scaffolding functions,
// and temporarily unused error variants — these will be wired in Phase 1+.
#![allow(dead_code)]

mod auth;
mod cache;
mod config;
mod crypto;
mod db;
mod error;
mod llm;
mod models;
mod patch;
mod routes;
mod state;
mod telemetry;
mod ws;

use axum::http::{HeaderValue, Method};
use axum::Router;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::state::AppState;

const LOCAL_FRONTEND_ORIGIN: &str = "http://localhost:5173";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file (silently skip if missing)
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,lingshu_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = AppConfig::load()?;
    tracing::info!(
        "Starting LingShu server on {}:{}",
        config.server.host,
        config.server.port
    );

    // Build application state
    let state = AppState::new(&config).await?;

    // Build CORS layer from config (defaults to localhost origins)
    let cors = CorsLayer::new()
        .allow_origin(allowed_cors_origins(&config.cors))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    // Build router
    let app = Router::new()
        .merge(routes::system::router())
        .merge(ws::router())
        .merge(routes::auth::router())
        .merge(routes::users::router())
        .merge(routes::settings::router())
        .merge(routes::calendar::router())
        .merge(routes::permissions::router())
        .merge(routes::projects::router())
        .merge(routes::tasks::router())
        .merge(routes::conversations::router())
        .merge(routes::sessions::router())
        .merge(routes::chat::router())
        .merge(routes::memories::router())
        .merge(routes::project_members::router())
        .merge(routes::task_dependencies::router())
        .merge(routes::personality::router())
        .merge(routes::thoughts::router())
        .merge(routes::integrations::router())
        .merge(routes::signals::router())
        .merge(routes::audit::router())
        .merge(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", routes::openapi_spec()),
        )
        .layer(cors)
        .with_state(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

fn allowed_cors_origins(config: &crate::config::CorsConfig) -> Vec<HeaderValue> {
    if config.allowed_origins.iter().any(|origin| origin == "*") {
        return vec![HeaderValue::from_static(LOCAL_FRONTEND_ORIGIN)];
    }

    config
        .allowed_origins
        .iter()
        .filter_map(|origin| origin.parse::<HeaderValue>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_cors_origin_falls_back_to_localhost_frontend() {
        let config = crate::config::CorsConfig {
            allowed_origins: vec!["*".to_string()],
        };

        assert_eq!(
            allowed_cors_origins(&config),
            vec![HeaderValue::from_static("http://localhost:5173")]
        );
    }
}
