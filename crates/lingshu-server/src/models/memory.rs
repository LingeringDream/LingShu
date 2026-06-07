use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Memory {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
    pub access_count: i32,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub vector_id: Option<String>,
    /// Memories that this entry was derived from (only populated when tier='derived').
    #[serde(default)]
    pub source_memory_ids: Vec<Uuid>,
    /// Memory tier: 'raw' for original episodic memories, 'derived' for LLM-consolidated summaries.
    #[serde(default = "default_tier")]
    pub tier: String,
    pub metadata: serde_json::Value,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_tier() -> String {
    "raw".to_string()
}
