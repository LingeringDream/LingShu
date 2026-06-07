// ── Signal Events Telemetry ────────────────────────────────────────────
//
// Append-only event log for SoulLedger calibration. Every significant
// user interaction records a row in `signal_events`. This module:
//
//   - Defines the `SignalEventType` enum (the only allowed event_type values).
//   - Provides `record()` — fire-and-forget INSERT that never fails the caller.
//   - Provides `detect_explicit_memory_request()` — heuristic for "remember this".
//
// The `/api/v1/signals` endpoint (routes/signals.rs) is the client-facing
// ingestion point. It accepts a restricted subset of event types.
//
// All INSERTs are parameterised and scoped by user_id.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ── Event Type Whitelist ─────────────────────────────────────────────

/// Every signal event type the system recognises.
///
/// Serialised as snake_case strings in JSON and the DB.
/// This enum is the *sole* whitelist — `SignalEventType::try_from_str()`
/// validates any string before an INSERT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalEventType {
    // ── Memory lifecycle ───────────────────────────────────────────
    MemoryExplicitRequest,
    MemoryDedupHit,
    MemoryCreated,
    MemoryRetrievalHit,
    /// Service-side only: a memory was soft-deleted by the background
    /// forgetting sweep because its decayed effective importance fell
    /// below the floor and it was not protected by provenance.
    MemoryForgotten,
    MemoryReferenced,
    MemoryCopied,
    MemoryDisputed,
    /// Service-side only: the offline consolidation engine produced a
    /// derived memory that summarises multiple raw source memories.
    MemoryConsolidated,

    // ── Reply feedback ─────────────────────────────────────────────
    ReplyThumbUp,
    ReplyThumbDown,
    ReplyStyleTag,

    // ── Thought lifecycle ──────────────────────────────────────────
    ThoughtShown,
    ThoughtAccepted,
    ThoughtDismissed,
    ThoughtSnoozed,

    // ── Personality ────────────────────────────────────────────────
    PersonalitySliderChanged,
}

impl SignalEventType {
    /// String representation (snake_case, matches the DB column).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryExplicitRequest => "memory_explicit_request",
            Self::MemoryDedupHit => "memory_dedup_hit",
            Self::MemoryCreated => "memory_created",
            Self::MemoryRetrievalHit => "memory_retrieval_hit",
            Self::MemoryForgotten => "memory_forgotten",
            Self::MemoryReferenced => "memory_referenced",
            Self::MemoryCopied => "memory_copied",
            Self::MemoryDisputed => "memory_disputed",
            Self::MemoryConsolidated => "memory_consolidated",
            Self::ReplyThumbUp => "reply_thumb_up",
            Self::ReplyThumbDown => "reply_thumb_down",
            Self::ReplyStyleTag => "reply_style_tag",
            Self::ThoughtShown => "thought_shown",
            Self::ThoughtAccepted => "thought_accepted",
            Self::ThoughtDismissed => "thought_dismissed",
            Self::ThoughtSnoozed => "thought_snoozed",
            Self::PersonalitySliderChanged => "personality_slider_changed",
        }
    }

    /// Try to parse from a snake_case string. Returns `None` for unknown types.
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "memory_explicit_request" => Some(Self::MemoryExplicitRequest),
            "memory_dedup_hit" => Some(Self::MemoryDedupHit),
            "memory_created" => Some(Self::MemoryCreated),
            "memory_retrieval_hit" => Some(Self::MemoryRetrievalHit),
            "memory_forgotten" => Some(Self::MemoryForgotten),
            "memory_referenced" => Some(Self::MemoryReferenced),
            "memory_copied" => Some(Self::MemoryCopied),
            "memory_disputed" => Some(Self::MemoryDisputed),
            "memory_consolidated" => Some(Self::MemoryConsolidated),
            "reply_thumb_up" => Some(Self::ReplyThumbUp),
            "reply_thumb_down" => Some(Self::ReplyThumbDown),
            "reply_style_tag" => Some(Self::ReplyStyleTag),
            "thought_shown" => Some(Self::ThoughtShown),
            "thought_accepted" => Some(Self::ThoughtAccepted),
            "thought_dismissed" => Some(Self::ThoughtDismissed),
            "thought_snoozed" => Some(Self::ThoughtSnoozed),
            "personality_slider_changed" => Some(Self::PersonalitySliderChanged),
            _ => None,
        }
    }

    /// The subset of event types allowed from the client-facing
    /// `POST /api/v1/signals` endpoint. Service-side-only types are rejected.
    pub fn allowed_from_client(s: &str) -> Option<Self> {
        match Self::try_from_str(s) {
            Some(
                Self::MemoryCopied
                | Self::MemoryDisputed
                | Self::ReplyThumbUp
                | Self::ReplyThumbDown
                | Self::ReplyStyleTag
                | Self::PersonalitySliderChanged,
            ) => Self::try_from_str(s),
            _ => None,
        }
    }
}

// ── Core Recording Function ───────────────────────────────────────────

/// Record a signal event. **Fire-and-forget**: on failure only
/// `tracing::warn!` is emitted — the caller is never interrupted.
///
/// Parameters:
/// - `db`         — database pool
/// - `user_id`    — owning user
/// - `event_type` — must be a valid [`SignalEventType`] variant
/// - `entity_type`— optional tag, e.g. `"memory"`, `"thought"`
/// - `entity_id`  — optional UUID of the associated entity
/// - `metadata`   — optional JSON (counters, labels, trait values — no raw
///   conversation text); defaults to `{}`
pub async fn record(
    db: &PgPool,
    user_id: Uuid,
    event_type: SignalEventType,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
    metadata: serde_json::Value,
) {
    let event_str = event_type.as_str();
    let meta = if metadata.is_null() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        metadata
    };

    let result = sqlx::query(
        "INSERT INTO signal_events (user_id, event_type, entity_type, entity_id, metadata) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(event_str)
    .bind(entity_type)
    .bind(entity_id)
    .bind(&meta)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!(
            event_type = %event_str,
            user_id = %user_id,
            error = %e,
            "Failed to record signal event (non-fatal)"
        );
    }
}

// ── Detection Heuristics ──────────────────────────────────────────────

/// Check whether `text` contains an explicit memory-storage request.
///
/// Matches the following patterns (case-insensitive):
///   - "记住" | "记一下" | "帮我记" | "remember"
///
/// Returns `true` when any pattern is found as a substring.
pub fn detect_explicit_memory_request(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("记住")
        || lower.contains("记一下")
        || lower.contains("帮我记")
        || lower.contains("remember")
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_explicit_memory_request ─────────────────────────────

    #[test]
    fn detect_记住() {
        assert!(detect_explicit_memory_request("请记住我喜欢喝咖啡"));
    }

    #[test]
    fn detect_记一下() {
        assert!(detect_explicit_memory_request("帮我记一下这个会议时间"));
    }

    #[test]
    fn detect_帮我记() {
        assert!(detect_explicit_memory_request("帮我记住明天是妈妈的生日"));
    }

    #[test]
    fn detect_remember_english() {
        assert!(detect_explicit_memory_request("please remember that I prefer tea"));
    }

    #[test]
    fn detect_case_insensitive() {
        assert!(detect_explicit_memory_request("REMEMBER this fact"));
    }

    #[test]
    fn reject_ordinary_text() {
        assert!(!detect_explicit_memory_request("今天天气不错"));
        assert!(!detect_explicit_memory_request("帮我写一封邮件"));
    }

    #[test]
    fn reject_empty() {
        assert!(!detect_explicit_memory_request(""));
    }

    // ── SignalEventType ────────────────────────────────────────────

    #[test]
    fn all_variants_roundtrip_via_str() {
        let variants = [
            SignalEventType::MemoryExplicitRequest,
            SignalEventType::MemoryDedupHit,
            SignalEventType::MemoryCreated,
            SignalEventType::MemoryRetrievalHit,
            SignalEventType::MemoryForgotten,
            SignalEventType::MemoryReferenced,
            SignalEventType::MemoryCopied,
            SignalEventType::MemoryDisputed,
            SignalEventType::ReplyThumbUp,
            SignalEventType::ReplyThumbDown,
            SignalEventType::ReplyStyleTag,
            SignalEventType::ThoughtShown,
            SignalEventType::ThoughtAccepted,
            SignalEventType::ThoughtDismissed,
            SignalEventType::ThoughtSnoozed,
            SignalEventType::PersonalitySliderChanged,
        ];
        for v in &variants {
            let s = v.as_str();
            let parsed = SignalEventType::try_from_str(s);
            assert_eq!(parsed, Some(*v), "roundtrip failed for {s}");
        }
    }

    #[test]
    fn try_from_str_rejects_unknown() {
        assert_eq!(SignalEventType::try_from_str("bogus_event"), None);
        assert_eq!(SignalEventType::try_from_str(""), None);
    }

    #[test]
    fn allowed_from_client_permits_subset() {
        // Allowed
        assert!(SignalEventType::allowed_from_client("memory_copied").is_some());
        assert!(SignalEventType::allowed_from_client("memory_disputed").is_some());
        assert!(SignalEventType::allowed_from_client("reply_thumb_up").is_some());
        assert!(SignalEventType::allowed_from_client("reply_thumb_down").is_some());
        assert!(SignalEventType::allowed_from_client("reply_style_tag").is_some());
        assert!(SignalEventType::allowed_from_client("personality_slider_changed").is_some());

        // Denied (server-side only)
        assert!(SignalEventType::allowed_from_client("memory_explicit_request").is_none());
        assert!(SignalEventType::allowed_from_client("memory_created").is_none());
        assert!(SignalEventType::allowed_from_client("memory_retrieval_hit").is_none());
        assert!(SignalEventType::allowed_from_client("memory_dedup_hit").is_none());
        assert!(SignalEventType::allowed_from_client("thought_shown").is_none());
        assert!(SignalEventType::allowed_from_client("thought_accepted").is_none());
        assert!(SignalEventType::allowed_from_client("thought_dismissed").is_none());
        assert!(SignalEventType::allowed_from_client("thought_snoozed").is_none());
        assert!(SignalEventType::allowed_from_client("memory_referenced").is_none());
        assert!(SignalEventType::allowed_from_client("memory_forgotten").is_none());
    }

    #[test]
    fn allowed_from_client_rejects_unknown() {
        assert!(SignalEventType::allowed_from_client("bogus").is_none());
        assert!(SignalEventType::allowed_from_client("").is_none());
    }
}
