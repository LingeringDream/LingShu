use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::crypto;
use crate::error::AppError;
use crate::models::integration::Integration;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/integrations",
            get(list_integrations).post(create_integration),
        )
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIntegrationRequest {
    pub platform: String,
    /// Plaintext access token. Encrypted with AES-256-GCM before being persisted;
    /// never stored or echoed back in the clear.
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub project_id: Option<Uuid>,
    pub config: Option<serde_json::Value>,
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

    // Select only the columns needed for the response — encrypted token
    // blobs are intentionally excluded from this query.
    let integrations: Vec<Integration> =
        sqlx::query_as(
            "SELECT id, user_id, project_id, platform, \
                    ''::bytea AS access_token_encrypted, \
                    NULL::bytea AS refresh_token_encrypted, \
                    token_expires_at, config, status, last_sync_at, \
                    created_at, updated_at \
             FROM integrations WHERE user_id = $1 ORDER BY created_at DESC",
        )
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
    post,
    path = "/api/v1/integrations",
    request_body = CreateIntegrationRequest,
    responses(
        (status = 201, description = "Integration created", body = IntegrationResponse),
        (status = 409, description = "Integration for this platform already exists"),
        (status = 422, description = "Validation error")
    )
)]
async fn create_integration(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateIntegrationRequest>,
) -> Result<(axum::http::StatusCode, Json<IntegrationResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;

    let platform = req.platform.trim().to_string();
    if platform.is_empty() {
        return Err(AppError::Validation("platform must not be empty".into()));
    }
    if req.access_token.trim().is_empty() {
        return Err(AppError::Validation(
            "access_token must not be empty".into(),
        ));
    }
    if let Some(project_id) = req.project_id {
        let owns_project: bool = sqlx::query_scalar(
            "SELECT EXISTS(\
                SELECT 1 FROM projects \
                WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL\
            )",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

        if !owns_project {
            return Err(AppError::NotFound("Project not found".to_string()));
        }
    }

    let duplicate_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM integrations \
            WHERE user_id = $1 \
              AND project_id IS NOT DISTINCT FROM $2 \
              AND platform = $3\
        )",
    )
    .bind(user_id)
    .bind(req.project_id)
    .bind(&platform)
    .fetch_one(&state.db)
    .await?;

    if duplicate_exists {
        return Err(AppError::Conflict(
            "An integration for this platform already exists".to_string(),
        ));
    }

    // Tokens must never be stored in the clear — refuse writes until an operator
    // configures ENCRYPTION_KEY rather than silently persisting plaintext.
    let encryption_key = state.encryption_key.as_deref().ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "ENCRYPTION_KEY is not configured; set it before creating integrations"
        ))
    })?;

    let access_token_encrypted = crypto::encrypt(&req.access_token, encryption_key)?;
    let refresh_token_encrypted = req
        .refresh_token
        .as_deref()
        .map(|token| crypto::encrypt(token, encryption_key))
        .transpose()?;
    let config = req.config.clone().unwrap_or_else(|| serde_json::json!({}));

    let integration: Integration = sqlx::query_as(
        "INSERT INTO integrations \
            (user_id, project_id, platform, access_token_encrypted, refresh_token_encrypted, config) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING *",
    )
    .bind(user_id)
    .bind(req.project_id)
    .bind(&platform)
    .bind(access_token_encrypted)
    .bind(refresh_token_encrypted)
    .bind(config)
    .fetch_one(&state.db)
    .await
    .map_err(|e: sqlx::Error| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Conflict("An integration for this platform already exists".to_string())
        } else {
            AppError::Database(e)
        }
    })?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(IntegrationResponse::from(integration)),
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

    let integration: Integration = sqlx::query_as(
        "SELECT id, user_id, project_id, platform, \
                ''::bytea AS access_token_encrypted, \
                NULL::bytea AS refresh_token_encrypted, \
                token_expires_at, config, status, last_sync_at, \
                created_at, updated_at \
         FROM integrations WHERE id = $1 AND user_id = $2",
    )
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

// ── Internal helpers ──────────────────────────────────────────────
//
// `decrypt_token` recovers a plaintext token for server-side use only — e.g. to
// authenticate an outbound call to the integrated platform. Its result must never
// be placed in an HTTP response (see `IntegrationResponse`, which omits token
// fields entirely).

pub(crate) fn decrypt_token(
    encryption_key: Option<&str>,
    encrypted: &[u8],
) -> anyhow::Result<String> {
    let key = encryption_key.ok_or_else(|| {
        anyhow::anyhow!("ENCRYPTION_KEY is not configured; cannot decrypt stored token")
    })?;
    crypto::decrypt(encrypted, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "test-integration-key";

    #[test]
    fn decrypt_token_round_trips_with_correct_key() {
        let blob = crypto::encrypt("super-secret-access-token", KEY).expect("encrypt");
        assert_eq!(
            decrypt_token(Some(KEY), &blob).expect("decrypt"),
            "super-secret-access-token"
        );
    }

    #[test]
    fn decrypt_token_errors_when_key_unconfigured() {
        let blob = crypto::encrypt("super-secret-access-token", KEY).expect("encrypt");
        assert!(decrypt_token(None, &blob).is_err());
    }
}
