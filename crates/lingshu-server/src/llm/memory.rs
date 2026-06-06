use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tracing;
use uuid::Uuid;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::llm::dedup::{is_duplicate, DEDUP_SIMILARITY_THRESHOLD};
use crate::models::memory::Memory;
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

/// Save a memory to the database with near-duplicate detection.
///
/// If a duplicate is found, the existing row's `importance` is bumped to
/// `GREATEST(importance, new)` and `updated_at` refreshed. Otherwise a new
/// row is inserted.
///
/// This is the canonical write path for memories — used by both automatic
/// extraction and the manual `POST /api/v1/memories` endpoint.
pub async fn save_memory(
    db: &PgPool,
    user_id: Uuid,
    memory_type: &str,
    content: &str,
    importance: f32,
) -> Result<SaveMemoryOutcome, sqlx::Error> {
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
        .fetch_one(db)
        .await?;

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
    .fetch_one(db)
    .await?;

    Ok(SaveMemoryOutcome {
        memory: inserted,
        created: true,
    })
}

/// Extract memorable facts from a user↔assistant exchange and persist high-value ones.
pub async fn extract_and_save(
    db: &PgPool,
    llm: &LlmClient,
    model: &str,
    user_id: Uuid,
    user_message: &str,
    _assistant_response: &str, // reserved for Phase 2 when streaming response is collected
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

    // For Phase 0: extract facts from the user message only (avoids a second LLM call).
    // Phase 2+ will include the full assistant response for richer memory candidates.
    let prompt = format!(
        r###"你是 灵枢（LingShu）的记忆抽取系统。请从用户最新一条消息中提取值得长期记住的信息。

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

用户消息：{user_message}

JSON 数组："###,
        user_message = user_message
    );

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
        match save_memory(db, user_id, &c.memory_type, &c.content, c.importance).await {
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
}
