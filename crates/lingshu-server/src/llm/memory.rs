use serde::Deserialize;
use tracing;
use uuid::Uuid;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::llm::dedup::{is_duplicate, DEDUP_SIMILARITY_THRESHOLD};
use crate::models::memory::Memory;
use lingshu_vector::search::{QdrantClient, SearchResult};
use sqlx::PgPool;

/// Minimum seconds between memory extraction calls to avoid overwhelming the LLM.
const EXTRACTION_COOLDOWN_SECS: u64 = 60;

/// A memory candidate extracted from conversation by LLM.
#[derive(Debug, Deserialize)]
struct MemoryCandidate {
    content: String,
    importance: f32,
    memory_type: String,
    #[serde(default)]
    scene_tags: Vec<String>,
    #[serde(default)]
    fact_key: Option<String>,
    #[serde(default)]
    fact_value: Option<String>,
    #[serde(default = "default_candidate_trust")]
    source_trust: f32,
}

fn default_candidate_trust() -> f32 {
    0.75
}

/// Outcome of [`save_memory`].
pub struct SaveMemoryOutcome {
    pub memory: Memory,
    /// `true` when a new row was inserted; `false` when an existing row was updated.
    pub created: bool,
}

/// Match within the legacy writer's private/default domain and transaction.
async fn find_duplicate_memory(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    memory_type: &str,
    content: &str,
    candidate: Option<&MemoryCandidate>,
) -> Result<Option<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, content FROM memories
         WHERE user_id=$1 AND memory_type=$2 AND deleted_at IS NULL AND NOT derivation_stale
           AND owner_agent_id='lingshu' AND memory_scope='private' AND context_scope='default'
           AND conflict_status IN ('active','disputed') AND retention_status='active'
           AND valid_from<=NOW() AND (valid_to IS NULL OR valid_to>NOW())
           AND ($4 OR (fact_key IS NOT DISTINCT FROM $5 AND fact_value IS NOT DISTINCT FROM $6))
           AND memory_search_terms(content) && memory_search_terms($3)
         ORDER BY (content=$3) DESC, importance DESC, updated_at DESC LIMIT 50",
    )
    .bind(user_id)
    .bind(memory_type)
    .bind(content)
    .bind(candidate.is_none())
    .bind(
        candidate
            .and_then(|c| c.fact_key.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        candidate
            .and_then(|c| c.fact_value.as_deref())
            .map(str::trim),
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().find_map(|(id, existing)| {
        is_duplicate(&existing, content, DEDUP_SIMILARITY_THRESHOLD).then_some(id)
    }))
}

/// Derive an i64 advisory lock key from (user_id, memory_type).
/// Uses the low 8 bytes of the UUID XOR'd with a hash of the type string
/// to produce a well-distributed partition key.
fn advisory_lock_key(user_id: Uuid, memory_type: &str) -> i64 {
    let uuid_bytes = user_id.as_bytes();
    let uuid_low = i64::from_le_bytes(uuid_bytes[0..8].try_into().unwrap());
    // Simple string hash for the type suffix
    let type_hash: i64 = memory_type
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64));
    uuid_low ^ type_hash
}

/// Store a manual memory. Index work is enqueued in the same transaction by
/// database triggers, regardless of whether Qdrant is currently available.
pub async fn save_memory(
    db: &PgPool,
    _vector: &Option<QdrantClient>,
    _llm: &LlmClient,
    _embed_model: &str,
    user_id: Uuid,
    memory_type: &str,
    content: &str,
    importance: f32,
) -> Result<SaveMemoryOutcome, sqlx::Error> {
    persist_memory(db, user_id, memory_type, content, importance, None).await
}

/// Insert/deduplicate, attach provenance, reconcile conflicts, and enqueue
/// indexing atomically. Acquire fact locks before any memory row lock.
async fn persist_memory(
    db: &PgPool,
    user_id: Uuid,
    memory_type: &str,
    content: &str,
    importance: f32,
    enrichment: Option<(&MemoryCandidate, Option<Uuid>)>,
) -> Result<SaveMemoryOutcome, sqlx::Error> {
    let mut tx = db.begin().await?;
    let key = enrichment
        .and_then(|(c, _)| c.fact_key.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(key) = key {
        crate::routes::memory_gateway::lock_fact(&mut tx, user_id, "default", key).await?;
    }
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(advisory_lock_key(user_id, memory_type))
        .execute(&mut *tx)
        .await?;
    let existing = find_duplicate_memory(
        &mut tx,
        user_id,
        memory_type,
        content,
        enrichment.map(|(c, _)| c),
    )
    .await?;
    let (id, created) = if let Some(id) = existing {
        sqlx::query("UPDATE memories SET importance=GREATEST(importance,$1),updated_at=NOW() WHERE id=$2 AND user_id=$3")
            .bind(importance).bind(id).bind(user_id).execute(&mut *tx).await?;
        (id, false)
    } else {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO memories(user_id,memory_type,content,importance,memory_layer)
            VALUES($1,$2,$3,$4,'fact') RETURNING id",
        )
        .bind(user_id)
        .bind(memory_type)
        .bind(content)
        .bind(importance)
        .fetch_one(&mut *tx)
        .await?;
        (id, true)
    };
    if let Some((candidate, episode_id)) = enrichment {
        let sources: Vec<Uuid> = episode_id.into_iter().collect();
        sqlx::query("UPDATE memories SET memory_layer='fact',scene_tags=$1,fact_key=$2,fact_value=$3,
            source_trust=$4,source_type='conversation_extraction',
            source_memory_ids=ARRAY(SELECT DISTINCT source_id FROM unnest(source_memory_ids || $5::uuid[]) source_id ORDER BY source_id),
            episode_id=COALESCE(episode_id,$8),updated_at=NOW() WHERE id=$6 AND user_id=$7")
            .bind(&candidate.scene_tags).bind(key).bind(candidate.fact_value.as_deref().map(str::trim))
            .bind(candidate.source_trust.clamp(0.0,1.0)).bind(&sources).bind(id).bind(user_id).bind(episode_id)
            .execute(&mut *tx).await?;
        if let Some(key) = key {
            crate::routes::memory_gateway::resolve_new_fact_conflicts(&mut tx, user_id, id, key)
                .await?;
        }
    }
    let memory = sqlx::query_as("SELECT * FROM memories WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::telemetry::record(
        db,
        user_id,
        if created {
            crate::telemetry::SignalEventType::MemoryCreated
        } else {
            crate::telemetry::SignalEventType::MemoryDedupHit
        },
        Some("memory"),
        Some(id),
        serde_json::json!({"importance":importance,"memory_type":memory_type}),
    )
    .await;
    Ok(SaveMemoryOutcome { memory, created })
}

/// Compatibility enqueue for callers that explicitly request reindexing.
/// Normal mutations are already queued by the transactional database trigger.
pub(crate) async fn upsert_memory_vector(
    db: &PgPool,
    _qdrant: &QdrantClient,
    _llm: &LlmClient,
    _embed_model: &str,
    user_id: Uuid,
    memory_id: Uuid,
    _content: &str,
) -> anyhow::Result<()> {
    // Compatibility for existing writers. The trigger already durably enqueues
    // mutations; never race the worker with a direct vector write.
    sqlx::query(
        "INSERT INTO memory_index_jobs(memory_id,user_id) VALUES($1,$2)
        ON CONFLICT(memory_id) DO NOTHING",
    )
    .bind(memory_id)
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Only the leased worker writes vectors, using the latest database content.
pub(crate) async fn index_memory_vector(
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
    _vector: &Option<QdrantClient>,
    llm: &LlmClient,
    model: &str,
    _embed_model: &str,
    user_id: Uuid,
    user_message: &str,
    assistant_response: &str,
) {
    // Every completed exchange is retained as a traceable episode. Structured
    // facts extracted below link back to this raw fragment through
    // source_memory_ids, forming the first two layers of the three-layer model.
    let episode_id: Option<Uuid> = match sqlx::query_scalar(
        "INSERT INTO memories (user_id, memory_type, content, importance, memory_layer, \
         owner_agent_id, memory_scope, scene_tags, episode_id, source_type, source_trust) \
         VALUES ($1, 'context', $2, 0.5, 'episode', 'lingshu', 'private', $3, gen_random_uuid(), 'conversation', 0.9) \
         RETURNING id",
    )
    .bind(user_id)
    .bind(format!(
        "用户：{}\n助手：{}",
        user_message.trim(),
        assistant_response.trim()
    ))
    .bind(vec!["conversation".to_string()])
    .fetch_one(db)
    .await
    {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(%user_id, %error, "Failed to persist raw interaction episode");
            None
        }
    };

    let should_extract = crate::cooldown::memory_extractions().should_run(user_id);
    if !should_extract {
        return;
    }

    let prompt = build_memory_extraction_prompt(user_message, assistant_response);

    let messages = vec![ChatMessage::user(prompt)];

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
        match persist_memory(
            db,
            user_id,
            &c.memory_type,
            &c.content,
            c.importance,
            Some((&c, episode_id)),
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

## 结构化与来源字段
- scene_tags：1-3 个简短场景标签，例如 project、schedule、preference
- fact_key：稳定的事实键，例如 profile.city、project.stack；无法结构化时为 null
- fact_value：事实当前值；无法结构化时为 null
- source_trust：本轮用户直接陈述通常为 0.85-1.0，含糊推断不得高于 0.7

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
严格返回 JSON 数组。每条对象包含：content（记忆内容，用中文概括）、importance（0.0-1.0）、memory_type（preference | fact | goal | context）、scene_tags、fact_key、fact_value、source_trust。

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

#[cfg(test)]
fn should_extract_memory(
    last_extractions: &mut std::collections::HashMap<Uuid, u64>,
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

    #[tokio::test]
    #[ignore = "Requires dedicated MEMORY_TEST_DATABASE_URL; applies migrations"]
    async fn extracted_fact_is_atomic_and_accumulates_sources() {
        let db = PgPool::connect(&std::env::var("MEMORY_TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        let user = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,'test','unused')",
        )
        .bind(user)
        .bind(format!("{user}@test.local"))
        .execute(&db)
        .await
        .unwrap();
        let mut candidate = MemoryCandidate {
            content: "项目数据库采用 PostgreSQL".into(),
            importance: 0.8,
            memory_type: "fact".into(),
            scene_tags: vec!["project".into()],
            fact_key: Some("project.database".into()),
            fact_value: Some("PostgreSQL".into()),
            source_trust: 0.8,
        };
        let mut sources = Vec::new();
        for _ in 0..2 {
            let source: Uuid = sqlx::query_scalar(
                "INSERT INTO memories(user_id,memory_type,content,memory_layer)
                VALUES($1,'context','原始对话','episode') RETURNING id",
            )
            .bind(user)
            .fetch_one(&db)
            .await
            .unwrap();
            sources.push(source);
        }
        let first = persist_memory(
            &db,
            user,
            "fact",
            &candidate.content,
            0.8,
            Some((&candidate, Some(sources[0]))),
        )
        .await
        .unwrap();
        let second = persist_memory(
            &db,
            user,
            "fact",
            &candidate.content,
            0.8,
            Some((&candidate, Some(sources[1]))),
        )
        .await
        .unwrap();
        assert_eq!(first.memory.id, second.memory.id);
        assert!(!second.created);
        assert_eq!(second.memory.source_memory_ids.len(), 2);
        assert!(sources
            .iter()
            .all(|id| second.memory.source_memory_ids.contains(id)));
        candidate.fact_value = Some("MySQL".into());
        candidate.content = "项目数据库采用 MySQL".into();
        let challenger = persist_memory(
            &db,
            user,
            "fact",
            &candidate.content,
            0.8,
            Some((&candidate, None)),
        )
        .await
        .unwrap();
        assert_ne!(challenger.memory.id, first.memory.id);
        assert_eq!(challenger.memory.conflict_status, "disputed");
        let status: String = sqlx::query_scalar("SELECT conflict_status FROM memories WHERE id=$1")
            .bind(first.memory.id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(status, "disputed");
        // Failure during enrichment must roll back the preceding INSERT and job.
        candidate.fact_key = Some("x".repeat(256));
        candidate.content = "must roll back".into();
        let before: i64 =
            sqlx::query_scalar("SELECT count(*) FROM memory_index_jobs WHERE user_id=$1")
                .bind(user)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(persist_memory(
            &db,
            user,
            "fact",
            &candidate.content,
            0.8,
            Some((&candidate, None))
        )
        .await
        .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM memories WHERE user_id=$1 AND content='must roll back'"
            )
            .bind(user)
            .fetch_one(&db)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_index_jobs WHERE user_id=$1")
                .bind(user)
                .fetch_one(&db)
                .await
                .unwrap(),
            before
        );
        db.close().await;
    }
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
