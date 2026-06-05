use sqlx::PgPool;
use uuid::Uuid;

use crate::models::project::Project;

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_by_owner(pool: &PgPool, owner_id: Uuid) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}
