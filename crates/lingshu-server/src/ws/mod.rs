pub mod handler;

use axum::{extract::ws::WebSocketUpgrade, response::IntoResponse, routing::get, Router};

use crate::state::AppState;

/// Mount WebSocket routes.
/// Frontend proxies /ws → backend (see frontend/vite.config.ts).
pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(ws_upgrade))
}

/// Upgrade GET /ws to a WebSocket and hand off to the echo handler.
async fn ws_upgrade(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handler::handle_socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_router_mounts_route() {
        let r = router();
        let repr = format!("{r:?}");
        assert!(
            repr.contains("/ws"),
            "router should mount /ws, debug: {repr}"
        );
    }
}
