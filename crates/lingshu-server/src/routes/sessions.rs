use axum::{
    extract::Path,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/chat/sessions", get(list_sessions))
        .route(
            "/api/v1/chat/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route(
            "/api/v1/chat/sessions/:id/messages",
            get(list_session_messages),
        )
        .route(
            "/api/v1/chat/messages/:id",
            patch(update_user_message).delete(withdraw_user_message),
        )
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct SessionResponse {
    pub id: Uuid,
    pub title: Option<String>,
    pub message_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct StoredChatMessageResponse {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserMessageRequest {
    pub content: String,
}

/// Invalidate the cached session list for a user. Best-effort.
pub async fn invalidate_session_cache(state: &AppState, user_id: Uuid) {
    crate::cache::del(
        &state.redis,
        &crate::cache::chat_sessions_cache_key(user_id),
    )
    .await;
}

#[utoipa::path(
    get,
    path = "/api/v1/chat/sessions",
    responses(
        (status = 200, description = "List of chat sessions", body = Vec<SessionResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_sessions(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<SessionResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // 1. Redis cache
    let cache_key = crate::cache::chat_sessions_cache_key(user_id);
    if let Some(cached) =
        crate::cache::get_json::<Vec<SessionResponse>>(&state.redis, &cache_key).await
    {
        return Ok(Json(cached));
    }

    // 2. PostgreSQL with real message count
    let sessions: Vec<SessionResponse> = sqlx::query_as(
        "SELECT c.id, c.title, c.created_at, \
         CAST(COUNT(m.id) AS BIGINT) AS message_count \
         FROM conversations c \
         LEFT JOIN messages m ON m.conversation_id = c.id \
         WHERE c.user_id = $1 AND c.deleted_at IS NULL \
         GROUP BY c.id, c.title, c.created_at \
         ORDER BY MAX(c.updated_at) DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    // 3. Write-through to Redis (TTL 30s)
    crate::cache::set_json(&state.redis, &cache_key, &sessions, Some(30)).await;

    Ok(Json(sessions))
}

#[utoipa::path(
    get,
    path = "/api/v1/chat/sessions/{id}",
    responses(
        (status = 200, description = "Chat session detail", body = SessionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn get_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let session: SessionResponse = sqlx::query_as(
        "SELECT c.id, c.title, c.created_at, CAST(COUNT(m.id) AS BIGINT) AS message_count \
         FROM conversations c \
         LEFT JOIN messages m ON m.conversation_id = c.id \
         WHERE c.id = $1 AND c.user_id = $2 AND c.deleted_at IS NULL \
         GROUP BY c.id, c.title, c.created_at",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    Ok(Json(session))
}

#[utoipa::path(
    get,
    path = "/api/v1/chat/sessions/{id}/messages",
    params(("id" = Uuid, Path, description = "Chat session id")),
    responses(
        (status = 200, description = "Messages in the chat session", body = Vec<StoredChatMessageResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn list_session_messages(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<StoredChatMessageResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversations \
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL)",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound("Session not found".to_string()));
    }

    let messages = sqlx::query_as::<_, StoredChatMessageResponse>(
        "SELECT id, role, content, model_id, created_at \
         FROM messages WHERE conversation_id = $1 \
         ORDER BY created_at ASC, id ASC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(messages))
}

#[utoipa::path(
    patch,
    path = "/api/v1/chat/messages/{id}",
    params(("id" = Uuid, Path, description = "User message id")),
    request_body = UpdateUserMessageRequest,
    responses(
        (status = 200, description = "Updated user message", body = StoredChatMessageResponse),
        (status = 422, description = "Message content is empty"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User message not found")
    )
)]
pub async fn update_user_message(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserMessageRequest>,
) -> Result<Json<StoredChatMessageResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let content = req.content.trim();
    if content.is_empty() {
        return Err(AppError::Validation(
            "Message content cannot be empty".to_string(),
        ));
    }

    let updated = sqlx::query_as::<_, StoredChatMessageResponse>(
        "UPDATE messages AS m SET content = $1 \
         FROM conversations AS c \
         WHERE m.id = $2 \
           AND m.role = 'user' \
           AND c.id = m.conversation_id \
           AND c.user_id = $3 \
           AND c.deleted_at IS NULL \
         RETURNING m.id, m.role, m.content, m.model_id, m.created_at",
    )
    .bind(content)
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("User message not found".to_string()))?;

    sqlx::query(
        "UPDATE conversations SET updated_at = NOW() \
         WHERE id = (SELECT conversation_id FROM messages WHERE id = $1)",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    invalidate_session_cache(&state, user_id).await;

    Ok(Json(updated))
}

#[utoipa::path(
    delete,
    path = "/api/v1/chat/messages/{id}",
    params(("id" = Uuid, Path, description = "User message id")),
    responses(
        (status = 204, description = "User message withdrawn"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User message not found")
    )
)]
pub async fn withdraw_user_message(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;
    let conversation_id = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM messages AS m USING conversations AS c \
         WHERE m.id = $1 \
           AND m.role = 'user' \
           AND c.id = m.conversation_id \
           AND c.user_id = $2 \
           AND c.deleted_at IS NULL \
         RETURNING m.conversation_id",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("User message not found".to_string()))?;

    sqlx::query("UPDATE conversations SET updated_at = NOW() WHERE id = $1")
        .bind(conversation_id)
        .execute(&state.db)
        .await?;
    invalidate_session_cache(&state, user_id).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/v1/chat/sessions/{id}",
    responses(
        (status = 204, description = "Session deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn delete_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;

    let result = sqlx::query(
        "UPDATE conversations SET deleted_at = NOW() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Session not found".to_string()));
    }

    // Invalidate cached session list
    invalidate_session_cache(&state, user_id).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn session_response_serialization() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let now = Utc::now();
        let resp = SessionResponse {
            id,
            title: Some("Chat about calendar".into()),
            message_count: 42,
            created_at: now,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["message_count"], 42);
    }

    #[test]
    fn session_response_no_title() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let now = Utc::now();
        let resp = SessionResponse {
            id,
            title: None,
            message_count: 0,
            created_at: now,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["title"].is_null());
    }

    #[test]
    fn update_user_message_request_deserialization() {
        let req: UpdateUserMessageRequest =
            serde_json::from_value(serde_json::json!({"content": "修正后的内容"})).unwrap();
        assert_eq!(req.content, "修正后的内容");
    }
}
