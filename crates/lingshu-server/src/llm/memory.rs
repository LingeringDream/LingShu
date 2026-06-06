use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tracing;
use uuid::Uuid;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::llm::dedup::{is_duplicate, DEDUP_SIMILARITY_THRESHOLD};
use crate::models::memory::Memory;
use lingshu_vector::search::{QdrantClient, SearchResult};
use sqlx::PgPool;

/// Minimum seconds between memory extraction calls to avoid overwhelming the LLM.
static LAST_EXTRACTIONS: OnceLock<Mutex<HashMap<Uuid, u64>>> = OnceLock::new();
const EXTRACTION_COOLDOWN_SECS: u64 = 60;

/// A memory candidate extracted from conversation by LLM.
#[derive(Debug, Deserialize)]
struct MemoryCandidate {
    content: String,
    importance: f32,
    memory_type: String,
}

/// Outcome of [`save_memory`].
pub struct SaveMemoryOutcome {
    pub memory: Memory,
    /// `true` when a new row was inserted; `false` when an existing row was updated.
    pub created: bool,
}

/// Query recent / high-importance memories of the same type and check for
/// near-duplicate content. Returns the `id` of the first duplicate found.
async fn find_duplicate_memory(
    db: &PgPool,
    user_id: Uuid,
    memory_type: &str,
    content: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, content FROM memories \
         WHERE user_id = $1 AND memory_type = $2 AND deleted_at IS NULL \
         ORDER BY importance DESC, updated_at DESC \
         LIMIT 50",
    )
    .bind(user_id)
    .bind(memory_type)
    .fetch_all(db)
    .await?;

    for (id, existing_content) in &rows {
        if is_duplicate(existing_content, content, DEDUP_SIMILARITY_THRESHOLD) {
            return Ok(Some(*id));
        }
    }

    Ok(None)
}

/// Derive an i64 advisory lock key from (user_id, memory_type).
/// Uses the low 8 bytes of the UUID XOR'd with a hash of the type string
/// to produce a well-distributed partition key.
fn advisory_lock_key(user_id: Uuid, memory_type: &str) -> i64 {
    let uuid_bytes = user_id.as_bytes();
    let uuid_low = i64::from_le_bytes(
        uuid_bytes[0..8].try_into().unwrap(),
    );
    // Simple string hash for the type suffix
    let type_hash: i64 = memory_type
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64));
    uuid_low ^ type_hash
}

/// Save a memory to the database with near-duplicate detection.
///
/// If a duplicate is found, the existing row's `importance` is bumped to
/// `GREATEST(importance, new)` and `updated_at` refreshed. Otherwise a new
/// row is inserted.
///
/// The duplicate-check + insert/update runs inside a transaction serialized
/// by `pg_advisory_xact_lock` keyed on (user_id, memory_type), preventing
/// TOCTOU races where concurrent callers could both pass the semantic
/// similarity check and insert near-duplicate rows.
///
/// After a successful insert, this function *best-effort* computes an
/// embedding via `llm.embed()`, upserts a point into Qdrant, and writes
/// `vector_id` back to the row.  When Qdrant or the embedding model is
/// unavailable the failure is logged and the memory is still saved — the
/// vector path is never allowed to fail the caller.
///
/// This is the canonical write path for memories — used by both automatic
/// extraction and the manual `POST /api/v1/memories` endpoint.
pub async fn save_memory(
    db: &PgPool,
    vector: &Option<QdrantClient>,
    llm: &LlmClient,
    embed_model: &str,
    user_id: Uuid,
    memory_type: &str,
    content: &str,
    importance: f32,
) -> Result<SaveMemoryOutcome, sqlx::Error> {
    // Derive a stable advisory lock key from (user_id, memory_type).
    // pg_advisory_xact_lock serializes concurrent writes within the same
    // (user, type) partition without blocking unrelated partitions.
    let lock_key = advisory_lock_key(user_id, memory_type);

    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *tx)
        .await?;

    // 1. Check for duplicates
    if let Some(existing_id) = find_duplicate_memory(db, user_id, memory_type, content).await? {
        let updated: Memory = sqlx::query_as(
            "UPDATE memories \
             SET importance = GREATEST(importance, $1), updated_at = NOW() \
             WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL \
             RETURNING *",
        )
        .bind(importance)
        .bind(existing_id)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            memory_type = %memory_type,
            existing_id = %existing_id,
            new_importance = %importance,
            "Skipped duplicate memory, bumped importance"
        );

        return Ok(SaveMemoryOutcome {
            memory: updated,
            created: false,
        });
    }

    // 2. No duplicate — insert
    let inserted: Memory = sqlx::query_as(
        "INSERT INTO memories (user_id, memory_type, content, importance) \
         VALUES ($1, $2, $3, $4) \
         RETURNING *",
    )
    .bind(user_id)
    .bind(memory_type)
    .bind(content)
    .bind(importance)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    // 3. Best-effort vector upsert (outside transaction — fire-and-forget)
    // Clone everything needed by the spawned task so nothing borrows from this scope.
    if vector.is_some() {
        let memory_id = inserted.id;
        let content_owned = content.to_string();
        let db_clone = db.clone();
        let user_id_val = user_id;
        let embed_model = embed_model.to_string();
        let qdrant_opt: Option<QdrantClient> = vector.clone();
        let llm_clone = llm.clone();

        tokio::spawn(async move {
            let Some(qdrant) = qdrant_opt else { return };
            if let Err(e) = upsert_memory_vector(
                &db_clone,
                &qdrant,
                &llm_clone,
                &embed_model,
                user_id_val,
                memory_id,
                &content_owned,
            )
            .await
            {
                tracing::warn!(%memory_id, %e, "Failed to upsert memory vector (non-fatal)");
            }
        });
    }

    Ok(SaveMemoryOutcome {
        memory: inserted,
        created: true,
    })
}

/// Best-effort: embed content → upsert Qdrant point → write vector_id back to PG.
/// Failures at any step are returned as `Err` and logged by the caller.
async fn upsert_memory_vector(
    db: &PgPool,
    qdrant: &QdrantClient,
    llm: &LlmClient,
    embed_model: &str,
    user_id: Uuid,
    memory_id: Uuid,
    content: &str,
) -> anyhow::Result<()> {
    // 3a. Compute embedding
    let embedding = llm.embed(embed_model, content).await?;

    // 3b. Upsert point into Qdrant
    let payload = serde_json::json!({
        "memory_id": memory_id.to_string(),
        "user_id": user_id.to_string(),
    });
    qdrant
        .upsert_point("memories", memory_id, embedding, payload)
        .await?;

    // 3c. Write vector_id back to PG (use memory_id as vector_id)
    sqlx::query("UPDATE memories SET vector_id = $1 WHERE id = $2 AND user_id = $3")
        .bind(memory_id.to_string())
        .bind(memory_id)
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(())
}

/// Build a Qdrant filter that restricts results to a single user.
/// The filter matches on the `user_id` payload field.
pub fn build_user_filter(user_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "must": [{
            "key": "user_id",
            "match": {
                "value": user_id.to_string()
            }
        }]
    })
}

/// Extract unique memory UUIDs from Qdrant search results.
/// Invalid UUIDs are silently skipped.
pub fn extract_memory_ids(results: &[SearchResult]) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for r in results {
        if let Ok(id) = Uuid::parse_str(&r.id) {
            if seen.insert(id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// Extract memorable facts from a user↔assistant exchange and persist high-value ones.
pub async fn extract_and_save(
    db: &PgPool,
    vector: &Option<QdrantClient>,
    llm: &LlmClient,
    model: &str,
    embed_model: &str,
    user_id: Uuid,
    user_message: &str,
    assistant_response: &str,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let should_extract = {
        let mut last_extractions = last_extractions()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        should_extract_memory(&mut last_extractions, user_id, now)
    };
    if !should_extract {
        return;
    }

    let prompt = build_memory_extraction_prompt(user_message, assistant_response);

    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    let response = match llm.chat(model, messages, None).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Memory extraction LLM call failed: {}", e);
            return;
        }
    };

    let candidates = match parse_candidates(&response) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to parse memory candidates: {}", e);
            return;
        }
    };

    let mut saved = 0u32;
    for c in candidates {
        if c.importance < 0.5 {
            continue; // below retention threshold
        }
        match save_memory(
            db,
            vector,
            llm,
            embed_model,
            user_id,
            &c.memory_type,
            &c.content,
            c.importance,
        )
        .await
        {
            Ok(outcome) => {
                if outcome.created {
                    saved += 1;
                }
            }
            Err(e) => tracing::warn!("Failed to save memory: {}", e),
        }
    }

    if saved > 0 {
        tracing::info!("Saved {} new memory candidates from chat", saved);
    }
}

/// Build the memory extraction prompt.
///
/// When `assistant_response` is non-empty it is included as disambiguation
/// context only — the LLM is instructed NOT to treat assistant statements
/// as user facts.  When empty the prompt degrades gracefully to user-only mode.
pub fn build_memory_extraction_prompt(user_message: &str, assistant_response: &str) -> String {
    let base = r###"你是 灵枢（LingShu）的记忆抽取系统。请从本轮用户-助手对话中提取值得长期记住的用户信息。

## 你应该记住什么
- **偏好 (preference)**：用户明确或隐含表达的习惯、喜好、厌恶。例如：「我习惯早上开会」「我不喜欢用飞书」「我偏好简洁的 UI」
- **事实 (fact)**：用户透露的客观信息。例如：「我的团队有 5 个人」「我们用的是 Rust + React 技术栈」「公司 VPN 地址是 xxx」
- **目标 (goal)**：用户提到的工作/生活目标或计划。例如：「这个季度要上线用户系统」「我计划年底前学会 SwiftUI」
- **上下文 (context)**：用户的工作场景、所处环境、角色身份。例如：「我最近在负责前端重构项目」「我是 iOS 团队的 PM」

## 不重要的事（跳过）
- 闲聊、问候、感谢（「谢谢」「好的」「明白了」）
- 一次性请求（「帮我查一下天气」）
- 纯技术问答（「Rust 的 borrow checker 怎么用」）
- 对助手行为的临场反馈（「回复太长了，短一点」）
- 助手自身的建议、推测、总结或行动计划（这些不是用户事实）

## 重要性评分指导
- 0.9-1.0：用户明确说「记住这个」或「很重要」，或涉及核心身份认同
- 0.7-0.8：反复出现、与长期目标/偏好相关、用户主动分享的个人信息
- 0.5-0.6：一次性但有参考价值的信息
- 0.0-0.4：不值得长期存储，跳过

## 去重提醒
如果用户说的内容与已有记忆高度重叠，降低 importance 或直接跳过。不要为同一事实创建多条重复记忆。

## 输出格式
严格返回 JSON 数组。每条对象包含：content（记忆内容，用中文概括）、importance（0.0-1.0）、memory_type（preference | fact | goal | context）。

如果消息中没有任何值得记住的内容，返回 []。

## 用户消息（主要事实来源）
{user_message}"###;

    let mut prompt = base.replace("{user_message}", user_message);

    if !assistant_response.trim().is_empty() {
        prompt.push_str("\n\n## 助手回复（仅作上下文，不作为事实来源）\n");
        prompt.push_str("以下助手的回复只能用于消歧和理解对话上下文。");
        prompt.push_str("不要把助手提出的建议、做出的推测、给出的总结或行动计划保存为用户事实。");
        prompt.push_str("只有用户本人明确表达或确认的信息才可以被记为记忆。\n\n");
        prompt.push_str(assistant_response);
    }

    prompt.push_str("\n\nJSON 数组：");
    prompt
}

/// Parse the LLM response for a JSON array of memory candidates.
/// Handles common LLM output quirks (markdown fences, trailing text).
fn parse_candidates(raw: &str) -> anyhow::Result<Vec<MemoryCandidate>> {
    let text = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Guarded extraction: find [ ] pair, verifying start < end
    let slice = match (text.find('['), text.rfind(']')) {
        (Some(start), Some(end)) if start < end => &text[start..=end],
        _ => text, // fallback: try parsing the whole text
    };

    let candidates: Vec<MemoryCandidate> = serde_json::from_str(slice)?;
    Ok(candidates)
}

fn last_extractions() -> &'static Mutex<HashMap<Uuid, u64>> {
    LAST_EXTRACTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn should_extract_memory(
    last_extractions: &mut HashMap<Uuid, u64>,
    user_id: Uuid,
    now: u64,
) -> bool {
    if let Some(last) = last_extractions.get(&user_id) {
        if now.saturating_sub(*last) < EXTRACTION_COOLDOWN_SECS {
            return false;
        }
    }

    last_extractions.insert(user_id, now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn extraction_cooldown_is_scoped_per_user() {
        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();
        let mut last_extractions = HashMap::new();

        assert!(should_extract_memory(&mut last_extractions, user_a, 100));
        assert!(!should_extract_memory(&mut last_extractions, user_a, 120));
        assert!(should_extract_memory(&mut last_extractions, user_b, 120));
    }

    // ── Prompt tests ──────────────────────────────────────────────

    #[test]
    fn prompt_contains_assistant_section_when_non_empty() {
        let prompt = build_memory_extraction_prompt("我喜欢 Rust", "Rust 确实很适合系统编程！");
        assert!(
            prompt.contains("助手回复"),
            "prompt should include assistant section when response is non-empty"
        );
        assert!(
            prompt.contains("Rust 确实很适合系统编程"),
            "prompt should include the assistant text"
        );
    }

    #[test]
    fn prompt_omits_assistant_section_when_empty() {
        let prompt = build_memory_extraction_prompt("我喜欢 Rust", "");
        assert!(
            !prompt.contains("助手回复"),
            "prompt should NOT include assistant section when response is empty"
        );
    }

    #[test]
    fn prompt_omits_assistant_section_when_whitespace_only() {
        let prompt = build_memory_extraction_prompt("我喜欢 Rust", "   \n  ");
        assert!(
            !prompt.contains("助手回复"),
            "prompt should treat whitespace-only response as empty"
        );
    }

    #[test]
    fn prompt_warns_assistant_is_not_fact_source() {
        let prompt = build_memory_extraction_prompt("我喜欢 Rust", "好的，Rust 很不错");
        assert!(
            prompt.contains("不作为事实来源"),
            "prompt must state assistant reply is not a fact source"
        );
        assert!(
            prompt.contains("不要把助手提出的建议"),
            "prompt must warn against treating assistant as fact source"
        );
    }

    #[test]
    fn prompt_contains_user_message() {
        let prompt = build_memory_extraction_prompt("我的团队有5个人", "收到，已记录");
        assert!(prompt.contains("我的团队有5个人"));
        assert!(prompt.contains("主要事实来源"));
    }

    #[test]
    fn prompt_requires_json_array_output() {
        let prompt = build_memory_extraction_prompt("hello", "world");
        assert!(
            prompt.contains("JSON 数组"),
            "prompt must require JSON array output format"
        );
        assert!(
            prompt.contains("content")
                && prompt.contains("importance")
                && prompt.contains("memory_type"),
            "prompt must specify output fields"
        );
    }

    #[test]
    fn prompt_degraded_to_user_only_is_still_valid() {
        let prompt = build_memory_extraction_prompt("记住：VPN 是 10.0.0.1", "");
        assert!(prompt.contains("记住：VPN 是 10.0.0.1"));
        assert!(prompt.contains("JSON 数组"));
        assert!(!prompt.contains("助手回复"));
    }

    // ── Vector helper tests ────────────────────────────────────────

    #[test]
    fn user_filter_contains_user_id() {
        let uid = Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap();
        let filter = build_user_filter(uid);
        let expected = uid.to_string();
        // Check the filter structure matches Qdrant's must-match format
        assert!(
            filter.to_string().contains(&expected),
            "filter must contain the user_id value: {filter}"
        );
        assert!(
            filter.to_string().contains("user_id"),
            "filter must key on user_id: {filter}"
        );
    }

    #[test]
    fn extract_memory_ids_parses_valid_uuids() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let results = vec![
            SearchResult {
                id: id1.to_string(),
                score: 0.9,
                payload: None,
            },
            SearchResult {
                id: id2.to_string(),
                score: 0.8,
                payload: None,
            },
        ];
        let ids = extract_memory_ids(&results);
        assert_eq!(ids, vec![id1, id2]);
    }

    #[test]
    fn extract_memory_ids_deduplicates() {
        let id = Uuid::new_v4();
        let results = vec![
            SearchResult {
                id: id.to_string(),
                score: 0.9,
                payload: None,
            },
            SearchResult {
                id: id.to_string(),
                score: 0.7,
                payload: None,
            },
        ];
        let ids = extract_memory_ids(&results);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], id);
    }

    #[test]
    fn extract_memory_ids_skips_invalid_uuids() {
        let valid = Uuid::new_v4();
        let results = vec![
            SearchResult {
                id: "not-a-uuid".to_string(),
                score: 0.9,
                payload: None,
            },
            SearchResult {
                id: valid.to_string(),
                score: 0.8,
                payload: None,
            },
            SearchResult {
                id: "".to_string(),
                score: 0.7,
                payload: None,
            },
        ];
        let ids = extract_memory_ids(&results);
        assert_eq!(ids, vec![valid]);
    }

    #[test]
    fn extract_memory_ids_empty_input() {
        let ids = extract_memory_ids(&[]);
        assert!(ids.is_empty());
    }
}
