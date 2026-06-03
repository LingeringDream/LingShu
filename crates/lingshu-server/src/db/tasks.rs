use sqlx::PgPool;
use uuid::Uuid;

use crate::models::task::Task;

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Task>, sqlx::Error> {
    sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_by_project(pool: &PgPool, project_id: Uuid) -> Result<Vec<Task>, sqlx::Error> {
    sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}
