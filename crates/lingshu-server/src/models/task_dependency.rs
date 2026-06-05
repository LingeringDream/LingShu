use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskDependency {
    pub id: Uuid,
    pub task_id: Uuid,
    pub depends_on_id: Uuid,
    pub dependency_type: String,
    pub created_at: DateTime<Utc>,
}
