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
#[cfg(test)]
mod tests_integration;
mod ws;

use axum::http::{HeaderValue, Method};
use axum::Router;
#[cfg(unix)]
use std::ffi::OsStr;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::state::AppState;

#[cfg(unix)]
const EXIT_WITH_PARENT_ENV: &str = "LINGSHU_EXIT_WITH_PARENT";
#[cfg(unix)]
const EXIT_WITH_PARENT_VALUE: &str = "1";
const LOCAL_FRONTEND_ORIGIN: &str = "http://localhost:5173";

/// When launched as the Tauri desktop sidecar (the spawner sets
/// `LINGSHU_EXIT_WITH_PARENT=1`), terminate ourselves as soon as that parent
/// process dies. The desktop app only kills the sidecar on a *graceful* exit;
/// an abrupt one (Ctrl+C on `tauri dev`, a rebuild-triggered restart, or a
/// crash) skips that cleanup and orphans us still holding :8080, which then
/// blocks the next launch ("address already in use"). macOS reparents orphans
/// to launchd, so a changed `getppid()` means the spawner is gone — release the
/// port and quit. No-op when the env var is unset (standalone `cargo run`,
/// Docker), so a manually-run backend is never affected.
#[cfg(unix)]
fn exit_when_parent_dies() {
    let requested = std::env::var_os(EXIT_WITH_PARENT_ENV);
    if !parent_watch_requested(requested.as_deref()) {
        return;
    }
    let original_ppid = unsafe { libc::getppid() };
    if !parent_watch_can_start(original_ppid) {
        return;
    }
    std::thread::Builder::new()
        .name("parent-watch".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let current_ppid = unsafe { libc::getppid() };
            if parent_has_died(original_ppid, current_ppid) {
                // Parent gone; free :8080 immediately for the next launch.
                std::process::exit(0);
            }
        })
        .ok();
}

#[cfg(unix)]
fn parent_watch_requested(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value == EXIT_WITH_PARENT_VALUE)
}

#[cfg(unix)]
fn parent_has_died(original_ppid: libc::pid_t, current_ppid: libc::pid_t) -> bool {
    parent_watch_can_start(original_ppid) && current_ppid != original_ppid
}

#[cfg(unix)]
fn parent_watch_can_start(original_ppid: libc::pid_t) -> bool {
    original_ppid > 1
}

#[cfg(not(unix))]
fn exit_when_parent_dies() {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Must run before dotenv so a local .env file cannot accidentally opt a
    // standalone backend into sidecar lifecycle behavior.
    exit_when_parent_dies();

    // Load .env file (silently skip if missing)
    let _ = dotenvy::dotenv();

    // Dev tool: dump the OpenAPI spec to stdout and exit (no DB needed).
    // Regenerate the committed contract file with:
    //   cargo run -p lingshu-server -- --dump-openapi > openapi.json
    if std::env::args().any(|a| a == "--dump-openapi") {
        println!("{}", serde_json::to_string(&routes::openapi_spec())?);
        return Ok(());
    }

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
    #[cfg(unix)]
    use std::ffi::OsStr;

    #[cfg(unix)]
    #[test]
    fn parent_watch_env_requires_exact_opt_in_value() {
        assert!(!parent_watch_requested(None));
        assert!(!parent_watch_requested(Some(OsStr::new(""))));
        assert!(!parent_watch_requested(Some(OsStr::new("0"))));
        assert!(parent_watch_requested(Some(OsStr::new("1"))));
    }

    #[cfg(unix)]
    #[test]
    fn parent_watch_exits_only_after_original_parent_changes() {
        assert!(!parent_has_died(1, 1));
        assert!(!parent_has_died(1, 99));
        assert!(!parent_has_died(42, 42));
        assert!(parent_has_died(42, 1));
        assert!(parent_has_died(42, 99));
    }

    #[cfg(unix)]
    #[test]
    fn parent_watch_starts_only_with_real_parent() {
        assert!(!parent_watch_can_start(0));
        assert!(!parent_watch_can_start(1));
        assert!(parent_watch_can_start(2));
    }

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

    /// The default origins must include the bundled Tauri webview origins,
    /// otherwise the packaged macOS/Windows app cannot reach the local backend
    /// (CORS blocks the local-session POST → "本地控制台启动失败"). Every entry
    /// must also parse as a valid `HeaderValue`, or `allowed_cors_origins`
    /// silently drops it.
    #[test]
    fn default_cors_origins_include_tauri_webview_origins() {
        let config = crate::config::CorsConfig::default();
        let origins = allowed_cors_origins(&config);

        // No entry was silently dropped by the filter_map parse.
        assert_eq!(origins.len(), config.allowed_origins.len());
        assert!(origins.contains(&HeaderValue::from_static("tauri://localhost")));
        assert!(origins.contains(&HeaderValue::from_static("http://tauri.localhost")));
    }
}
