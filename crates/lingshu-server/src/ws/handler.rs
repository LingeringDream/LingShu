use axum::extract::ws::{Message, WebSocket};

/// Subscribe the pet-window WebSocket to the broadcast channel.
/// Every [`crate::state::PetNotification`] sent via `AppState.pet_notifications`
/// is forwarded to the connected client as a JSON text frame.
pub async fn handle_socket(mut socket: WebSocket, state: crate::state::AppState) {
    let mut rx = state.pet_notifications.subscribe();

    // Send a welcome message so the frontend knows the connection is live.
    let welcome = serde_json::to_string(&crate::state::PetNotification::new(
        "connected",
        "灵枢",
        "桌面宠物已连接",
    ))
    .unwrap_or_default();
    if socket.send(Message::Text(welcome)).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            // Broadcast notification → forward to client
            result = rx.recv() => {
                match result {
                    Ok(notification) => {
                        let json = serde_json::to_string(&notification).unwrap_or_default();
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(n, "Pet WS client lagging, dropped messages");
                        // Re-subscribe to get fresh messages
                        rx = state.pet_notifications.subscribe();
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            // Client message → ignore (pet window doesn't send data, only receives)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        tracing::warn!("Pet WS error: {e}");
                        break;
                    }
                    _ => {} // ignore text/binary from client
                }
            }
        }
    }
}
