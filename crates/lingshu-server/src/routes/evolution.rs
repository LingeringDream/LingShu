use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::evolution::EvolutionRun;
#[cfg(test)]
use crate::models::evolution::EvolutionRunStatus;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/evolution/runs", get(list_evolution_runs))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListEvolutionRunsParams {
    pub scope: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EvolutionRunResponse {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub scope: String,
    pub status: String,
    pub evidence_summary: serde_json::Value,
    pub proposed_changes: serde_json::Value,
    pub applied_changes: serde_json::Value,
    pub skipped_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<EvolutionRun> for EvolutionRunResponse {
    fn from(run: EvolutionRun) -> Self {
        Self {
            id: run.id,
            user_id: run.user_id,
            scope: run.scope.as_str().to_string(),
            status: run.status.as_str().to_string(),
            evidence_summary: run.evidence_summary,
            proposed_changes: run.proposed_changes,
            applied_changes: run.applied_changes,
            skipped_reason: run.skipped_reason,
            created_at: run.created_at,
        }
    }
}

fn clamped_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(20).clamp(1, 100)
}

fn normalize_scope(scope: Option<String>) -> Result<Option<String>, AppError> {
    match scope.as_deref() {
        None | Some("") => Ok(None),
        Some("personality") => Ok(Some("personality".to_string())),
        Some(other) => Err(AppError::Validation(format!(
            "Invalid evolution scope '{other}'. Allowed: personality"
        ))),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/evolution/runs",
    params(ListEvolutionRunsParams),
    responses((status = 200, body = Vec<EvolutionRunResponse>))
)]
pub async fn list_evolution_runs(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<ListEvolutionRunsParams>,
) -> Result<Json<Vec<EvolutionRunResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = clamped_limit(params.limit);
    let scope = normalize_scope(params.scope)?;

    let runs: Vec<EvolutionRun> = if let Some(scope) = scope {
        sqlx::query_as(
            "SELECT * FROM evolution_runs \
             WHERE user_id = $1 AND scope = $2 \
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(user_id)
        .bind(scope)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT * FROM evolution_runs \
             WHERE user_id = $1 \
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        runs.into_iter().map(EvolutionRunResponse::from).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_defaults_and_clamps() {
        assert_eq!(clamped_limit(None), 20);
        assert_eq!(clamped_limit(Some(0)), 1);
        assert_eq!(clamped_limit(Some(50)), 50);
        assert_eq!(clamped_limit(Some(500)), 100);
    }

    #[test]
    fn scope_accepts_personality_and_empty_values() {
        assert_eq!(normalize_scope(None).unwrap(), None);
        assert_eq!(normalize_scope(Some(String::new())).unwrap(), None);
        assert_eq!(
            normalize_scope(Some("personality".to_string())).unwrap(),
            Some("personality".to_string())
        );
    }

    #[test]
    fn scope_rejects_unknown_values() {
        let err = normalize_scope(Some("pet".to_string())).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn response_uses_snake_case_strings() {
        let run = EvolutionRun {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            scope: crate::models::evolution::EvolutionScope::Personality,
            status: EvolutionRunStatus::Skipped,
            evidence_summary: serde_json::json!({"memory_count": 0}),
            proposed_changes: serde_json::json!([]),
            applied_changes: serde_json::json!([]),
            skipped_reason: Some("no evidence".to_string()),
            created_at: chrono::Utc::now(),
        };
        let response = EvolutionRunResponse::from(run);
        assert_eq!(response.scope, "personality");
        assert_eq!(response.status, "skipped");
        assert_eq!(response.skipped_reason.as_deref(), Some("no evidence"));
    }
}
