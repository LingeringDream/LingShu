use axum::{extract::Path, routing::get, Json, Router};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/chat/sessions", get(list_sessions))
        .route("/api/v1/chat/sessions/{id}", get(get_session).delete(delete_session))
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: Uuid,
    pub title: Option<String>,
    pub message_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_sessions(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<SessionResponse>>, AppError> {
    let user_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_one(&state.db)
    .await?;

    let sessions = sqlx::query_as::<_, (Uuid, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, title, created_at FROM conversations WHERE user_id = $1 AND deleted_at IS NULL ORDER BY updated_at DESC"
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        sessions
            .into_iter()
            .map(|s| SessionResponse {
                id: s.0,
                title: s.1,
                message_count: 0, // TODO: count messages
                created_at: s.2,
            })
            .collect(),
    ))
}

pub async fn get_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionResponse>, AppError> {
    let session = sqlx::query_as::<_, (Uuid, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, title, created_at FROM conversations WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse {
        id: session.0,
        title: session.1,
        message_count: 0,
        created_at: session.2,
    }))
}

pub async fn delete_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let result = sqlx::query(
        "UPDATE conversations SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Session not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
