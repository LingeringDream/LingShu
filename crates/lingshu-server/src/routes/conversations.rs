use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/api/v1/conversations/:id",
            get(get_conversation).delete(delete_conversation),
        )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateConversationRequest {
    pub title: Option<String>,
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub title: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Row helper ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct ConversationRow {
    id: Uuid,
    user_id: Uuid,
    project_id: Option<Uuid>,
    title: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl ConversationRow {
    fn into_response(self) -> ConversationResponse {
        ConversationResponse {
            id: self.id,
            user_id: self.user_id,
            project_id: self.project_id,
            title: self.title,
            created_at: self.created_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/conversations",
    responses(
        (status = 200, description = "List of conversations", body = Vec<ConversationResponse>)
    )
)]
pub async fn list_conversations(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<ConversationResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let convs = sqlx::query_as::<_, ConversationRow>(
        "SELECT id, user_id, project_id, title, created_at \
         FROM conversations WHERE user_id = $1 AND deleted_at IS NULL ORDER BY updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        convs
            .into_iter()
            .map(ConversationRow::into_response)
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/conversations",
    request_body = CreateConversationRequest,
    responses(
        (status = 201, description = "Conversation created", body = ConversationResponse)
    )
)]
pub async fn create_conversation(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateConversationRequest>,
) -> Result<(axum::http::StatusCode, Json<ConversationResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;

    let conv = sqlx::query_as::<_, ConversationRow>(
        "INSERT INTO conversations (user_id, project_id, title) VALUES ($1, $2, $3) \
         RETURNING id, user_id, project_id, title, created_at",
    )
    .bind(user_id)
    .bind(req.project_id)
    .bind(&req.title)
    .fetch_one(&state.db)
    .await?;

    // Invalidate cached session list
    crate::routes::sessions::invalidate_session_cache(&state, user_id).await;

    Ok((axum::http::StatusCode::CREATED, Json(conv.into_response())))
}

#[utoipa::path(
    get,
    path = "/api/v1/conversations/{id}",
    responses(
        (status = 200, description = "Conversation detail", body = ConversationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Conversation not found")
    )
)]
pub async fn get_conversation(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConversationResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let conv = sqlx::query_as::<_, ConversationRow>(
        "SELECT id, user_id, project_id, title, created_at \
         FROM conversations WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    Ok(Json(conv.into_response()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/conversations/{id}",
    responses(
        (status = 204, description = "Conversation deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Conversation not found")
    )
)]
pub async fn delete_conversation(
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
        return Err(AppError::NotFound("Conversation not found".to_string()));
    }

    // Invalidate cached session list
    crate::routes::sessions::invalidate_session_cache(&state, user_id).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
