use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/conversations", get(list_conversations).post(create_conversation))
        .route(
            "/api/v1/conversations/{id}",
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

#[utoipa::path(
    get,
    path = "/api/v1/conversations",
    responses(
        (status = 200, description = "List of conversations", body = Vec<ConversationResponse>)
    )
)]
pub async fn list_conversations(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<ConversationResponse>>, AppError> {
    // Phase 0: use first user
    let user_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_one(&state.db)
    .await?;

    let convs = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, user_id, project_id, title, created_at FROM conversations WHERE user_id = $1 AND deleted_at IS NULL ORDER BY updated_at DESC"
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        convs
            .into_iter()
            .map(|c| ConversationResponse {
                id: c.0,
                user_id: c.1,
                project_id: c.2,
                title: c.3,
                created_at: c.4,
            })
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
    Json(req): Json<CreateConversationRequest>,
) -> Result<(axum::http::StatusCode, Json<ConversationResponse>), AppError> {
    let user_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_one(&state.db)
    .await?;

    let conv = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "INSERT INTO conversations (user_id, project_id, title) VALUES ($1, $2, $3) RETURNING id, user_id, project_id, title, created_at"
    )
    .bind(user_id)
    .bind(req.project_id)
    .bind(&req.title)
    .fetch_one(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ConversationResponse {
            id: conv.0,
            user_id: conv.1,
            project_id: conv.2,
            title: conv.3,
            created_at: conv.4,
        }),
    ))
}

pub async fn get_conversation(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConversationResponse>, AppError> {
    let conv = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, user_id, project_id, title, created_at FROM conversations WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    Ok(Json(ConversationResponse {
        id: conv.0,
        user_id: conv.1,
        project_id: conv.2,
        title: conv.3,
        created_at: conv.4,
    }))
}

pub async fn delete_conversation(
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
        return Err(AppError::NotFound("Conversation not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
