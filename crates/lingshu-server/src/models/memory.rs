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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tier_is_raw() {
        assert_eq!(default_tier(), "raw");
    }

    #[test]
    fn memory_importance_defaults_to_0() {
        // When no importance is set, it should default to 0.0
        let json = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "user_id": "550e8400-e29b-41d4-a716-446655440001",
            "memory_type": "fact",
            "content": "test",
            "importance": 0.0,
            "access_count": 0,
            "metadata": {},
            "tier": "raw",
            "created_at": "2026-06-01T00:00:00Z",
            "updated_at": "2026-06-01T00:00:00Z"
        });
        let m: Memory = serde_json::from_value(json).unwrap();
        assert_eq!(m.memory_type, "fact");
        assert_eq!(m.content, "test");
    }
}
