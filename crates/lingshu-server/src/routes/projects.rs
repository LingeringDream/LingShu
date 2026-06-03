use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route(
            "/api/v1/projects/{id}",
            get(get_project).patch(update_project).delete(delete_project),
        )
        .route("/api/v1/projects/{id}/health", get(get_health))
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

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    responses(
        (status = 200, description = "List of projects", body = Vec<ProjectResponse>)
    )
)]
pub async fn list_projects(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    let projects = sqlx::query_as::<_, (Uuid, String, Option<String>, String, Option<f32>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, name, description, status, health_score, created_at FROM projects WHERE deleted_at IS NULL ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        projects
            .into_iter()
            .map(|p| ProjectResponse {
                id: p.0,
                name: p.1,
                description: p.2,
                status: p.3,
                health_score: p.4,
                created_at: p.5,
            })
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
    Json(req): Json<CreateProjectRequest>,
) -> Result<(axum::http::StatusCode, Json<ProjectResponse>), AppError> {
    // Phase 0: use first user as owner
    let owner_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE deleted_at IS NULL LIMIT 1"
    )
    .fetch_one(&state.db)
    .await?;

    let project = sqlx::query_as::<_, (Uuid, String, Option<String>, String, Option<f32>, chrono::DateTime<chrono::Utc>)>(
        "INSERT INTO projects (name, description, owner_id) VALUES ($1, $2, $3) RETURNING id, name, description, status, health_score, created_at"
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(owner_id)
    .fetch_one(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ProjectResponse {
            id: project.0,
            name: project.1,
            description: project.2,
            status: project.3,
            health_score: project.4,
            created_at: project.5,
        }),
    ))
}

pub async fn get_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = sqlx::query_as::<_, (Uuid, String, Option<String>, String, Option<f32>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, name, description, status, health_score, created_at FROM projects WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    Ok(Json(ProjectResponse {
        id: project.0,
        name: project.1,
        description: project.2,
        status: project.3,
        health_score: project.4,
        created_at: project.5,
    }))
}

pub async fn update_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = sqlx::query_as::<_, (Uuid, String, Option<String>, String, Option<f32>, chrono::DateTime<chrono::Utc>)>(
        "UPDATE projects SET name = $1, description = $2, updated_at = NOW() WHERE id = $3 AND deleted_at IS NULL RETURNING id, name, description, status, health_score, created_at"
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    Ok(Json(ProjectResponse {
        id: project.0,
        name: project.1,
        description: project.2,
        status: project.3,
        health_score: project.4,
        created_at: project.5,
    }))
}

pub async fn delete_project(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let result = sqlx::query(
        "UPDATE projects SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Project not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn get_health(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = sqlx::query_as::<_, (String, Option<f32>)>(
        "SELECT status, health_score FROM projects WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
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
