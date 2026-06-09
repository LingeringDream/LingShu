use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/users/me", get(get_me).patch(update_me))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub preferences: Option<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses(
        (status = 200, description = "Current user profile", body = UserResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn get_me(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<UserResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, email, display_name, role FROM users WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(user_id)
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

#[utoipa::path(
    patch,
    path = "/api/v1/users/me",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "Updated user profile", body = UserResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn update_me(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Fetch current values so we can PATCH correctly
    let current = sqlx::query_as::<_, (Uuid, String, String, String, serde_json::Value)>(
        "SELECT id, email, display_name, role, preferences \
         FROM users WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No users found".to_string()))?;

    let display_name = req.display_name.unwrap_or(current.2);
    // preferences is NOT NULL DEFAULT '{}' — cannot be set to NULL,
    // so unwrap_or works correctly (omitted = keep current).
    let preferences = req.preferences.unwrap_or(current.4);

    let user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "UPDATE users SET display_name = $1, preferences = $2, updated_at = NOW()
         WHERE id = $3 AND deleted_at IS NULL
         RETURNING id, email, display_name, role",
    )
    .bind(&display_name)
    .bind(&preferences)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(UserResponse {
        id: user.0,
        email: user.1,
        display_name: user.2,
        role: user.3,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_response_serialization() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let resp = UserResponse {
            id,
            email: "test@example.com".into(),
            display_name: "Test User".into(),
            role: "owner".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["email"], "test@example.com");
        assert_eq!(json["role"], "owner");
    }

    #[test]
    fn update_user_request_partial() {
        let json = serde_json::json!({"display_name": "New Name"});
        let req: UpdateUserRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.display_name.unwrap(), "New Name");
        assert!(req.preferences.is_none());
    }

    #[test]
    fn update_user_request_all_fields() {
        let prefs = serde_json::json!({"theme": "dark", "language": "zh"});
        let json = serde_json::json!({"display_name": "Updated", "preferences": prefs});
        let req: UpdateUserRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.display_name.unwrap(), "Updated");
        assert!(req.preferences.is_some());
    }
}
