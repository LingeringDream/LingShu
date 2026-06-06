use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::thought::Thought;
use crate::routes::settings::llm_settings_for_user;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/thoughts", get(list_thoughts))
        .route(
            "/api/v1/thoughts/{id}",
            get(get_thought).patch(update_thought),
        )
        .route(
            "/api/v1/thoughts/generate",
            axum::routing::post(generate_thoughts),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListThoughtsParams {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateThoughtRequest {
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThoughtResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub detail: Option<String>,
    pub reason: Option<String>,
    pub confidence: f32,
    pub source_memory_ids: Vec<Uuid>,
    pub requires_confirmation: bool,
    pub status: String,
    pub scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Thought> for ThoughtResponse {
    fn from(t: Thought) -> Self {
        Self {
            id: t.id,
            user_id: t.user_id,
            title: t.title,
            detail: t.detail,
            reason: t.reason,
            confidence: t.confidence,
            source_memory_ids: t.source_memory_ids,
            requires_confirmation: t.requires_confirmation,
            status: t.status.as_str().to_string(),
            scheduled_at: t.scheduled_at,
            resolved_at: t.resolved_at,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/thoughts",
    params(
        ("status" = Option<String>, Query, description = "Filter by status"),
        ("limit" = Option<i64>, Query, description = "Max results")
    ),
    responses((status = 200, body = Vec<ThoughtResponse>))
)]
async fn list_thoughts(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<ListThoughtsParams>,
) -> Result<Json<Vec<ThoughtResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(50).min(200);

    let thoughts: Vec<Thought> = if let Some(st) = &params.status {
        sqlx::query_as(
            "SELECT * FROM thought_queue WHERE user_id = $1 AND status = $2 \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $3",
        )
        .bind(user_id)
        .bind(st)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT * FROM thought_queue WHERE user_id = $1 \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        thoughts.into_iter().map(ThoughtResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/thoughts/{id}",
    params(("id" = Uuid, Path, description = "Thought ID")),
    responses((status = 200, body = ThoughtResponse))
)]
async fn get_thought(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ThoughtResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let thought: Thought =
        sqlx::query_as("SELECT * FROM thought_queue WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Thought not found".to_string()))?;

    Ok(Json(ThoughtResponse::from(thought)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/thoughts/{id}",
    params(("id" = Uuid, Path, description = "Thought ID")),
    request_body = UpdateThoughtRequest,
    responses((status = 200, body = ThoughtResponse))
)]
async fn update_thought(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateThoughtRequest>,
) -> Result<Json<ThoughtResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Only allow specific status transitions
    let new_status = match req.status.as_deref() {
        Some("confirmed") | Some("dismissed") => req.status.unwrap(),
        Some(other) => {
            return Err(AppError::Validation(format!(
                "Invalid status '{other}'. Allowed: confirmed, dismissed"
            )))
        }
        None => return Err(AppError::BadRequest("status field is required".to_string())),
    };

    // Fetch current thought to validate the transition
    let current: Thought =
        sqlx::query_as("SELECT * FROM thought_queue WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Thought not found".to_string()))?;

    // Only allow transitions from pending or shown states
    let current_status = current.status.as_str();
    if current_status != "pending" && current_status != "shown" {
        return Err(AppError::Validation(format!(
            "Cannot transition thought from '{current_status}' status. Only pending or shown thoughts can be confirmed/dismissed."
        )));
    }

    let thought: Thought = sqlx::query_as(
        "UPDATE thought_queue SET status = $1, resolved_at = NOW(), \
         updated_at = NOW() \
         WHERE id = $2 AND user_id = $3 RETURNING *",
    )
    .bind(&new_status)
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ThoughtResponse::from(thought)))
}

// ── Handler: generate ─────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateThoughtsResponse {
    /// Number of new thoughts created (0-3)
    pub created: usize,
}

#[utoipa::path(
    post,
    path = "/api/v1/thoughts/generate",
    responses(
        (status = 200, description = "Thoughts generated", body = GenerateThoughtsResponse),
        (status = 401, description = "Unauthorized")
    )
)]
async fn generate_thoughts(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<GenerateThoughtsResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let settings = llm_settings_for_user(&state, user_id).await;

    if settings.model.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!("Model not configured.")));
    }

    let created = crate::llm::thoughts::generate_and_save_thoughts(
        &state.db,
        &state.llm,
        &settings.model,
        user_id,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(Json(GenerateThoughtsResponse { created }))
}
