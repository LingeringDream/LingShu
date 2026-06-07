use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

/// Verify that a project exists and belongs to the given user.
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/:pid/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/api/v1/projects/:pid/tasks/:tid",
            get(get_task).patch(update_task).delete(delete_task),
        )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub assignee_id: Option<Uuid>,
    pub due_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i16>,
    /// Set to `null` to clear the assignee. Omit to leave unchanged.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub assignee_id: Option<Option<Uuid>>,
    /// Set to `null` to clear the due date. Omit to leave unchanged.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub due_date: Option<Option<chrono::NaiveDate>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: i16,
    pub assignee_id: Option<Uuid>,
    pub due_date: Option<chrono::NaiveDate>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Row helper ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    project_id: Uuid,
    title: String,
    description: Option<String>,
    status: String,
    priority: i16,
    assignee_id: Option<Uuid>,
    due_date: Option<chrono::NaiveDate>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl TaskRow {
    fn into_response(self) -> TaskResponse {
        TaskResponse {
            id: self.id,
            project_id: self.project_id,
            title: self.title,
            description: self.description,
            status: self.status,
            priority: self.priority,
            assignee_id: self.assignee_id,
            due_date: self.due_date,
            created_at: self.created_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/projects/{pid}/tasks",
    params(
        ("pid" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "List of tasks", body = Vec<TaskResponse>)
    )
)]
pub async fn list_tasks(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(pid): Path<Uuid>,
) -> Result<Json<Vec<TaskResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let tasks = sqlx::query_as::<_, TaskRow>(
        "SELECT id, project_id, title, description, status, priority, \
         assignee_id, due_date, created_at \
         FROM tasks WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(pid)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        tasks.into_iter().map(TaskRow::into_response).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{pid}/tasks",
    params(
        ("pid" = Uuid, Path, description = "Project ID")
    ),
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "Task created", body = TaskResponse)
    )
)]
pub async fn create_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(pid): Path<Uuid>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(axum::http::StatusCode, Json<TaskResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let task = sqlx::query_as::<_, TaskRow>(
        "INSERT INTO tasks (project_id, title, description, priority, assignee_id, due_date) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, project_id, title, description, status, priority, \
         assignee_id, due_date, created_at",
    )
    .bind(pid)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.priority.unwrap_or(3))
    .bind(req.assignee_id)
    .bind(req.due_date)
    .fetch_one(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(task.into_response())))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{pid}/tasks/{tid}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task detail", body = TaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project or task not found")
    )
)]
pub async fn get_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> Result<Json<TaskResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let task = sqlx::query_as::<_, TaskRow>(
        "SELECT id, project_id, title, description, status, priority, \
         assignee_id, due_date, created_at \
         FROM tasks WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL",
    )
    .bind(tid)
    .bind(pid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".to_string()))?;

    Ok(Json(task.into_response()))
}

#[utoipa::path(
    patch,
    path = "/api/v1/projects/{pid}/tasks/{tid}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID")
    ),
    request_body = UpdateTaskRequest,
    responses(
        (status = 200, description = "Task updated", body = TaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project or task not found")
    )
)]
pub async fn update_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    // Fetch current row so we can distinguish "omitted" from "explicit null"
    let current = sqlx::query_as::<_, TaskRow>(
        "SELECT id, project_id, title, description, status, priority, \
         assignee_id, due_date, created_at \
         FROM tasks WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL",
    )
    .bind(tid)
    .bind(pid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".to_string()))?;

    // Resolve fields: omitted → keep current, explicit value/null → use it
    let title = req.title.unwrap_or(current.title);
    let description = req.description.or(current.description);
    let status = req.status.unwrap_or(current.status);
    let priority = req.priority.unwrap_or(current.priority);
    let assignee_id = match req.assignee_id {
        Some(v) => v,                // Some(None) or Some(Some(id))
        None => current.assignee_id, // omitted → keep current
    };
    let due_date = match req.due_date {
        Some(v) => v,
        None => current.due_date,
    };

    let task = sqlx::query_as::<_, TaskRow>(
        "UPDATE tasks SET \
         title = $1, description = $2, status = $3, priority = $4, \
         assignee_id = $5, due_date = $6, updated_at = NOW() \
         WHERE id = $7 AND project_id = $8 AND deleted_at IS NULL \
         RETURNING id, project_id, title, description, status, priority, \
         assignee_id, due_date, created_at",
    )
    .bind(&title)
    .bind(&description)
    .bind(&status)
    .bind(priority)
    .bind(assignee_id)
    .bind(due_date)
    .bind(tid)
    .bind(pid)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(task.into_response()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{pid}/tasks/{tid}",
    params(
        ("pid" = Uuid, Path, description = "Project ID"),
        ("tid" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 204, description = "Task deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project or task not found")
    )
)]
pub async fn delete_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;
    verify_project_ownership(&state.db, pid, user_id).await?;

    let result = sqlx::query(
        "UPDATE tasks SET deleted_at = NOW() WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL"
    )
    .bind(tid)
    .bind(pid)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Task not found".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_task_request_distinguishes_omitted_null_and_value_patch_fields() {
        let omitted: UpdateTaskRequest = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(omitted.assignee_id, None);
        assert_eq!(omitted.due_date, None);

        let cleared: UpdateTaskRequest = serde_json::from_value(serde_json::json!({
            "assignee_id": null,
            "due_date": null,
        }))
        .unwrap();
        assert_eq!(cleared.assignee_id, Some(None));
        assert_eq!(cleared.due_date, Some(None));

        let assignee_id = Uuid::new_v4();
        let set: UpdateTaskRequest = serde_json::from_value(serde_json::json!({
            "assignee_id": assignee_id.to_string(),
            "due_date": "2026-06-05",
        }))
        .unwrap();
        assert_eq!(set.assignee_id, Some(Some(assignee_id)));
        assert_eq!(
            set.due_date,
            Some(Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()))
        );
    }
}
