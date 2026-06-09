use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/:pid/members",
            get(list_members).post(add_member),
        )
        .route(
            "/api/v1/projects/:pid/members/:uid",
            get(get_member).delete(remove_member),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "member".to_string()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: chrono::DateTime<chrono::Utc>,
}

// ── Helpers ──────────────────────────────────────────────────────

async fn verify_project_ownership(
    db: &sqlx::PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL)",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(db)
    .await?;

    if !exists {
        return Err(AppError::NotFound("Project not found".to_string()));
    }
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/projects/{pid}/members",
    params(("pid" = Uuid, Path, description = "Project ID")),
    responses((status = 200, body = Vec<MemberResponse>))
)]
async fn list_members(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(pid): Path<Uuid>,
) -> Result<Json<Vec<MemberResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT id, project_id, user_id, role, joined_at \
         FROM project_members WHERE project_id = $1 ORDER BY joined_at DESC",
    )
    .bind(pid)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter().map(MemberRow::into_response).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{pid}/members",
    params(("pid" = Uuid, Path, description = "Project ID")),
    request_body = AddMemberRequest,
    responses((status = 201, body = MemberResponse))
)]
async fn add_member(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(pid): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<(axum::http::StatusCode, Json<MemberResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    // Verify the target user exists
    let user_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND deleted_at IS NULL)",
    )
    .bind(req.user_id)
    .fetch_one(&state.db)
    .await?;

    if !user_exists {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    let row: MemberRow = sqlx::query_as(
        "INSERT INTO project_members (project_id, user_id, role) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (project_id, user_id) DO UPDATE SET role = $3 \
         RETURNING id, project_id, user_id, role, joined_at",
    )
    .bind(pid)
    .bind(req.user_id)
    .bind(&req.role)
    .fetch_one(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(row.into_response())))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{pid}/members/{uid}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("uid" = Uuid, Path, description = "User ID")
    ),
    responses((status = 200, body = MemberResponse))
)]
async fn get_member(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, uid)): Path<(Uuid, Uuid)>,
) -> Result<Json<MemberResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let row: MemberRow = sqlx::query_as(
        "SELECT id, project_id, user_id, role, joined_at \
         FROM project_members WHERE project_id = $1 AND user_id = $2",
    )
    .bind(pid)
    .bind(uid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Member not found".to_string()))?;

    Ok(Json(row.into_response()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{pid}/members/{uid}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("uid" = Uuid, Path, description = "User ID")
    ),
    responses((status = 204))
)]
async fn remove_member(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, uid)): Path<(Uuid, Uuid)>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let rows = sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
        .bind(pid)
        .bind(uid)
        .execute(&state.db)
        .await?
        .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Member not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Row helper ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct MemberRow {
    id: Uuid,
    project_id: Uuid,
    user_id: Uuid,
    role: String,
    joined_at: chrono::DateTime<chrono::Utc>,
}

impl MemberRow {
    fn into_response(self) -> MemberResponse {
        MemberResponse {
            id: self.id,
            project_id: self.project_id,
            user_id: self.user_id,
            role: self.role,
            joined_at: self.joined_at,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_role_is_member() {
        assert_eq!(default_role(), "member");
    }

    #[test]
    fn add_member_request_defaults() {
        let uid = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let json = serde_json::json!({"user_id": uid.to_string()});
        let req: AddMemberRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.user_id, uid);
        assert_eq!(req.role, "member");
    }

    #[test]
    fn add_member_request_custom_role() {
        let uid = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let json = serde_json::json!({"user_id": uid.to_string(), "role": "admin"});
        let req: AddMemberRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.role, "admin");
    }
}
