use axum::{extract::Query, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/memories", get(list_memories))
        .route("/api/v1/memories/search", get(search_memories))
}

#[derive(Debug, Deserialize)]
pub struct ListMemoriesParams {
    pub memory_type: Option<String>,
    pub project_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryResponse {
    pub id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/memories",
    responses(
        (status = 200, description = "List of memories", body = Vec<MemoryResponse>)
    )
)]
pub async fn list_memories(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(params): Query<ListMemoriesParams>,
) -> Result<Json<Vec<MemoryResponse>>, AppError> {
    let user_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_one(&state.db)
    .await?;

    let limit = params.limit.unwrap_or(50).min(200);

    let memories = if let Some(mt) = &params.memory_type {
        sqlx::query_as::<_, (Uuid, String, String, f32, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, memory_type, content, importance, created_at FROM memories WHERE user_id = $1 AND memory_type = $2 AND deleted_at IS NULL ORDER BY importance DESC, created_at DESC LIMIT $3"
        )
        .bind(user_id)
        .bind(mt)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, f32, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, memory_type, content, importance, created_at FROM memories WHERE user_id = $1 AND deleted_at IS NULL ORDER BY importance DESC, created_at DESC LIMIT $2"
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        memories
            .into_iter()
            .map(|m| MemoryResponse {
                id: m.0,
                memory_type: m.1,
                content: m.2,
                importance: m.3,
                created_at: m.4,
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/memories/search",
    params(
        ("q" = String, Query, description = "Search query"),
        ("limit" = Option<i64>, Query, description = "Max results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<MemoryResponse>)
    )
)]
pub async fn search_memories(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<MemoryResponse>>, AppError> {
    let user_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_one(&state.db)
    .await?;

    let limit = params.limit.unwrap_or(20).min(100);
    let pattern = format!("%{}%", params.q);

    let memories = sqlx::query_as::<_, (Uuid, String, String, f32, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, memory_type, content, importance, created_at FROM memories WHERE user_id = $1 AND content ILIKE $2 AND deleted_at IS NULL ORDER BY importance DESC LIMIT $3"
    )
    .bind(user_id)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        memories
            .into_iter()
            .map(|m| MemoryResponse {
                id: m.0,
                memory_type: m.1,
                content: m.2,
                importance: m.3,
                created_at: m.4,
            })
            .collect(),
    ))
}
