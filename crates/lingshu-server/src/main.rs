mod config;
mod db;
mod error;
mod llm;
mod models;
mod routes;
mod state;
mod ws;

use axum::Router;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,lingshu_server=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = AppConfig::load()?;
    tracing::info!("Starting LingShu server on {}:{}", config.server.host, config.server.port);

    // Build application state
    let state = AppState::new(&config).await?;

    // Build router
    let app = Router::new()
        .merge(routes::system::router())
        .merge(routes::auth::router())
        .merge(routes::users::router())
        .merge(routes::projects::router())
        .merge(routes::tasks::router())
        .merge(routes::conversations::router())
        .merge(routes::chat::router())
        .merge(routes::memories::router())
        .merge(utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", routes::openapi_spec()))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
