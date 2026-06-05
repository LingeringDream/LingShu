use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// External service integration (Calendar, Slack, etc.).
/// Encrypted tokens are stored in DB but never exposed via API.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Integration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub platform: String,
    #[serde(skip_serializing)]
    pub access_token_encrypted: Vec<u8>,
    #[serde(skip_serializing)]
    pub refresh_token_encrypted: Option<Vec<u8>>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub config: serde_json::Value,
    pub status: String,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
