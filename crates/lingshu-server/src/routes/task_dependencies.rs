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
            "/api/v1/projects/:pid/tasks/:tid/dependencies",
            get(list_dependencies).post(add_dependency),
        )
        .route(
            "/api/v1/projects/:pid/tasks/:tid/dependencies/:did",
            get(get_dependency).delete(remove_dependency),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddDependencyRequest {
    pub depends_on_id: Uuid,
    #[serde(default = "default_dep_type")]
    pub dependency_type: String,
}

fn default_dep_type() -> String {
    "finish_to_start".to_string()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DependencyResponse {
    pub id: Uuid,
    pub task_id: Uuid,
    pub depends_on_id: Uuid,
    pub dependency_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Helpers ──────────────────────────────────────────────────────

async fn verify_task_access(
    db: &sqlx::PgPool,
    project_id: Uuid,
    task_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(\
            SELECT 1 FROM tasks t \
            JOIN projects p ON t.project_id = p.id \
            WHERE t.id = $1 AND t.project_id = $2 AND p.owner_id = $3 \
            AND t.deleted_at IS NULL AND p.deleted_at IS NULL\
        )",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(user_id)
    .fetch_one(db)
    .await?;

    if !exists {
        return Err(AppError::NotFound("Task not found".to_string()));
    }
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/projects/{pid}/tasks/{tid}/dependencies",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID")
    ),
    responses((status = 200, body = Vec<DependencyResponse>))
)]
async fn list_dependencies(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<DependencyResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_task_access(&state.db, pid, tid, user_id).await?;

    let rows: Vec<DepRow> = sqlx::query_as(
        "SELECT id, task_id, depends_on_id, dependency_type, created_at \
         FROM task_dependencies WHERE task_id = $1 ORDER BY created_at",
    )
    .bind(tid)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(DepRow::into_response).collect()))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{pid}/tasks/{tid}/dependencies",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID")
    ),
    request_body = AddDependencyRequest,
    responses((status = 201, body = DependencyResponse))
)]
async fn add_dependency(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(req): Json<AddDependencyRequest>,
) -> Result<(axum::http::StatusCode, Json<DependencyResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_task_access(&state.db, pid, tid, user_id).await?;

    // Verify depends_on task also belongs to same project
    verify_task_access(&state.db, pid, req.depends_on_id, user_id).await?;

    let row: DepRow = sqlx::query_as(
        "INSERT INTO task_dependencies (task_id, depends_on_id, dependency_type) \
         VALUES ($1, $2, $3) RETURNING id, task_id, depends_on_id, dependency_type, created_at",
    )
    .bind(tid)
    .bind(req.depends_on_id)
    .bind(&req.dependency_type)
    .fetch_one(&state.db)
    .await
    .map_err(|e: sqlx::Error| {
        if e.to_string().contains("check_task_no_self_dep")
            || e.to_string().contains("task_id != depends_on_id")
        {
            AppError::Validation("Task cannot depend on itself".to_string())
        } else if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Validation("Dependency already exists".to_string())
        } else {
            AppError::Database(e)
        }
    })?;

    Ok((axum::http::StatusCode::CREATED, Json(row.into_response())))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{pid}/tasks/{tid}/dependencies/{did}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID"),
        ("did" = Uuid, Path, description = "Dependency ID")
    ),
    responses((status = 200, body = DependencyResponse))
)]
async fn get_dependency(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, _tid, did)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<DependencyResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_task_access(&state.db, pid, _tid, user_id).await?;

    let row: DepRow = sqlx::query_as(
        "SELECT td.id, td.task_id, td.depends_on_id, td.dependency_type, td.created_at \
         FROM task_dependencies td \
         JOIN tasks t ON td.task_id = t.id \
         WHERE td.id = $1 AND t.project_id = $2 AND t.deleted_at IS NULL",
    )
    .bind(did)
    .bind(pid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Dependency not found".to_string()))?;

    Ok(Json(row.into_response()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{pid}/tasks/{tid}/dependencies/{did}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID"),
        ("did" = Uuid, Path, description = "Dependency ID")
    ),
    responses((status = 204))
)]
async fn remove_dependency(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, _tid, did)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_task_access(&state.db, pid, _tid, user_id).await?;

    let rows = sqlx::query(
        "DELETE FROM task_dependencies td \
         USING tasks t \
         WHERE td.id = $1 AND td.task_id = t.id AND t.project_id = $2 \
         AND t.deleted_at IS NULL",
    )
    .bind(did)
    .bind(pid)
    .execute(&state.db)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Dependency not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Row helper ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct DepRow {
    id: Uuid,
    task_id: Uuid,
    depends_on_id: Uuid,
    dependency_type: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl DepRow {
    fn into_response(self) -> DependencyResponse {
        DependencyResponse {
            id: self.id,
            task_id: self.task_id,
            depends_on_id: self.depends_on_id,
            dependency_type: self.dependency_type,
            created_at: self.created_at,
        }
    }
}
