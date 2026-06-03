use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{pid}/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/projects/{pid}/tasks/{tid}",
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
    Path(pid): Path<Uuid>,
) -> Result<Json<Vec<TaskResponse>>, AppError> {
    let tasks = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, i16, Option<Uuid>, Option<chrono::NaiveDate>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, project_id, title, description, status, priority, assignee_id, due_date, created_at FROM tasks WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(pid)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        tasks
            .into_iter()
            .map(|t| TaskResponse {
                id: t.0,
                project_id: t.1,
                title: t.2,
                description: t.3,
                status: t.4,
                priority: t.5,
                assignee_id: t.6,
                due_date: t.7,
                created_at: t.8,
            })
            .collect(),
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
    Path(pid): Path<Uuid>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(axum::http::StatusCode, Json<TaskResponse>), AppError> {
    let task = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, i16, Option<Uuid>, Option<chrono::NaiveDate>, chrono::DateTime<chrono::Utc>)>(
        "INSERT INTO tasks (project_id, title, description, priority, assignee_id, due_date) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, project_id, title, description, status, priority, assignee_id, due_date, created_at"
    )
    .bind(pid)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.priority.unwrap_or(3))
    .bind(req.assignee_id)
    .bind(req.due_date)
    .fetch_one(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(TaskResponse {
            id: task.0,
            project_id: task.1,
            title: task.2,
            description: task.3,
            status: task.4,
            priority: task.5,
            assignee_id: task.6,
            due_date: task.7,
            created_at: task.8,
        }),
    ))
}

pub async fn get_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> Result<Json<TaskResponse>, AppError> {
    let task = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, i16, Option<Uuid>, Option<chrono::NaiveDate>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, project_id, title, description, status, priority, assignee_id, due_date, created_at FROM tasks WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL"
    )
    .bind(tid)
    .bind(pid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".to_string()))?;

    Ok(Json(TaskResponse {
        id: task.0,
        project_id: task.1,
        title: task.2,
        description: task.3,
        status: task.4,
        priority: task.5,
        assignee_id: task.6,
        due_date: task.7,
        created_at: task.8,
    }))
}

pub async fn update_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<TaskResponse>, AppError> {
    let task = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, i16, Option<Uuid>, Option<chrono::NaiveDate>, chrono::DateTime<chrono::Utc>)>(
        "UPDATE tasks SET title = $1, description = $2, priority = COALESCE($3, priority), assignee_id = $4, due_date = $5, updated_at = NOW() WHERE id = $6 AND project_id = $7 AND deleted_at IS NULL RETURNING id, project_id, title, description, status, priority, assignee_id, due_date, created_at"
    )
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.priority)
    .bind(req.assignee_id)
    .bind(req.due_date)
    .bind(tid)
    .bind(pid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".to_string()))?;

    Ok(Json(TaskResponse {
        id: task.0,
        project_id: task.1,
        title: task.2,
        description: task.3,
        status: task.4,
        priority: task.5,
        assignee_id: task.6,
        due_date: task.7,
        created_at: task.8,
    }))
}

pub async fn delete_task(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((pid, tid)): Path<(Uuid, Uuid)>,
) -> Result<axum::http::StatusCode, AppError> {
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
