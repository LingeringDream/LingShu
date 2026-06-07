//! Thought Queue generation engine.
//!
//! Periodically triggered (per-user cooldown) after chat messages, this
//! module gathers recent context, active goals, and pending tasks, then
//! asks the LLM for 0-3 proactive suggestions.  Candidates are filtered,
//! deduplicated, and inserted into `thought_queue` with status `pending`.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::llm::dedup::{is_duplicate, DEDUP_SIMILARITY_THRESHOLD};
use crate::llm::prompts::thought_queue_prompt;
use sqlx::PgPool;

/// Minimum seconds between thought generation calls per user.
const THOUGHT_COOLDOWN_SECS: u64 = 600; // 10 minutes

/// Maximum new thoughts to insert per generation round.
const MAX_NEW_THOUGHTS: usize = 3;

/// Minimum confidence to persist a candidate.
const MIN_CONFIDENCE: f32 = 0.55;

/// Max active (pending + shown) thoughts before generation is suppressed.
const MAX_ACTIVE_THOUGHTS: usize = 5;

/// Recently dismissed thoughts with the same/similar title are suppressed
/// for this many days to prevent immediate regeneration.
const SUPPRESS_DISMISSED_DAYS: i32 = 14;

// ── Cooldown tracking ─────────────────────────────────────────────

static LAST_GENERATIONS: OnceLock<Mutex<HashMap<Uuid, u64>>> = OnceLock::new();

pub fn should_generate_thoughts(user_id: Uuid) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut map = last_generations()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Some(last) = map.get(&user_id) {
        if now.saturating_sub(*last) < THOUGHT_COOLDOWN_SECS {
            return false;
        }
    }

    map.insert(user_id, now);
    true
}

fn last_generations() -> &'static Mutex<HashMap<Uuid, u64>> {
    LAST_GENERATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── Candidate type ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ThoughtCandidate {
    title: String,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    confidence: f32,
    #[serde(default)]
    source_memory_ids: Vec<Uuid>,
    #[serde(default = "default_requires_confirmation")]
    requires_confirmation: bool,
}

fn default_requires_confirmation() -> bool {
    true
}

// ── Public API ────────────────────────────────────────────────────

/// Generate and persist thought queue suggestions for a user.
/// Returns the number of new thoughts inserted (0-3).
pub async fn generate_and_save_thoughts(
    db: &PgPool,
    llm: &LlmClient,
    model: &str,
    user_id: Uuid,
) -> anyhow::Result<usize> {
    // 0. Active cap: don't generate if the queue is already full
    let active_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM thought_queue \
         WHERE user_id = $1 AND status IN ('pending', 'shown')",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;

    if !active_count_allows_more(active_count.0 as usize, MAX_ACTIVE_THOUGHTS) {
        tracing::info!(
            %user_id,
            active = active_count.0,
            max = MAX_ACTIVE_THOUGHTS,
            "Skipping thought generation: active cap reached"
        );
        return Ok(0);
    }

    // 1. Gather context
    let recent_context = gather_recent_context(db, user_id).await?;
    let active_goals = gather_active_goals(db, user_id).await?;
    let pending_tasks = gather_pending_tasks(db, user_id).await?;
    let now = chrono::Utc::now().to_rfc3339();

    // 2. Build prompt and call LLM
    let prompt = thought_queue_prompt(&recent_context, &active_goals, &pending_tasks, &now);
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    let response = llm.chat(model, messages, None).await?;

    // 3. Parse candidates
    let candidates = parse_thought_candidates(&response)?;

    // 4. Filter, dedup, and insert (max 3)
    let mut inserted = 0usize;
    for c in candidates {
        // Clamp confidence
        let confidence = c.confidence.clamp(0.0, 1.0);
        if c.title.trim().is_empty() || confidence < MIN_CONFIDENCE {
            continue;
        }
        if inserted >= MAX_NEW_THOUGHTS {
            break;
        }
        // Dedup: skip if same/very-similar title exists in pending/shown
        if check_duplicate_thought(db, user_id, &c.title).await? {
            continue;
        }
        insert_thought(
            db,
            user_id,
            &c.title,
            &c.detail,
            &c.reason,
            confidence,
            &c.source_memory_ids,
            c.requires_confirmation,
        )
        .await?;

        // Signal: memory_referenced for each source memory
        for memory_id in &c.source_memory_ids {
            crate::telemetry::record(
                db,
                user_id,
                crate::telemetry::SignalEventType::MemoryReferenced,
                Some("memory"),
                Some(*memory_id),
                serde_json::json!({"source": "thought"}),
            )
            .await;
        }

        inserted += 1;
    }

    if inserted > 0 {
        tracing::info!(%user_id, inserted, "Generated new thought queue suggestions");
    }

    Ok(inserted)
}

// ── Context gathering ─────────────────────────────────────────────

async fn gather_recent_context(db: &PgPool, user_id: Uuid) -> anyhow::Result<String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT m.role, m.content FROM messages m \
         JOIN conversations c ON m.conversation_id = c.id \
         WHERE c.user_id = $1 AND c.deleted_at IS NULL \
         ORDER BY m.created_at DESC LIMIT 10",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    let lines: Vec<String> = rows
        .into_iter()
        .rev() // chronological order
        .map(|(role, content)| format!("[{role}] {content}"))
        .collect();

    Ok(if lines.is_empty() {
        "（暂无最近对话）".to_string()
    } else {
        lines.join("\n")
    })
}

async fn gather_active_goals(db: &PgPool, user_id: Uuid) -> anyhow::Result<String> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM memories \
         WHERE user_id = $1 AND memory_type = 'goal' AND deleted_at IS NULL \
         ORDER BY importance DESC, updated_at DESC LIMIT 5",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(if rows.is_empty() {
        "（无活跃目标）".to_string()
    } else {
        rows.into_iter()
            .enumerate()
            .map(|(i, g)| format!("{}. {}", i + 1, g))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

async fn gather_pending_tasks(db: &PgPool, user_id: Uuid) -> anyhow::Result<String> {
    // Tasks from projects owned by this user, not done/completed.
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT t.title, t.status FROM tasks t \
         JOIN projects p ON t.project_id = p.id \
         WHERE p.owner_id = $1 \
           AND t.status NOT IN ('done', 'completed') \
           AND t.deleted_at IS NULL \
           AND p.deleted_at IS NULL \
         ORDER BY t.due_date ASC NULLS LAST LIMIT 10",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(if rows.is_empty() {
        "（无待完成任务）".to_string()
    } else {
        rows.into_iter()
            .map(|(title, status)| format!("- [{status}] {title}"))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

// ── Parsing ───────────────────────────────────────────────────────

/// Parse the LLM response for a JSON array of thought candidates.
fn parse_thought_candidates(raw: &str) -> anyhow::Result<Vec<ThoughtCandidate>> {
    let text = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let slice = match (text.find('['), text.rfind(']')) {
        (Some(start), Some(end)) if start < end => &text[start..=end],
        _ => text,
    };

    let candidates: Vec<ThoughtCandidate> = serde_json::from_str(slice)?;
    Ok(candidates)
}

// ── Dedup ─────────────────────────────────────────────────────────

async fn check_duplicate_thought(
    db: &PgPool,
    user_id: Uuid,
    title: &str,
) -> Result<bool, sqlx::Error> {
    // 1. Check against currently active (pending/shown) thoughts
    let active: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, title FROM thought_queue \
         WHERE user_id = $1 AND status IN ('pending', 'shown') \
         ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    for (_id, existing_title) in &active {
        if existing_title == title
            || is_duplicate(existing_title, title, DEDUP_SIMILARITY_THRESHOLD)
        {
            return Ok(true);
        }
    }

    // 2. Suppress if recently dismissed with same/similar title
    let dismissed: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, title FROM thought_queue \
         WHERE user_id = $1 AND status = 'dismissed' \
           AND resolved_at > NOW() - ($2::integer || ' days')::INTERVAL \
         ORDER BY resolved_at DESC LIMIT 50",
    )
    .bind(user_id)
    .bind(SUPPRESS_DISMISSED_DAYS)
    .fetch_all(db)
    .await?;

    if is_recently_dismissed(&dismissed, title) {
        tracing::debug!(
            %user_id, %title,
            "Suppressed thought generation: similar title was recently dismissed"
        );
        return Ok(true);
    }

    Ok(false)
}

/// Pure predicate: check whether a candidate title matches any recently-dismissed
/// thought (exact or semantic similarity).
fn is_recently_dismissed(dismissed: &[(Uuid, String)], title: &str) -> bool {
    for (_id, dismissed_title) in dismissed {
        if dismissed_title == title
            || is_duplicate(dismissed_title, title, DEDUP_SIMILARITY_THRESHOLD)
        {
            return true;
        }
    }
    false
}

// ── Insert ────────────────────────────────────────────────────────

async fn insert_thought(
    db: &PgPool,
    user_id: Uuid,
    title: &str,
    detail: &Option<String>,
    reason: &Option<String>,
    confidence: f32,
    source_memory_ids: &[Uuid],
    requires_confirmation: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO thought_queue \
         (user_id, title, detail, reason, confidence, source_memory_ids, requires_confirmation, status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')",
    )
    .bind(user_id)
    .bind(title)
    .bind(detail)
    .bind(reason)
    .bind(confidence)
    .bind(source_memory_ids)
    .bind(requires_confirmation)
    .execute(db)
    .await?;
    Ok(())
}

// ── Pure predicates ────────────────────────────────────────────────

/// Whether the active thought count allows more suggestions.
fn active_count_allows_more(count: usize, max: usize) -> bool {
    count < max
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_blocks_second_call_within_window() {
        let user = Uuid::new_v4();
        // First call should pass
        assert!(should_generate_thoughts(user));
        // Second call within cooldown should fail
        assert!(!should_generate_thoughts(user));
    }

    #[test]
    fn cooldown_is_per_user() {
        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();
        assert!(should_generate_thoughts(user_a));
        assert!(should_generate_thoughts(user_b));
        assert!(!should_generate_thoughts(user_a));
    }

    #[test]
    fn parse_json_array_with_fence() {
        let raw = r#"```json
[{"title":"建议创建日程","detail":"明天下午开会","reason":"用户提到需要开会","confidence":0.85,"source_memory_ids":[],"requires_confirmation":true}]
```"#;
        let candidates = parse_thought_candidates(raw).expect("should parse");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "建议创建日程");
        assert!((candidates[0].confidence - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_json_array_without_fence() {
        let raw = r#"[{"title":"检查日程","confidence":0.72,"source_memory_ids":[],"requires_confirmation":false}]"#;
        let candidates = parse_thought_candidates(raw).expect("should parse");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "检查日程");
        assert!(!candidates[0].requires_confirmation);
    }

    #[test]
    fn parse_empty_array() {
        let candidates = parse_thought_candidates("[]").expect("should parse");
        assert!(candidates.is_empty());
    }

    #[test]
    fn confidence_clamp_to_range() {
        let raw = r#"[{"title":"测试","confidence":1.5,"source_memory_ids":[],"requires_confirmation":true}]"#;
        let candidates = parse_thought_candidates(raw).expect("should parse");
        let clamped = candidates[0].confidence.clamp(0.0, 1.0);
        assert!((clamped - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn low_confidence_filtered_out() {
        // The filtering is in generate_and_save_thoughts, but we can test the threshold directly
        let raw = r#"[{"title":"低置信度建议","confidence":0.3,"source_memory_ids":[],"requires_confirmation":false}]"#;
        let candidates = parse_thought_candidates(raw).expect("should parse");
        let confidence = candidates[0].confidence.clamp(0.0, 1.0);
        assert!(confidence < MIN_CONFIDENCE);
        assert!(!candidates[0].title.is_empty());
    }

    #[test]
    fn duplicate_title_detection_identical() {
        // Simulate the dedup logic: exact title match
        let existing = "建议创建日程";
        let candidate = "建议创建日程";
        assert_eq!(existing, candidate);
        assert!(is_duplicate(
            existing,
            candidate,
            DEDUP_SIMILARITY_THRESHOLD
        ));
    }

    #[test]
    fn field_defaults_are_populated() {
        let raw = r#"[{"title":"简洁建议","confidence":0.6}]"#;
        let candidates: Vec<ThoughtCandidate> = serde_json::from_str(raw).expect("should parse");
        assert_eq!(candidates[0].source_memory_ids, Vec::<Uuid>::new());
        assert!(candidates[0].requires_confirmation); // default true
    }

    // ── is_recently_dismissed ─────────────────────────────────────

    #[test]
    fn recently_dismissed_exact_match() {
        let id = Uuid::new_v4();
        let dismissed = vec![(id, "建议创建日程".to_string())];
        assert!(is_recently_dismissed(&dismissed, "建议创建日程"));
    }

    #[test]
    fn recently_dismissed_no_match() {
        let id = Uuid::new_v4();
        let dismissed = vec![(id, "建议创建日程".to_string())];
        assert!(!is_recently_dismissed(&dismissed, "完全不相关的建议"));
    }

    #[test]
    fn recently_dismissed_empty_list() {
        assert!(!is_recently_dismissed(&[], "建议创建日程"));
    }

    #[test]
    fn recently_dismissed_semantic_similarity() {
        let id = Uuid::new_v4();
        // Near-identical text with minor whitespace/punctuation variation
        // IS caught by is_duplicate (Jaccard similarity > threshold)
        let dismissed = vec![(id, "建议创建一个日程提醒".to_string())];
        assert!(is_recently_dismissed(
            &dismissed,
            "建议创建一个日程提醒！"
        ));
    }

    // ── active_count_allows_more ──────────────────────────────────

    #[test]
    fn allows_when_below_max() {
        assert!(active_count_allows_more(0, 5));
        assert!(active_count_allows_more(4, 5));
    }

    #[test]
    fn disallows_when_at_or_above_max() {
        assert!(!active_count_allows_more(5, 5));
        assert!(!active_count_allows_more(6, 5));
    }
}
