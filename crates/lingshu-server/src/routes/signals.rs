// ── Client Signal Ingestion Endpoint ────────────────────────────────────
//
// POST /api/v1/signals
//
// Accepts a restricted subset of [`SignalEventType`] variants from the
// frontend. Service-only types (e.g. memory_created) are rejected with
// 422 Unprocessable Entity.
//
// Metadata conventions for specific event types:
//   reply_style_tag        → { "tag": "too_long" | "too_short" | "too_formal" }
//   personality_slider_changed → { "trait": "warmth", "from": 0.5, "to": 0.6 }

use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;
use crate::telemetry;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/signals", post(ingest_signal))
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignalIngestRequest {
    /// Must be a valid client-allowed signal event type (snake_case string).
    pub event_type: String,

    /// Optional entity tag, e.g. "memory", "thought".
    #[serde(default)]
    pub entity_type: Option<String>,

    /// Optional UUID of the associated entity.
    #[serde(default)]
    pub entity_id: Option<Uuid>,

    /// Optional metadata. Shape depends on event_type (see per-type docs).
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

// ── Validators ─────────────────────────────────────────────────────

/// Validate `reply_style_tag` metadata shape: `{ "tag": "too_long" | "too_short" | "too_formal" }`.
fn validate_reply_style_tag_metadata(meta: &serde_json::Value) -> Result<(), AppError> {
    let tag = meta
        .get("tag")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::Validation(
                "reply_style_tag requires metadata.tag to be one of: too_long, too_short, too_formal"
                    .into(),
            )
        })?;

    match tag {
        "too_long" | "too_short" | "too_formal" => Ok(()),
        other => Err(AppError::Validation(format!(
            "Invalid reply_style_tag value '{other}'. Allowed: too_long, too_short, too_formal"
        ))),
    }
}

/// Validate `personality_slider_changed` metadata shape:
/// `{ "trait": string, "from": number, "to": number }`.
fn validate_personality_slider_metadata(meta: &serde_json::Value) -> Result<(), AppError> {
    let trait_name = meta.get("trait").and_then(|v| v.as_str()).ok_or_else(|| {
        AppError::Validation(
            "personality_slider_changed requires metadata.trait as a string".into(),
        )
    })?;

    if trait_name.is_empty() {
        return Err(AppError::Validation("metadata.trait must not be empty".into()));
    }

    let _from = meta.get("from").and_then(|v| v.as_f64()).ok_or_else(|| {
        AppError::Validation(
            "personality_slider_changed requires metadata.from as a number".into(),
        )
    })?;

    let _to = meta.get("to").and_then(|v| v.as_f64()).ok_or_else(|| {
        AppError::Validation(
            "personality_slider_changed requires metadata.to as a number".into(),
        )
    })?;

    Ok(())
}

// ── Handler ────────────────────────────────────────────────────────

/// Ingest a client-originated signal event.
///
/// Only a restricted subset of event types is accepted (see
/// [`telemetry::SignalEventType::allowed_from_client`]).
/// Service-only types return 422.
///
/// Metadata is validated per event_type conventions.
#[utoipa::path(
    post,
    path = "/api/v1/signals",
    request_body = SignalIngestRequest,
    responses(
        (status = 204, description = "Signal recorded"),
        (status = 401, description = "Unauthorized"),
        (status = 422, description = "Unknown or client-disallowed event type"),
    )
)]
async fn ingest_signal(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<SignalIngestRequest>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_id = auth::require_user(auth).await?;

    // 1. Validate event_type is in the client-allowed subset
    let event_type = telemetry::SignalEventType::allowed_from_client(&req.event_type)
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Unknown or client-disallowed signal event type: '{}'",
                req.event_type
            ))
        })?;

    let meta = req.metadata.unwrap_or(serde_json::Value::Null);

    // 2. Validate per-type metadata conventions
    match event_type {
        telemetry::SignalEventType::ReplyStyleTag => {
            validate_reply_style_tag_metadata(&meta)?;
        }
        telemetry::SignalEventType::PersonalitySliderChanged => {
            validate_personality_slider_metadata(&meta)?;
        }
        _ => {} // no metadata constraints for other client types
    }

    // 3. Fire-and-forget record
    telemetry::record(
        &state.db,
        user_id,
        event_type,
        req.entity_type.as_deref(),
        req.entity_id,
        meta,
    )
    .await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Metadata validation ───────────────────────────────────────

    #[test]
    fn reply_style_tag_valid_values() {
        for tag in &["too_long", "too_short", "too_formal"] {
            assert!(
                validate_reply_style_tag_metadata(&json!({"tag": tag})).is_ok(),
                "tag '{tag}' should be valid"
            );
        }
    }

    #[test]
    fn reply_style_tag_invalid_value() {
        let err = validate_reply_style_tag_metadata(&json!({"tag": "too_verbose"})).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn reply_style_tag_missing_tag() {
        let err = validate_reply_style_tag_metadata(&json!({})).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn personality_slider_valid() {
        assert!(
            validate_personality_slider_metadata(&json!({
                "trait": "warmth", "from": 0.5, "to": 0.6
            }))
            .is_ok()
        );
    }

    #[test]
    fn personality_slider_missing_trait() {
        let err =
            validate_personality_slider_metadata(&json!({"from": 0.5, "to": 0.6})).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn personality_slider_missing_from() {
        let err =
            validate_personality_slider_metadata(&json!({"trait": "warmth", "to": 0.6}))
                .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn personality_slider_empty_trait() {
        let err = validate_personality_slider_metadata(&json!({
            "trait": "", "from": 0.5, "to": 0.6
        }))
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
