use axum::{extract::Path, routing::get, Json, Router};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::thought::Thought;
use crate::routes::settings::llm_settings_for_user;
use crate::state::AppState;

/// Duration after which a snoozed thought resurfaces (3 days).
const SNOOZE_DURATION: Duration = Duration::days(3);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/thoughts", get(list_thoughts))
        .route(
            "/api/v1/thoughts/:id",
            get(get_thought).patch(update_thought),
        )
        .route(
            "/api/v1/thoughts/generate",
            axum::routing::post(generate_thoughts),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListThoughtsParams {
    pub status: Option<String>,
    /// When `true`, only return thoughts whose `scheduled_at` is NULL or in the past
    /// (hiding unripe snoozed thoughts). Default behaviour (active absent or false)
    /// shows all thoughts regardless of schedule.
    pub active: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateThoughtRequest {
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThoughtResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub detail: Option<String>,
    pub reason: Option<String>,
    pub confidence: f32,
    pub source_memory_ids: Vec<Uuid>,
    pub requires_confirmation: bool,
    pub status: String,
    pub shown_at: Option<chrono::DateTime<chrono::Utc>>,
    pub scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Thought> for ThoughtResponse {
    fn from(t: Thought) -> Self {
        Self {
            id: t.id,
            user_id: t.user_id,
            title: t.title,
            detail: t.detail,
            reason: t.reason,
            confidence: t.confidence,
            source_memory_ids: t.source_memory_ids,
            requires_confirmation: t.requires_confirmation,
            status: t.status.as_str().to_string(),
            shown_at: t.shown_at,
            scheduled_at: t.scheduled_at,
            resolved_at: t.resolved_at,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/thoughts",
    params(
        ("status" = Option<String>, Query, description = "Filter by status"),
        ("limit" = Option<i64>, Query, description = "Max results")
    ),
    responses((status = 200, body = Vec<ThoughtResponse>))
)]
async fn list_thoughts(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<ListThoughtsParams>,
) -> Result<Json<Vec<ThoughtResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = params.limit.unwrap_or(50).min(200);

    // User-scoped, with optional status filter and optional "active" filter
    // (hide future-scheduled snoozed thoughts when active=true).
    let base_sql = "SELECT * FROM thought_queue WHERE user_id = $1";

    let thoughts: Vec<Thought> = if let Some(st) = &params.status {
        sqlx::query_as(&format!(
            "{base_sql} AND status = $2 ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $3"
        ))
        .bind(user_id)
        .bind(st)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else if params.active.unwrap_or(false) {
        sqlx::query_as(&format!(
            "{base_sql} AND (scheduled_at IS NULL OR scheduled_at <= NOW()) \
             ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2"
        ))
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(&format!(
            "{base_sql} ORDER BY scheduled_at ASC NULLS FIRST, created_at DESC LIMIT $2"
        ))
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(
        thoughts.into_iter().map(ThoughtResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/thoughts/{id}",
    params(("id" = Uuid, Path, description = "Thought ID")),
    responses((status = 200, body = ThoughtResponse))
)]
async fn get_thought(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ThoughtResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let thought: Thought =
        sqlx::query_as("SELECT * FROM thought_queue WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Thought not found".to_string()))?;

    Ok(Json(ThoughtResponse::from(thought)))
}

// ── State machine ──────────────────────────────────────────────────

/// Validate a thought status transition against the lifecycle state machine:
///
/// ```text
/// pending → shown | accepted | dismissed | snoozed
/// shown   → accepted | dismissed | snoozed
/// (accepted, dismissed, expired = terminal; snoozed → pending is system-only)
/// ```
fn next_status_is_valid(from: &str, to: &str) -> bool {
    match from {
        "pending" => matches!(to, "shown" | "accepted" | "dismissed" | "snoozed"),
        "shown" => matches!(to, "accepted" | "dismissed" | "snoozed"),
        _ => false, // terminal states + snoozed (system-only transition)
    }
}

#[utoipa::path(
    patch,
    path = "/api/v1/thoughts/{id}",
    params(("id" = Uuid, Path, description = "Thought ID")),
    request_body = UpdateThoughtRequest,
    responses((status = 200, body = ThoughtResponse))
)]
async fn update_thought(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateThoughtRequest>,
) -> Result<Json<ThoughtResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Canonicalise the target status; "confirmed" is a legacy alias for "accepted".
    let raw = match req.status.as_deref() {
        Some(s) => s,
        None => return Err(AppError::BadRequest("status field is required".to_string())),
    };
    let new_status = match raw {
        "shown" | "accepted" | "dismissed" | "snoozed" => raw.to_string(),
        "confirmed" => "accepted".to_string(), // legacy compat
        other => {
            return Err(AppError::Validation(format!(
                "Invalid status '{other}'. Allowed: shown, accepted, dismissed, snoozed"
            )))
        }
    };

    // Fetch current thought
    let current: Thought =
        sqlx::query_as("SELECT * FROM thought_queue WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Thought not found".to_string()))?;

    let current_status = current.status.as_str();

    // Validate transition
    if !next_status_is_valid(current_status, &new_status) {
        return Err(AppError::Validation(format!(
            "Cannot transition thought from '{current_status}' to '{new_status}'. \
             Allowed transitions: pending→shown|accepted|dismissed|snoozed, \
             shown→accepted|dismissed|snoozed"
        )));
    }

    // Build UPDATE based on target status (timestamp semantics differ per status)
    let thought: Thought = match new_status.as_str() {
        "shown" => {
            sqlx::query_as(
                "UPDATE thought_queue SET status = $1, shown_at = COALESCE(shown_at, NOW()), \
                 updated_at = NOW() WHERE id = $2 AND user_id = $3 RETURNING *",
            )
            .bind(&new_status)
            .bind(id)
            .bind(user_id)
            .fetch_one(&state.db)
            .await?
        }
        "accepted" | "dismissed" => {
            sqlx::query_as(
                "UPDATE thought_queue SET status = $1, resolved_at = NOW(), \
                 updated_at = NOW() WHERE id = $2 AND user_id = $3 RETURNING *",
            )
            .bind(&new_status)
            .bind(id)
            .bind(user_id)
            .fetch_one(&state.db)
            .await?
        }
        "snoozed" => {
            let snooze_until = chrono::Utc::now() + SNOOZE_DURATION;
            sqlx::query_as(
                "UPDATE thought_queue SET status = $1, scheduled_at = $2, \
                 updated_at = NOW() WHERE id = $3 AND user_id = $4 RETURNING *",
            )
            .bind(&new_status)
            .bind(snooze_until)
            .bind(id)
            .bind(user_id)
            .fetch_one(&state.db)
            .await?
        }
        _ => unreachable!("validated by next_status_is_valid"),
    };

    // Signal: map status to thought lifecycle event
    let event_type = match new_status.as_str() {
        "shown" => crate::telemetry::SignalEventType::ThoughtShown,
        "accepted" => crate::telemetry::SignalEventType::ThoughtAccepted,
        "dismissed" => crate::telemetry::SignalEventType::ThoughtDismissed,
        "snoozed" => crate::telemetry::SignalEventType::ThoughtSnoozed,
        _ => return Ok(Json(ThoughtResponse::from(thought))),
    };

    crate::telemetry::record(
        &state.db,
        user_id,
        event_type,
        Some("thought"),
        Some(thought.id),
        serde_json::json!({"previous_status": current_status}),
    )
    .await;

    Ok(Json(ThoughtResponse::from(thought)))
}

// ── Handler: generate ─────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateThoughtsResponse {
    /// Number of new thoughts created (0-3)
    pub created: usize,
}

#[utoipa::path(
    post,
    path = "/api/v1/thoughts/generate",
    responses(
        (status = 200, description = "Thoughts generated", body = GenerateThoughtsResponse),
        (status = 401, description = "Unauthorized")
    )
)]
async fn generate_thoughts(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<GenerateThoughtsResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let settings = llm_settings_for_user(&state, user_id).await;

    if settings.model.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!("Model not configured.")));
    }

    let created = crate::llm::thoughts::generate_and_save_thoughts(
        &state.db,
        &state.llm,
        &settings.model,
        user_id,
    )
    .await
    .map_err(AppError::Internal)?;

    if created > 0 {
        let _ = state
            .pet_notifications
            .send(crate::state::PetNotification::new(
                "thought",
                "灵枢有新的想法",
                format!("产生了 {created} 条新建议"),
            ));
    }

    Ok(Json(GenerateThoughtsResponse { created }))
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── next_status_is_valid ──────────────────────────────────────

    #[test]
    fn pending_to_shown_is_valid() {
        assert!(next_status_is_valid("pending", "shown"));
    }

    #[test]
    fn pending_to_accepted_is_valid() {
        assert!(next_status_is_valid("pending", "accepted"));
    }

    #[test]
    fn pending_to_dismissed_is_valid() {
        assert!(next_status_is_valid("pending", "dismissed"));
    }

    #[test]
    fn pending_to_snoozed_is_valid() {
        assert!(next_status_is_valid("pending", "snoozed"));
    }

    #[test]
    fn pending_to_expired_is_invalid() {
        assert!(!next_status_is_valid("pending", "expired"));
    }

    #[test]
    fn pending_to_pending_is_invalid() {
        assert!(!next_status_is_valid("pending", "pending"));
    }

    #[test]
    fn shown_to_accepted_is_valid() {
        assert!(next_status_is_valid("shown", "accepted"));
    }

    #[test]
    fn shown_to_dismissed_is_valid() {
        assert!(next_status_is_valid("shown", "dismissed"));
    }

    #[test]
    fn shown_to_snoozed_is_valid() {
        assert!(next_status_is_valid("shown", "snoozed"));
    }

    #[test]
    fn shown_to_shown_is_invalid() {
        assert!(!next_status_is_valid("shown", "shown"));
    }

    #[test]
    fn shown_to_pending_is_invalid() {
        assert!(!next_status_is_valid("shown", "pending"));
    }

    #[test]
    fn accepted_is_terminal() {
        assert!(!next_status_is_valid("accepted", "shown"));
        assert!(!next_status_is_valid("accepted", "pending"));
        assert!(!next_status_is_valid("accepted", "accepted"));
        assert!(!next_status_is_valid("accepted", "dismissed"));
        assert!(!next_status_is_valid("accepted", "snoozed"));
        assert!(!next_status_is_valid("accepted", "expired"));
    }

    #[test]
    fn dismissed_is_terminal() {
        assert!(!next_status_is_valid("dismissed", "shown"));
        assert!(!next_status_is_valid("dismissed", "pending"));
        assert!(!next_status_is_valid("dismissed", "accepted"));
        assert!(!next_status_is_valid("dismissed", "dismissed"));
        assert!(!next_status_is_valid("dismissed", "snoozed"));
        assert!(!next_status_is_valid("dismissed", "expired"));
    }

    #[test]
    fn expired_is_terminal() {
        assert!(!next_status_is_valid("expired", "shown"));
        assert!(!next_status_is_valid("expired", "pending"));
    }

    #[test]
    fn snoozed_is_system_only() {
        assert!(!next_status_is_valid("snoozed", "pending"));
        assert!(!next_status_is_valid("snoozed", "shown"));
        assert!(!next_status_is_valid("snoozed", "accepted"));
    }
}
