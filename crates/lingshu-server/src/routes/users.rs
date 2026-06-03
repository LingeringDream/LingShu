use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/users/me", get(get_me).patch(update_me))
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub preferences: Option<serde_json::Value>,
}

pub async fn get_me(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<UserResponse>, AppError> {
    // Phase 0: return first user (no auth middleware yet)
    let user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, email, display_name, role FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No users found".to_string()))?;

    Ok(Json(UserResponse {
        id: user.0,
        email: user.1,
        display_name: user.2,
        role: user.3,
    }))
}

pub async fn update_me(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    // Phase 0: update first user
    let user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "UPDATE users SET display_name = COALESCE($1, display_name), preferences = COALESCE($2, preferences), updated_at = NOW()
         WHERE id = (SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1)
         RETURNING id, email, display_name, role"
    )
    .bind(&req.display_name)
    .bind(&req.preferences)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No users found".to_string()))?;

    Ok(Json(UserResponse {
        id: user.0,
        email: user.1,
        display_name: user.2,
        role: user.3,
    }))
}
