use axum::{extract::State, routing::post, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth;
use crate::error::AppError;
use crate::state::AppState;

const LOCAL_USER_EMAIL: &str = "local@lingshu.internal";
const LOCAL_USER_DISPLAY_NAME: &str = "本地用户";
const LOCAL_USER_PASSWORD_MARKER: &str = "local-session-only";

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/auth/local-session", post(local_session))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub token: String,
    pub display_name: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/local-session",
    responses(
        (status = 200, description = "Local session created", body = AuthResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn local_session(State(state): State<AppState>) -> Result<Json<AuthResponse>, AppError> {
    let user_id = ensure_local_user(&state).await?;
    let token = auth::sign_token(user_id, &state.config.security.jwt_secret)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Token generation failed: {e}")))?;

    Ok(Json(AuthResponse {
        user_id,
        token,
        display_name: LOCAL_USER_DISPLAY_NAME.to_string(),
    }))
}

async fn ensure_local_user(state: &AppState) -> Result<Uuid, AppError> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (email, display_name, password_hash, role) \
         VALUES ($1, $2, $3, 'owner') \
         ON CONFLICT (email) DO UPDATE SET \
            display_name = EXCLUDED.display_name, \
            role = EXCLUDED.role, \
            deleted_at = NULL, \
            updated_at = NOW() \
         RETURNING id",
    )
    .bind(LOCAL_USER_EMAIL)
    .bind(LOCAL_USER_DISPLAY_NAME)
    .bind(LOCAL_USER_PASSWORD_MARKER)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Database)
}
