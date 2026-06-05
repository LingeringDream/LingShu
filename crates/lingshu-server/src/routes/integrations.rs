use axum::{extract::Path, routing::get, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::integration::Integration;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/integrations", get(list_integrations))
        .route(
            "/api/v1/integrations/{id}",
            get(get_integration).delete(delete_integration),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct IntegrationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub platform: String,
    pub token_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub config: serde_json::Value,
    pub status: String,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Integration> for IntegrationResponse {
    fn from(i: Integration) -> Self {
        Self {
            id: i.id,
            user_id: i.user_id,
            project_id: i.project_id,
            platform: i.platform,
            token_expires_at: i.token_expires_at,
            config: i.config,
            status: i.status,
            last_sync_at: i.last_sync_at,
            created_at: i.created_at,
            updated_at: i.updated_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/integrations",
    responses((status = 200, body = Vec<IntegrationResponse>))
)]
async fn list_integrations(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<IntegrationResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let integrations: Vec<Integration> =
        sqlx::query_as("SELECT * FROM integrations WHERE user_id = $1 ORDER BY created_at DESC")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;

    Ok(Json(
        integrations
            .into_iter()
            .map(IntegrationResponse::from)
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/integrations/{id}",
    params(("id" = Uuid, Path, description = "Integration ID")),
    responses((status = 200, body = IntegrationResponse))
)]
async fn get_integration(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<IntegrationResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let integration: Integration =
        sqlx::query_as("SELECT * FROM integrations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Integration not found".to_string()))?;

    Ok(Json(IntegrationResponse::from(integration)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/integrations/{id}",
    params(("id" = Uuid, Path, description = "Integration ID")),
    responses((status = 204))
)]
async fn delete_integration(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;

    let rows = sqlx::query("DELETE FROM integrations WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Integration not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
