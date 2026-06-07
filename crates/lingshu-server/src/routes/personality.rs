use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::personality::{PersonalitySnapshot, PersonalityTraits};
use crate::routes::settings::llm_settings_for_user;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/personality/snapshots",
            get(list_snapshots).post(create_snapshot),
        )
        .route(
            "/api/v1/personality/snapshots/active",
            get(get_active_snapshot),
        )
        .route(
            "/api/v1/personality/snapshots/:id/activate",
            post(activate_snapshot),
        )
        .route("/api/v1/personality/evolve", post(evolve_personality))
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSnapshotRequest {
    pub trait_values: PersonalityTraits,
    #[serde(default)]
    pub change_reason: Option<String>,
    #[serde(default)]
    pub source_memory_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SnapshotResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub trait_values: serde_json::Value,
    pub change_reason: Option<String>,
    pub source_memory_ids: Vec<Uuid>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<PersonalitySnapshot> for SnapshotResponse {
    fn from(s: PersonalitySnapshot) -> Self {
        Self {
            id: s.id,
            user_id: s.user_id,
            trait_values: s.trait_values,
            change_reason: s.change_reason,
            source_memory_ids: s.source_memory_ids,
            is_active: s.is_active,
            created_at: s.created_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/personality/snapshots",
    responses((status = 200, body = Vec<SnapshotResponse>))
)]
async fn list_snapshots(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<SnapshotResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let snapshots: Vec<PersonalitySnapshot> = sqlx::query_as(
        "SELECT * FROM personality_snapshots WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        snapshots.into_iter().map(SnapshotResponse::from).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/personality/snapshots",
    request_body = CreateSnapshotRequest,
    responses((status = 201, body = SnapshotResponse))
)]
async fn create_snapshot(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<(axum::http::StatusCode, Json<SnapshotResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;

    let trait_values = serde_json::to_value(&req.trait_values)
        .map_err(|e| AppError::BadRequest(format!("Invalid trait values: {e}")))?;

    let source_memory_ids = req.source_memory_ids;

    let snapshot: PersonalitySnapshot = sqlx::query_as(
        "INSERT INTO personality_snapshots \
         (user_id, trait_values, change_reason, source_memory_ids) \
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(user_id)
    .bind(&trait_values)
    .bind(&req.change_reason)
    .bind(&source_memory_ids)
    .fetch_one(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(SnapshotResponse::from(snapshot)),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/personality/snapshots/active",
    responses((status = 200, body = SnapshotResponse))
)]
async fn get_active_snapshot(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<SnapshotResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let snapshot: PersonalitySnapshot = sqlx::query_as(
        "SELECT * FROM personality_snapshots WHERE user_id = $1 AND is_active = true LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No active personality snapshot found. Create one with POST /api/v1/personality/snapshots.".to_string()))?;

    Ok(Json(SnapshotResponse::from(snapshot)))
}

#[utoipa::path(
    post,
    path = "/api/v1/personality/snapshots/{id}/activate",
    params(("id" = Uuid, Path, description = "Snapshot ID")),
    responses((status = 200, body = SnapshotResponse))
)]
async fn activate_snapshot(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<SnapshotResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Atomic: deactivate all + activate target in a transaction
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE personality_snapshots SET is_active = false WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    let snapshot: PersonalitySnapshot = sqlx::query_as(
        "UPDATE personality_snapshots SET is_active = true \
         WHERE id = $1 AND user_id = $2 RETURNING *",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Snapshot not found".to_string()))?;

    tx.commit().await?;

    Ok(Json(SnapshotResponse::from(snapshot)))
}

// ── Handler: evolve ───────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct EvolvePersonalityResponse {
    pub created: bool,
    pub reason: String,
    pub snapshot: Option<SnapshotResponse>,
}

#[utoipa::path(
    post,
    path = "/api/v1/personality/evolve",
    responses(
        (status = 200, description = "Evolution result", body = EvolvePersonalityResponse),
        (status = 401, description = "Unauthorized")
    )
)]
async fn evolve_personality(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<EvolvePersonalityResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let settings = llm_settings_for_user(&state, user_id).await;

    if settings.model.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Model not configured. Set it via PATCH /api/v1/settings/llm or LLM_DEFAULT_MODEL in .env."
        )));
    }

    let outcome = crate::llm::personality::evolve_and_save_personality(
        &state.db,
        &state.llm,
        &settings.model,
        user_id,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(Json(EvolvePersonalityResponse {
        created: outcome.created,
        reason: outcome.reason,
        snapshot: outcome.snapshot.map(SnapshotResponse::from),
    }))
}
