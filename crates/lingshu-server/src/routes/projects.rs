use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route(
            "/api/v1/projects/:id",
            get(get_project)
                .patch(update_project)
                .delete(delete_project),
        )
        .route("/api/v1/projects/:id/health", get(get_health))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub health_score: Option<f32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Row helper ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    status: String,
    health_score: Option<f32>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl ProjectRow {
    fn into_response(self) -> ProjectResponse {
        ProjectResponse {
            id: self.id,
            name: self.name,
            description: self.description,
            status: self.status,
            health_score: self.health_score,
            created_at: self.created_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    responses(
        (status = 200, description = "List of projects", body = Vec<ProjectResponse>),
        (status = 401)
    )
)]
pub async fn list_projects(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let projects = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, status, health_score, created_at \
         FROM projects WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        projects
            .into_iter()
            .map(ProjectRow::into_response)
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, description = "Project created", body = ProjectResponse)
    )
)]
pub async fn create_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(axum::http::StatusCode, Json<ProjectResponse>), AppError> {
    let owner_id = auth::require_user(auth).await?;

    let project = sqlx::query_as::<_, ProjectRow>(
        "INSERT INTO projects (name, description, owner_id) VALUES ($1, $2, $3) \
         RETURNING id, name, description, status, health_score, created_at",
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(owner_id)
    .fetch_one(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(project.into_response()),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}",
    responses(
        (status = 200, description = "Project detail", body = ProjectResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found")
    )
)]
pub async fn get_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let project = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, status, health_score, created_at \
         FROM projects WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    Ok(Json(project.into_response()))
}

#[utoipa::path(
    patch,
    path = "/api/v1/projects/{id}",
    request_body = CreateProjectRequest,
    responses(
        (status = 200, description = "Project updated", body = ProjectResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found")
    )
)]
pub async fn update_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let project = sqlx::query_as::<_, ProjectRow>(
        "UPDATE projects SET name = $1, description = $2, updated_at = NOW() \
         WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL \
         RETURNING id, name, description, status, health_score, created_at",
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    Ok(Json(project.into_response()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}",
    responses(
        (status = 204, description = "Project deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found")
    )
)]
pub async fn delete_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;

    let result =
        sqlx::query("UPDATE projects SET deleted_at = NOW() WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL")
            .bind(id)
            .bind(user_id)
            .execute(&state.db)
            .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Project not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}/health",
    responses(
        (status = 200, description = "Project health indicators"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found")
    )
)]
pub async fn get_health(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let project = sqlx::query_as::<_, (String, Option<f32>)>(
        "SELECT status, health_score FROM projects WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    Ok(Json(serde_json::json!({
        "project_id": id,
        "status": project.0,
        "health_score": project.1.unwrap_or(0.0),
        "indicators": {
            "schedule_variance": 0.0,
            "cost_variance": 0.0,
            "risk_exposure": 0.0,
            "team_load_balance": 1.0,
            "dependency_block_index": 0.0
        }
    })))
}
