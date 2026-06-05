use axum::{
    extract::{Path, Query},
    routing, Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/memories",
            routing::get(list_memories).post(create_memory),
        )
        .route(
            "/api/v1/memories/{id}",
            routing::get(get_memory)
                .patch(update_memory)
                .delete(delete_memory),
        )
        .route("/api/v1/memories/search", routing::get(search_memories))
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub memory_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryResponse {
    pub id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMemoryRequest {
    pub memory_type: String,
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: f32,
}

fn default_importance() -> f32 {
    0.5
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMemoryRequest {
    pub content: Option<String>,
    pub memory_type: Option<String>,
    pub importance: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<i64>,
}

// ── Handler: list ──────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/memories",
    responses((status = 200, body = Vec<MemoryResponse>))
)]
async fn list_memories(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<MemoryResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let rows: Vec<MemoryRow> = if let Some(mt) = &params.memory_type {
        sqlx::query_as(
            "SELECT id, memory_type, content, importance, metadata, \
             created_at, updated_at \
             FROM memories WHERE user_id = $1 AND memory_type = $2 \
             AND deleted_at IS NULL \
             ORDER BY importance DESC, created_at DESC LIMIT $3 OFFSET $4",
        )
        .bind(user_id)
        .bind(mt)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, memory_type, content, importance, metadata, \
             created_at, updated_at \
             FROM memories WHERE user_id = $1 AND deleted_at IS NULL \
             ORDER BY importance DESC, created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        rows.into_iter().map(MemoryRow::into_response).collect(),
    ))
}

// ── Handler: get by id ─────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/memories/{id}",
    responses((status = 200, body = MemoryResponse))
)]
async fn get_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<MemoryResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let row: MemoryRow = sqlx::query_as(
        "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
         FROM memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Memory not found".into()))?;

    Ok(Json(row.into_response()))
}

// ── Handler: create ────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/v1/memories",
    request_body = CreateMemoryRequest,
    responses((status = 201, body = MemoryResponse))
)]
async fn create_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<MemoryResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;

    let row: MemoryRow = sqlx::query_as(
        "INSERT INTO memories (user_id, memory_type, content, importance) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, memory_type, content, importance, metadata, created_at, updated_at",
    )
    .bind(user_id)
    .bind(&req.memory_type)
    .bind(&req.content)
    .bind(req.importance)
    .fetch_one(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(row.into_response())))
}

// ── Handler: update ────────────────────────────────────────────────

#[utoipa::path(
    patch,
    path = "/api/v1/memories/{id}",
    request_body = UpdateMemoryRequest,
    responses((status = 200, body = MemoryResponse))
)]
async fn update_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMemoryRequest>,
) -> Result<Json<MemoryResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Fetch current values for fields that weren't provided
    let current: MemoryRow = sqlx::query_as(
        "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
         FROM memories WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Memory not found".into()))?;

    let content = req.content.unwrap_or(current.content);
    let memory_type = req.memory_type.unwrap_or(current.memory_type);
    let importance = req.importance.unwrap_or(current.importance);

    let row: MemoryRow = sqlx::query_as(
        "UPDATE memories SET content = $1, memory_type = $2, importance = $3, \
         updated_at = NOW() \
         WHERE id = $4 AND user_id = $5 AND deleted_at IS NULL \
         RETURNING id, memory_type, content, importance, metadata, created_at, updated_at",
    )
    .bind(&content)
    .bind(&memory_type)
    .bind(importance)
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_response()))
}

// ── Handler: delete (soft) ─────────────────────────────────────────

#[utoipa::path(
    delete,
    path = "/api/v1/memories/{id}",
    responses((status = 204))
)]
async fn delete_memory(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;

    let rows = sqlx::query(
        "UPDATE memories SET deleted_at = NOW() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Memory not found".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Handler: search ────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/memories/search",
    params(
        ("q" = String, Query),
        ("limit" = Option<i64>, Query)
    ),
    responses((status = 200, body = Vec<MemoryResponse>))
)]
async fn search_memories(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<MemoryResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(20).min(100);
    let pattern = format!("%{}%", params.q);

    let rows: Vec<MemoryRow> = sqlx::query_as(
        "SELECT id, memory_type, content, importance, metadata, created_at, updated_at \
         FROM memories WHERE user_id = $1 AND content ILIKE $2 AND deleted_at IS NULL \
         ORDER BY importance DESC LIMIT $3",
    )
    .bind(user_id)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter().map(MemoryRow::into_response).collect(),
    ))
}

// ── FromRow helper ─────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct MemoryRow {
    id: Uuid,
    memory_type: String,
    content: String,
    importance: f32,
    metadata: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl MemoryRow {
    fn into_response(self) -> MemoryResponse {
        MemoryResponse {
            id: self.id,
            memory_type: self.memory_type,
            content: self.content,
            importance: self.importance,
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
