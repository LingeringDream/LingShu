use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/audit-log", get(list_entries))
}

/// Write an immutable audit-trail entry. Best-effort: a DB failure is logged
/// but never propagated, so auditing can't break the action it records.
/// `action`/`resource_type` are capped at 50 chars by the schema.
pub async fn record(
    db: &sqlx::PgPool,
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
) {
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_log (user_id, action, resource_type, resource_id, details) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .execute(db)
    .await
    {
        tracing::warn!(%user_id, action, resource_type, %e, "failed to write audit_log entry");
    }
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuditParams {
    pub resource_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEntryResponse {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Handler ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/audit-log",
    params(
        ("resource_type" = Option<String>, Query, description = "Filter by resource type"),
        ("limit" = Option<i64>, Query, description = "Max results (default 50)"),
        ("offset" = Option<i64>, Query, description = "Pagination offset")
    ),
    responses((status = 200, body = Vec<AuditEntryResponse>))
)]
async fn list_entries(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<AuditParams>,
) -> Result<Json<Vec<AuditEntryResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let rows: Vec<AuditRow> = if let Some(rt) = &params.resource_type {
        sqlx::query_as(
            "SELECT id, user_id, action, resource_type, resource_id, details, \
             CAST(ip_address AS TEXT) AS ip_address, created_at \
             FROM audit_log WHERE user_id = $1 AND resource_type = $2 \
             ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        )
        .bind(user_id)
        .bind(rt)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, user_id, action, resource_type, resource_id, details, \
             CAST(ip_address AS TEXT) AS ip_address, created_at \
             FROM audit_log WHERE user_id = $1 \
             ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        rows.into_iter().map(AuditRow::into_response).collect(),
    ))
}

// ── Row helper ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct AuditRow {
    id: Uuid,
    user_id: Option<Uuid>,
    action: String,
    resource_type: String,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
    ip_address: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl AuditRow {
    fn into_response(self) -> AuditEntryResponse {
        AuditEntryResponse {
            id: self.id,
            user_id: self.user_id,
            action: self.action,
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            details: self.details,
            ip_address: self.ip_address,
            created_at: self.created_at,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn audit_params_defaults() {
        let json = serde_json::json!({});
        let params: AuditParams = serde_json::from_value(json).unwrap();
        assert!(params.resource_type.is_none());
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
    }

    #[test]
    fn audit_params_with_filters() {
        let json =
            serde_json::json!({"resource_type": "calendar_event", "limit": 50, "offset": 10});
        let params: AuditParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.resource_type.unwrap(), "calendar_event");
        assert_eq!(params.limit.unwrap(), 50);
        assert_eq!(params.offset.unwrap(), 10);
    }

    #[test]
    fn audit_entry_response_serialization() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let now = Utc::now();
        let resp = AuditEntryResponse {
            id,
            user_id: Some(uuid::Uuid::new_v4()),
            action: "delete".into(),
            resource_type: "calendar_event".into(),
            resource_id: Some(uuid::Uuid::new_v4()),
            details: serde_json::json!({"title": "Meeting"}),
            ip_address: Some("127.0.0.1".into()),
            created_at: now,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["action"], "delete");
        assert_eq!(json["resource_type"], "calendar_event");
    }
}
