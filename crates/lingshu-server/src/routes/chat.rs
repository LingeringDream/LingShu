use axum::{
    response::sse::{Event, Sse},
    routing::post,
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/chat", post(chat))
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ChatChunk {
    pub content: String,
    pub done: bool,
}

pub async fn chat(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Phase 0: echo back the message with a prefix
    let response = format!("灵枢收到: {}", req.message);

    let stream = futures::stream::once(async move {
        Ok(Event::default().data(
            serde_json::to_string(&ChatChunk {
                content: response,
                done: true,
            })
            .unwrap_or_default(),
        ))
    });

    Ok(Sse::new(stream))
}
