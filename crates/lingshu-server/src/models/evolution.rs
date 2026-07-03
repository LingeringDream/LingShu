use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[serde(rename_all = "snake_case")]
pub enum EvolutionScope {
    #[sqlx(rename = "personality")]
    Personality,
}

impl EvolutionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Personality => "personality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[serde(rename_all = "snake_case")]
pub enum EvolutionRunStatus {
    #[sqlx(rename = "applied")]
    Applied,
    #[sqlx(rename = "skipped")]
    Skipped,
    #[sqlx(rename = "failed")]
    Failed,
    #[sqlx(rename = "review_required")]
    ReviewRequired,
}

impl EvolutionRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::ReviewRequired => "review_required",
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct EvolutionRun {
    pub id: Uuid,
    pub user_id: Uuid,
    pub scope: EvolutionScope,
    pub status: EvolutionRunStatus,
    pub evidence_summary: serde_json::Value,
    pub proposed_changes: serde_json::Value,
    pub applied_changes: serde_json::Value,
    pub skipped_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_serializes_to_snake_case() {
        assert_eq!(EvolutionScope::Personality.as_str(), "personality");
        assert_eq!(
            serde_json::to_string(&EvolutionScope::Personality).unwrap(),
            "\"personality\""
        );
    }

    #[test]
    fn run_status_serializes_to_snake_case() {
        assert_eq!(EvolutionRunStatus::Applied.as_str(), "applied");
        assert_eq!(EvolutionRunStatus::Skipped.as_str(), "skipped");
        assert_eq!(EvolutionRunStatus::Failed.as_str(), "failed");
        assert_eq!(
            EvolutionRunStatus::ReviewRequired.as_str(),
            "review_required"
        );
    }
}
