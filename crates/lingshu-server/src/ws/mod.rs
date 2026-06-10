pub mod handler;

use axum::{
    extract::ws::WebSocketUpgrade, extract::State, response::IntoResponse, routing::get, Router,
};

use crate::state::AppState;

/// Mount WebSocket routes.
/// Frontend proxies /ws → backend (see frontend/vite.config.ts).
pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(ws_upgrade))
}

/// Upgrade GET /ws to a WebSocket and hand off to the broadcast handler.
async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handler::handle_socket(socket, state))
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
