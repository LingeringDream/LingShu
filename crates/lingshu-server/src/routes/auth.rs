use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub token: String,
}

pub async fn register(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Phase 0: simplified auth, just insert and return a dummy token
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(&req.email)
    .bind(&req.display_name)
    .bind(&req.password) // TODO: hash with argon2
    .fetch_one(&state.db)
    .await?;

    Ok(Json(AuthResponse {
        user_id,
        token: "phase0-placeholder-token".to_string(),
    }))
}

pub async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE email = $1 AND password_hash = $2 AND deleted_at IS NULL"
    )
    .bind(&req.email)
    .bind(&req.password) // TODO: verify with argon2
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    Ok(Json(AuthResponse {
        user_id: user.0,
        token: "phase0-placeholder-token".to_string(),
    }))
}
