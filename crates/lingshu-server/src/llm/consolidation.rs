//! Offline memory consolidation (LLM-as-judge).
//!
//! This is the only place the LLM is allowed to *judge* — because it sees
//! **all of a user's memories at once**.  The engine periodically scans the
//! memory pool, asks the LLM to suggest semantically-overlapping groups worth
//! merging, and produces "derived" summaries while soft-demoting (never
//! deleting) the source memories.
//!
//! Trigger: per-user 24h cooldown, invoked from the chat post-stream hook.
//! Failures never propagate — the LLM call is optional background work.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::llm::memory::upsert_memory_vector;
use crate::models::memory::Memory;
use lingshu_vector::search::QdrantClient;
use sqlx::PgPool;

// ── Tunables ─────────────────────────────────────────────────────────

const CONSOLIDATION_COOLDOWN_SECS: u64 = 24 * 60 * 60;

/// Minimum number of active memories before consolidation even considers running.
const MIN_MEMORIES_FOR_CONSOLIDATION: usize = 8;

/// LLM-proposed merge groups below this confidence are discarded.
const MIN_CONFIDENCE: f32 = 0.7;

/// At most this many merge groups per consolidation run (avoids overwhelming
/// the pool with derived memories in one batch).
const MAX_MERGES: usize = 3;

/// Raw source memories that feed into a derived summary are soft-demoted by
/// this factor (importance * 0.5). They are NOT soft-deleted.
const DEMOTE_FACTOR: f32 = 0.5;

// ── Cooldown ─────────────────────────────────────────────────────────

static LAST_CONSOLIDATIONS: OnceLock<Mutex<HashMap<Uuid, u64>>> = OnceLock::new();

/// Pure predicate — testable without a real clock.
pub fn should_run_consolidation_at(
    last: &mut HashMap<Uuid, u64>,
    user_id: Uuid,
    now: u64,
) -> bool {
    if let Some(prev) = last.get(&user_id) {
        if now.saturating_sub(*prev) < CONSOLIDATION_COOLDOWN_SECS {
            return false;
        }
    }
    last.insert(user_id, now);
    true
}

/// Per-user cooldown gate (reads the real system clock).
pub fn should_run_consolidation(user_id: Uuid) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut map = LAST_CONSOLIDATIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    should_run_consolidation_at(&mut map, user_id, now)
}

// ── LLM response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct MergeCandidate {
    merged_content: String,
    memory_type: String,
    importance: f32,
    source_ids: Vec<Uuid>,
    confidence: f32,
}

// ── Pure validation ──────────────────────────────────────────────────

/// Filter and deduplicate LLM-proposed merge groups against the loaded memory
/// set. Returns up to `MAX_MERGES` valid groups (first-wins for overlapping
/// sources).
///
/// Rules (applied in order):
/// 1. confidence >= MIN_CONFIDENCE
/// 2. source_ids must contain ≥ 2 entries, and every id must appear in `loaded_ids`
/// 3. A source UUID may only appear in one accepted group; later groups with
///    overlapping sources are skipped.
pub fn validate_merge_groups<'a>(
    candidates: &'a [MergeCandidate],
    loaded_ids: &HashSet<Uuid>,
) -> Vec<&'a MergeCandidate> {
    let mut accepted: Vec<&MergeCandidate> = Vec::new();
    let mut claimed_sources: HashSet<Uuid> = HashSet::new();

    for c in candidates {
        if accepted.len() >= MAX_MERGES {
            break;
        }
        if c.confidence < MIN_CONFIDENCE || c.source_ids.len() < 2 {
            continue;
        }
        // All source IDs must be in the loaded set
        if !c.source_ids.iter().all(|id| loaded_ids.contains(id)) {
            continue;
        }
        // No overlapping sources with already-accepted groups
        if c.source_ids.iter().any(|id| claimed_sources.contains(id)) {
            continue;
        }
        for id in &c.source_ids {
            claimed_sources.insert(*id);
        }
        accepted.push(c);
    }

    accepted
}

// ── Public API ───────────────────────────────────────────────────────

/// Run the offline consolidation pipeline for one user.
///
/// Returns the number of derived memories created (0 on skip / empty / error).
/// This function is **best-effort**: LLM / parse failures only `warn!` and
/// return 0 — the caller must never treat failure as a chat-breaking event.
pub async fn consolidate_memories(
    db: &PgPool,
    llm: &LlmClient,
    model: &str,
    embed_model: &str,
    vector: &Option<QdrantClient>,
    user_id: Uuid,
) -> usize {
    let result = try_consolidate(db, llm, model, embed_model, vector, user_id).await;
    match result {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(%user_id, %e, "Memory consolidation failed (non-fatal)");
            0
        }
    }
}

async fn try_consolidate(
    db: &PgPool,
    llm: &LlmClient,
    model: &str,
    embed_model: &str,
    vector: &Option<QdrantClient>,
    user_id: Uuid,
) -> anyhow::Result<usize> {
    // 1. Load all active memories
    let memories: Vec<Memory> = sqlx::query_as(
        "SELECT * FROM memories \
         WHERE user_id = $1 AND deleted_at IS NULL \
         ORDER BY importance DESC, updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    if memories.len() < MIN_MEMORIES_FOR_CONSOLIDATION {
        return Ok(0);
    }

    let loaded_ids: HashSet<Uuid> = memories.iter().map(|m| m.id).collect();

    // 2. Build context for the LLM
    let memory_list = build_memory_list(&memories);
    let prompt = build_consolidation_prompt(&memory_list);

    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    // 3. Call LLM
    let response = llm.chat(model, messages, None).await?;
    let candidates = parse_merge_candidates(&response)?;

    // 4. Validate
    let valid_groups = validate_merge_groups(&candidates, &loaded_ids);
    if valid_groups.is_empty() {
        return Ok(0);
    }

    // 5. Apply each merge group in a transaction
    let mut created = 0usize;
    for group in valid_groups {
        if let Err(e) = apply_merge_group(db, llm, embed_model, vector, user_id, group).await {
            tracing::warn!(%user_id, %e, "Failed to apply merge group, continuing with next");
            continue;
        }
        created += 1;
    }

    if created > 0 {
        tracing::info!(%user_id, created, "Memory consolidation produced derived memories");
    }

    Ok(created)
}

// ── Prompt building ──────────────────────────────────────────────────

fn build_memory_list(memories: &[Memory]) -> String {
    let mut lines = Vec::with_capacity(memories.len());
    for m in memories {
        let tier_label = if m.tier == "derived" { "[归纳]" } else { "" };
        lines.push(format!(
            "{} [{}] (importance={:.2}) {}",
            m.id, m.memory_type, m.importance, m.content
        ));
        if !tier_label.is_empty() {
            lines.last_mut().unwrap().push_str(tier_label);
        }
    }
    lines.join("\n")
}

fn build_consolidation_prompt(memory_list: &str) -> String {
    format!(
        r###"你是 灵枢（LingShu）的记忆离线整理引擎。

## 你的任务
在下面这个用户的全量记忆列表中，找出语义高度重叠的记忆组，用一条简洁、准确的记忆来概括它们。
这不是"改写"，而是从多条相似的片段中提炼出一条更能代表它们的语义记忆。

## 合并约束
- 只合并语义高度重叠的记忆（比如多条记忆都在说同一件事的不同侧面）；证据不够就不合并
- 每组至少包含 2 条源记忆（source_ids）
- 源记忆必须从下面列表中选取（通过 UUID 引用）
- 合并后的内容用中文精炼概括，保留具体信息（人名、数字、偏好细节）不要泛化成空话
- memory_type 保持与源记忆一致的类型（preference / fact / goal / context）
- importance 取源记忆中的合理值（0.0-1.0），作为新记忆的初始重要性
- confidence 表达你对这组合并质量的信心（0.0-1.0），低于 0.7 的组合会被丢弃

## 不要做的事
- 不要合并两三条实际上不相关的记忆
- 不要凭空创造源列表中不存在的信息
- 不要合并已经标记为 [归纳] 的记忆（它们已经是提炼过的）
- 一个源 UUID 只能出现在一个合并组里

## 输出格式
严格返回 JSON 数组。每条对象包含：merged_content, memory_type, importance, source_ids（UUID 数组）, confidence。

如果没有值得合并的组，返回 []。

## 记忆列表
{memory_list}

JSON 数组："###
    )
}

// ── Parsing ──────────────────────────────────────────────────────────

fn parse_merge_candidates(raw: &str) -> anyhow::Result<Vec<MergeCandidate>> {
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

    let candidates: Vec<MergeCandidate> = serde_json::from_str(slice)?;
    Ok(candidates)
}

// ── Apply one merge group ────────────────────────────────────────────

async fn apply_merge_group(
    db: &PgPool,
    llm: &LlmClient,
    embed_model: &str,
    vector: &Option<QdrantClient>,
    user_id: Uuid,
    group: &MergeCandidate,
) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;

    // 1. Insert the derived memory
    let derived: Memory = sqlx::query_as(
        "INSERT INTO memories \
         (user_id, memory_type, content, importance, source_memory_ids, tier) \
         VALUES ($1, $2, $3, $4, $5, 'derived') \
         RETURNING *",
    )
    .bind(user_id)
    .bind(&group.memory_type)
    .bind(&group.merged_content)
    .bind(group.importance)
    .bind(&group.source_ids)
    .fetch_one(&mut *tx)
    .await?;

    // 2. Soft-demote source memories (never soft-delete)
    sqlx::query(
        "UPDATE memories SET importance = importance * $1, updated_at = NOW() \
         WHERE id = ANY($2) AND user_id = $3 AND deleted_at IS NULL",
    )
    .bind(DEMOTE_FACTOR)
    .bind(&group.source_ids)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // 3. Best-effort vector indexing (outside transaction, fire-and-forget)
    if let Some(qdrant) = vector {
        let embed_model = embed_model.to_string();
        let db_clone = db.clone();
        let llm_clone = llm.clone();
        let qdrant_clone = qdrant.clone();
        let content = group.merged_content.clone();
        let memory_id = derived.id;

        tokio::spawn(async move {
            if let Err(e) = upsert_memory_vector(
                &db_clone,
                &qdrant_clone,
                &llm_clone,
                &embed_model,
                user_id,
                memory_id,
                &content,
            )
            .await
            {
                tracing::warn!(%memory_id, %e, "Failed to index derived memory vector (non-fatal)");
            }
        });
    }

    // 4. Telemetry: reference each source + consolidation event
    for source_id in &group.source_ids {
        crate::telemetry::record(
            db,
            user_id,
            crate::telemetry::SignalEventType::MemoryReferenced,
            Some("memory"),
            Some(*source_id),
            serde_json::json!({"source": "consolidation"}),
        )
        .await;
    }

    crate::telemetry::record(
        db,
        user_id,
        crate::telemetry::SignalEventType::MemoryConsolidated,
        Some("memory"),
        Some(derived.id),
        serde_json::json!({
            "source_ids": &group.source_ids,
            "confidence": group.confidence,
        }),
    )
    .await;

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── should_run_consolidation_at ────────────────────────────────

    #[test]
    fn consolidation_cooldown_allows_first_call() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_run_consolidation_at(&mut map, user, 100));
    }

    #[test]
    fn consolidation_cooldown_blocks_second_call_within_window() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_run_consolidation_at(&mut map, user, 100));
        assert!(!should_run_consolidation_at(&mut map, user, 100 + 3600));
    }

    #[test]
    fn consolidation_cooldown_is_per_user() {
        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_run_consolidation_at(&mut map, user_a, 100));
        assert!(should_run_consolidation_at(&mut map, user_b, 100));
    }

    #[test]
    fn consolidation_cooldown_allows_after_window_expires() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_run_consolidation_at(&mut map, user, 100));
        let after = 100 + CONSOLIDATION_COOLDOWN_SECS;
        assert!(should_run_consolidation_at(&mut map, user, after));
    }

    // ── validate_merge_groups ──────────────────────────────────────

    fn make_ids(count: usize) -> Vec<Uuid> {
        (0..count).map(|_| Uuid::new_v4()).collect()
    }

    #[test]
    fn validate_accepts_valid_group() {
        let ids = make_ids(3);
        let loaded: HashSet<Uuid> = ids.iter().copied().collect();
        let candidates = vec![MergeCandidate {
            merged_content: "merged".into(),
            memory_type: "fact".into(),
            importance: 0.8,
            source_ids: ids[..2].to_vec(),
            confidence: 0.85,
        }];
        let accepted = validate_merge_groups(&candidates, &loaded);
        assert_eq!(accepted.len(), 1);
    }

    #[test]
    fn validate_rejects_low_confidence() {
        let ids = make_ids(2);
        let loaded: HashSet<Uuid> = ids.iter().copied().collect();
        let candidates = vec![MergeCandidate {
            merged_content: "merged".into(),
            memory_type: "fact".into(),
            importance: 0.5,
            source_ids: ids.clone(),
            confidence: 0.5, // < 0.7
        }];
        let accepted = validate_merge_groups(&candidates, &loaded);
        assert!(accepted.is_empty());
    }

    #[test]
    fn validate_rejects_single_source() {
        let ids = make_ids(2);
        let loaded: HashSet<Uuid> = ids.iter().copied().collect();
        let candidates = vec![MergeCandidate {
            merged_content: "merged".into(),
            memory_type: "fact".into(),
            importance: 0.8,
            source_ids: vec![ids[0]], // only 1 source
            confidence: 0.9,
        }];
        let accepted = validate_merge_groups(&candidates, &loaded);
        assert!(accepted.is_empty());
    }

    #[test]
    fn validate_rejects_foreign_source_ids() {
        let ids = make_ids(2);
        let foreign = Uuid::new_v4();
        let loaded: HashSet<Uuid> = ids.iter().copied().collect(); // foreign NOT in loaded
        let candidates = vec![MergeCandidate {
            merged_content: "merged".into(),
            memory_type: "fact".into(),
            importance: 0.8,
            source_ids: vec![ids[0], foreign],
            confidence: 0.9,
        }];
        let accepted = validate_merge_groups(&candidates, &loaded);
        assert!(accepted.is_empty());
    }

    #[test]
    fn validate_caps_at_max_merges() {
        // Need (MAX_MERGES+2) * 2 unique IDs (2 per group, no overlap)
        let all_ids = make_ids((MAX_MERGES + 2) * 2);
        let loaded: HashSet<Uuid> = all_ids.iter().copied().collect();
        let mut candidates: Vec<MergeCandidate> = Vec::new();
        // Create MAX_MERGES+2 valid groups (each with 2 unique sources, no overlap)
        for i in 0..(MAX_MERGES + 2) {
            let src = vec![all_ids[i * 2], all_ids[i * 2 + 1]];
            candidates.push(MergeCandidate {
                merged_content: format!("merged {i}"),
                memory_type: "fact".into(),
                importance: 0.8,
                source_ids: src,
                confidence: 0.9,
            });
        }
        let accepted = validate_merge_groups(&candidates, &loaded);
        assert_eq!(accepted.len(), MAX_MERGES);
    }

    #[test]
    fn validate_rejects_overlapping_sources() {
        let ids = make_ids(3); // share ids[1] between groups
        let loaded: HashSet<Uuid> = ids.iter().copied().collect();
        let candidates = vec![
            MergeCandidate {
                merged_content: "first".into(),
                memory_type: "fact".into(),
                importance: 0.8,
                source_ids: vec![ids[0], ids[1]],
                confidence: 0.85,
            },
            MergeCandidate {
                merged_content: "second".into(),
                memory_type: "fact".into(),
                importance: 0.8,
                source_ids: vec![ids[1], ids[2]], // overlaps ids[1]
                confidence: 0.85,
            },
        ];
        let accepted = validate_merge_groups(&candidates, &loaded);
        // First-wins: only the first group is accepted; second is skipped because ids[1] is claimed
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].merged_content, "first");
    }

    // ── parse_merge_candidates ────────────────────────────────────

    #[test]
    fn parse_valid_merge_candidates() {
        let raw = r#"[
            {"merged_content":"合并内容","memory_type":"preference","importance":0.75,"source_ids":["a1b2c3d4-e5f6-7890-abcd-ef1234567890","b2c3d4e5-f6a7-8901-bcde-f12345678901"],"confidence":0.8}
        ]"#;
        let candidates = parse_merge_candidates(raw).expect("should parse");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].merged_content, "合并内容");
        assert!((candidates[0].confidence - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_empty_array() {
        let candidates = parse_merge_candidates("[]").expect("should parse");
        assert!(candidates.is_empty());
    }

    #[test]
    fn parse_with_fence() {
        let raw = r#"```json
[{"merged_content":"test","memory_type":"fact","importance":0.7,"source_ids":["a1b2c3d4-e5f6-7890-abcd-ef1234567890","b2c3d4e5-f6a7-8901-bcde-f12345678901"],"confidence":0.9}]
```"#;
        let candidates = parse_merge_candidates(raw).expect("should parse");
        assert_eq!(candidates.len(), 1);
    }
}
