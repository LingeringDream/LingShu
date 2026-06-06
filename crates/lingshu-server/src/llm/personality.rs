//! Personality evolution engine.
//!
//! Analyses the user's long-term memories and proposes small, conservative
//! adjustments to the 7 personality traits.  Results are saved as a new
//! active [`PersonalitySnapshot`] only when the LLM is sufficiently confident
//! and at least one trait shifts by a meaningful amount.
//!
//! This engine can be manually triggered through the personality route, and
//! the chat post-stream path may trigger it automatically behind a per-user
//! cooldown.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::models::personality::{PersonalitySnapshot, PersonalityTraits};
use sqlx::PgPool;

// ── Thresholds ────────────────────────────────────────────────────

/// Minimum LLM confidence before a snapshot is saved.
const MIN_CONFIDENCE: f32 = 0.65;

/// Maximum single-trait change per evolution round (conservative).
const MAX_TRAIT_DELTA: f32 = 0.10;

/// Minimum absolute delta on any trait to consider the round meaningful.
const MIN_MEANINGFUL_DELTA: f32 = 0.03;

/// Minimum seconds between automatic personality evolution triggers.
/// Manual `POST /api/v1/personality/evolve` bypasses this cooldown.
const PERSONALITY_EVOLUTION_COOLDOWN_SECS: u64 = 24 * 60 * 60; // 24 hours

// ── Cooldown (auto-trigger only) ──────────────────────────────────

static LAST_EVOLUTIONS: OnceLock<Mutex<HashMap<Uuid, u64>>> = OnceLock::new();

/// Check whether the per-user cooldown allows an automatic evolution
/// trigger. Used by the chat post-stream path; the manual endpoint
/// (`POST /api/v1/personality/evolve`) does **not** call this.
pub fn should_evolve_personality(user_id: Uuid) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut map = last_evolutions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    should_evolve_personality_at(&mut map, user_id, now)
}

/// Pure helper — testable without real system clock.
pub fn should_evolve_personality_at(
    last: &mut HashMap<Uuid, u64>,
    user_id: Uuid,
    now: u64,
) -> bool {
    if let Some(prev) = last.get(&user_id) {
        if now.saturating_sub(*prev) < PERSONALITY_EVOLUTION_COOLDOWN_SECS {
            return false;
        }
    }
    last.insert(user_id, now);
    true
}

fn last_evolutions() -> &'static Mutex<HashMap<Uuid, u64>> {
    LAST_EVOLUTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── Public types ──────────────────────────────────────────────────

/// Outcome of [`evolve_and_save_personality`].
pub struct PersonalityEvolutionOutcome {
    pub created: bool,
    pub reason: String,
    pub snapshot: Option<PersonalitySnapshot>,
}

/// A memory record used as input to the evolution prompt.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PersonalityMemoryContext {
    pub id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: f32,
}

// ── LLM output shape ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct EvolutionLLMOutput {
    trait_values: PersonalityTraits,
    change_reason: String,
    confidence: f32,
    #[serde(default)]
    source_memory_ids: Vec<Uuid>,
}

// ── Public API ────────────────────────────────────────────────────

/// Analyse memories and, when warranted, create a new active personality snapshot.
pub async fn evolve_and_save_personality(
    db: &PgPool,
    llm: &LlmClient,
    model: &str,
    user_id: Uuid,
) -> anyhow::Result<PersonalityEvolutionOutcome> {
    // 1. Load current traits (active snapshot or default)
    let current = load_active_traits(db, user_id).await;

    // 2. Load evolution evidence
    let memories = load_evolution_memories(db, user_id).await?;
    if memories.is_empty() {
        return Ok(PersonalityEvolutionOutcome {
            created: false,
            reason: "no memories".to_string(),
            snapshot: None,
        });
    }

    // 3. Build prompt and call LLM
    let prompt = build_personality_evolution_prompt(&current, &memories);
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];
    let response = llm.chat(model, messages, None).await?;

    // 4. Parse output
    let output = parse_evolution_output(&response)?;

    // 5. Clamp and constrain
    let clamped = clamp_and_filter_traits(&output.trait_values, &current);
    let max_delta = max_trait_delta(&current, &clamped);
    let confidence = output.confidence.clamp(0.0, 1.0);

    // 6. Gate: confidence too low or no meaningful change
    if !should_create_snapshot(confidence, max_delta) {
        let reason = if confidence < MIN_CONFIDENCE {
            format!("confidence {confidence:.3} below threshold {MIN_CONFIDENCE}")
        } else {
            format!("max trait delta {max_delta:.3} below threshold {MIN_MEANINGFUL_DELTA}")
        };
        return Ok(PersonalityEvolutionOutcome {
            created: false,
            reason,
            snapshot: None,
        });
    }

    // 7. Filter source_memory_ids to loaded set
    let valid_memory_ids: Vec<Uuid> = memories.iter().map(|m| m.id).collect();
    let valid_ids = filter_source_memory_ids(&output.source_memory_ids, &valid_memory_ids);

    // 8. Save in a transaction
    let trait_json =
        serde_json::to_value(&clamped).map_err(|e| anyhow::anyhow!("serialize traits: {e}"))?;
    let reason = format!("auto-evolution: {}", output.change_reason);

    let mut tx = db.begin().await?;
    sqlx::query("UPDATE personality_snapshots SET is_active = false WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    let snapshot: PersonalitySnapshot = sqlx::query_as(
        "INSERT INTO personality_snapshots \
         (user_id, trait_values, change_reason, source_memory_ids, is_active) \
         VALUES ($1, $2, $3, $4, true) RETURNING *",
    )
    .bind(user_id)
    .bind(&trait_json)
    .bind(&reason)
    .bind(&valid_ids)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        %user_id,
        snapshot_id = %snapshot.id,
        max_delta = %max_delta,
        %confidence,
        "Created personality evolution snapshot"
    );

    Ok(PersonalityEvolutionOutcome {
        created: true,
        reason,
        snapshot: Some(snapshot),
    })
}

// ── Helpers: load ─────────────────────────────────────────────────

async fn load_active_traits(db: &PgPool, user_id: Uuid) -> PersonalityTraits {
    let row: Option<(serde_json::Value,)> = match sqlx::query_as(
        "SELECT trait_values FROM personality_snapshots \
         WHERE user_id = $1 AND is_active = true LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(%user_id, %e, "Failed to load active personality snapshot");
            return PersonalityTraits::default();
        }
    };

    let Some((trait_values,)) = row else {
        return PersonalityTraits::default();
    };

    match serde_json::from_value(trait_values) {
        Ok(traits) => traits,
        Err(e) => {
            tracing::warn!(%user_id, %e, "Failed to parse active personality snapshot");
            PersonalityTraits::default()
        }
    }
}

async fn load_evolution_memories(
    db: &PgPool,
    user_id: Uuid,
) -> anyhow::Result<Vec<PersonalityMemoryContext>> {
    let rows: Vec<PersonalityMemoryContext> = sqlx::query_as(
        "SELECT id, memory_type, content, importance FROM memories \
         WHERE user_id = $1 AND deleted_at IS NULL \
         ORDER BY importance DESC, updated_at DESC LIMIT 20",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

// ── Prompt builder ────────────────────────────────────────────────

pub fn build_personality_evolution_prompt(
    current: &PersonalityTraits,
    memories: &[PersonalityMemoryContext],
) -> String {
    let memory_lines: Vec<String> = memories
        .iter()
        .map(|m| {
            format!(
                "- [{}] (importance={:.2}, id={}) {}",
                m.memory_type, m.importance, m.id, m.content
            )
        })
        .collect();

    let memory_section = if memory_lines.is_empty() {
        "（无记忆）".to_string()
    } else {
        memory_lines.join("\n")
    };

    format!(
        r###"你是灵枢（LingShu）的人格演化引擎。请根据用户长期记忆判断 7 个人格 trait 是否需要小幅调整。

## 当前人格参数
- directness: {directness:.2}
- warmth: {warmth:.2}
- proactivity: {proactivity:.2}
- risk_tolerance: {risk_tolerance:.2}
- verbosity: {verbosity:.2}
- formality: {formality:.2}
- humor: {humor:.2}

## 用户长期记忆（按重要性排序）
{memory_section}

## 调整规则
- **directness**：用户偏好直接了当还是委婉含蓄？经常使用「建议」「或许」→ 降低；频繁表达明确意见 → 提高。
- **warmth**：用户表现热情友善还是冷静疏离？频繁感谢/鼓励 → 提高；语气简短冷淡 → 降低。
- **proactivity**：用户喜欢助手主动建议还是被动回应？经常问「还有什么…」→ 提高；只说具体需求 → 降低。
- **risk_tolerance**：用户倾向冒险还是谨慎？常说「试试」「没关系」→ 提高；反复要求确认 → 降低。
- **verbosity**：用户喜欢详细还是简洁？要求展开/解释 → 提高；说「简单说」「精简」→ 降低。
- **formality**：用户偏好正式还是随性？使用敬语/正式表达 → 提高；用「行」「好的」「嗯」→ 降低。
- **humor**：用户喜欢幽默还是严肃？主动调侃/轻松氛围 → 提高；始终严肃 → 降低。

## 重要约束
- 没有足够证据时，保持当前值不变。
- 每轮调整应保守，每个 trait 变化建议不超过 ±0.1。
- 不要根据单一记忆过度调整。
- 置信度（confidence）应如实反映证据的充分程度；证据薄弱时降低。

## 输出格式
严格返回一个 JSON 对象（不要数组、不要 markdown fence）：

{{
  "trait_values": {{
    "directness": 0.0-1.0,
    "warmth": 0.0-1.0,
    "proactivity": 0.0-1.0,
    "risk_tolerance": 0.0-1.0,
    "verbosity": 0.0-1.0,
    "formality": 0.0-1.0,
    "humor": 0.0-1.0
  }},
  "change_reason": "简述最关键的调整理由，1-2 句",
  "confidence": 0.0-1.0,
  "source_memory_ids": ["uuid1", "uuid2"]
}}

JSON 对象："###,
        directness = current.directness,
        warmth = current.warmth,
        proactivity = current.proactivity,
        risk_tolerance = current.risk_tolerance,
        verbosity = current.verbosity,
        formality = current.formality,
        humor = current.humor,
    )
}

// ── Output parsing ────────────────────────────────────────────────

fn parse_evolution_output(raw: &str) -> anyhow::Result<EvolutionLLMOutput> {
    let text = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let slice = match (text.find('{'), text.rfind('}')) {
        (Some(start), Some(end)) if start < end => &text[start..=end],
        _ => text,
    };

    let output: EvolutionLLMOutput = serde_json::from_str(slice)?;
    Ok(output)
}

// ── Pure helpers (public for testing) ─────────────────────────────

/// Clamp trait values to [0,1] and limit each delta vs current to ±MAX_TRAIT_DELTA.
pub fn clamp_and_filter_traits(
    target: &PersonalityTraits,
    current: &PersonalityTraits,
) -> PersonalityTraits {
    let step = |target: f32, current: f32| {
        let current = current.clamp(0.0, 1.0);
        let target = target.clamp(0.0, 1.0);
        (current + (target - current).clamp(-MAX_TRAIT_DELTA, MAX_TRAIT_DELTA)).clamp(0.0, 1.0)
    };

    PersonalityTraits {
        directness: step(target.directness, current.directness),
        warmth: step(target.warmth, current.warmth),
        proactivity: step(target.proactivity, current.proactivity),
        risk_tolerance: step(target.risk_tolerance, current.risk_tolerance),
        verbosity: step(target.verbosity, current.verbosity),
        formality: step(target.formality, current.formality),
        humor: step(target.humor, current.humor),
    }
}

/// Largest absolute delta across all 7 traits.
pub fn max_trait_delta(current: &PersonalityTraits, target: &PersonalityTraits) -> f32 {
    let deltas = [
        (target.directness - current.directness).abs(),
        (target.warmth - current.warmth).abs(),
        (target.proactivity - current.proactivity).abs(),
        (target.risk_tolerance - current.risk_tolerance).abs(),
        (target.verbosity - current.verbosity).abs(),
        (target.formality - current.formality).abs(),
        (target.humor - current.humor).abs(),
    ];
    deltas.into_iter().fold(0.0f32, f32::max)
}

/// Gate: confidence must be high enough AND at least one trait changed meaningfully.
pub fn should_create_snapshot(confidence: f32, max_delta: f32) -> bool {
    confidence >= MIN_CONFIDENCE && max_delta >= MIN_MEANINGFUL_DELTA
}

/// Keep only the `source_memory_ids` that appear in the `valid_set`.
pub fn filter_source_memory_ids(ids: &[Uuid], valid_set: &[Uuid]) -> Vec<Uuid> {
    let mut filtered = Vec::new();
    for id in ids {
        if valid_set.contains(id) && !filtered.contains(id) {
            filtered.push(*id);
        }
    }
    filtered
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_traits(directness: f32) -> PersonalityTraits {
        PersonalityTraits {
            directness,
            ..Default::default()
        }
    }

    // ── Prompt tests ────────────────────────────────────────────

    #[test]
    fn prompt_contains_all_7_current_traits() {
        let prompt = build_personality_evolution_prompt(&PersonalityTraits::default(), &[]);
        for name in &[
            "directness",
            "warmth",
            "proactivity",
            "risk_tolerance",
            "verbosity",
            "formality",
            "humor",
        ] {
            assert!(
                prompt.contains(name),
                "prompt should contain trait '{name}'"
            );
        }
    }

    #[test]
    fn prompt_contains_memory_fields() {
        let mem = PersonalityMemoryContext {
            id: Uuid::new_v4(),
            memory_type: "preference".to_string(),
            content: "喜欢安静的环境".to_string(),
            importance: 0.85,
        };
        let prompt = build_personality_evolution_prompt(&PersonalityTraits::default(), &[mem]);
        assert!(prompt.contains("喜欢安静的环境"));
        assert!(prompt.contains("0.85"));
        assert!(prompt.contains("preference"));
    }

    #[test]
    fn prompt_no_memories_shows_placeholder() {
        let prompt = build_personality_evolution_prompt(&PersonalityTraits::default(), &[]);
        assert!(prompt.contains("无记忆"));
    }

    #[test]
    fn prompt_requires_json_object_output() {
        let prompt = build_personality_evolution_prompt(&PersonalityTraits::default(), &[]);
        assert!(prompt.contains("JSON 对象"));
        assert!(prompt.contains("trait_values"));
        assert!(prompt.contains("change_reason"));
        assert!(prompt.contains("source_memory_ids"));
    }

    // ── Clamp & delta tests ────────────────────────────────────

    #[test]
    fn clamp_and_filter_respects_max_delta() {
        let current = PersonalityTraits::default(); // all 0.5
        let target = PersonalityTraits {
            directness: 1.0, // LLM proposes big jump
            ..Default::default()
        };
        let result = clamp_and_filter_traits(&target, &current);
        assert!((result.directness - 0.60).abs() < f32::EPSILON); // 0.5 + 0.10
        assert!((result.warmth - 0.5).abs() < f32::EPSILON); // unchanged
    }

    #[test]
    fn clamp_and_filter_respects_0_1_bounds() {
        let current = PersonalityTraits {
            directness: 0.95,
            warmth: 0.05,
            ..Default::default()
        };
        let target = PersonalityTraits {
            directness: 2.0, // way over
            warmth: -1.0,    // way under
            ..Default::default()
        };
        let result = clamp_and_filter_traits(&target, &current);
        assert!(result.directness <= 1.0);
        assert!(result.warmth >= 0.0);
    }

    #[test]
    fn clamp_and_filter_clamps_out_of_bounds_current_values() {
        let current = PersonalityTraits {
            directness: 1.2,
            warmth: -0.2,
            ..Default::default()
        };
        let target = PersonalityTraits::default();
        let result = clamp_and_filter_traits(&target, &current);
        assert!((0.0..=1.0).contains(&result.directness));
        assert!((0.0..=1.0).contains(&result.warmth));
    }

    #[test]
    fn clamp_and_filter_delta_negative_direction() {
        let current = PersonalityTraits {
            directness: 0.7,
            ..Default::default()
        };
        let target = PersonalityTraits {
            directness: 0.1, // LLM proposes drop
            ..Default::default()
        };
        let result = clamp_and_filter_traits(&target, &current);
        // delta = clamp(0.1) - 0.7 = -0.6, clamped to -0.10
        assert!((result.directness - 0.60).abs() < f32::EPSILON);
    }

    // ── max_trait_delta tests ──────────────────────────────────

    #[test]
    fn max_delta_zero_when_identical() {
        let t = PersonalityTraits::default();
        assert!((max_trait_delta(&t, &t) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn max_delta_captures_largest_change() {
        let current = PersonalityTraits::default();
        let target = PersonalityTraits {
            warmth: 0.55,
            proactivity: 0.62,
            ..Default::default()
        };
        let d = max_trait_delta(&current, &target);
        assert!((d - 0.12).abs() < f32::EPSILON); // proactivity delta is 0.12, which is the max
    }

    // Wait, proactivity goes from 0.5 → 0.62, delta = 0.12, but max delta limit is 0.10.
    // max_trait_delta operates on raw values (before clamping by clamp_and_filter_traits).
    // Actually the test is testing max_trait_delta on raw values, which is 0.12. That's correct.
    // But after clamp_and_filter_traits, it would be capped to 0.10.

    #[test]
    fn max_delta_small_when_close() {
        let t = PersonalityTraits::default();
        let t2 = PersonalityTraits {
            directness: 0.51,
            ..Default::default()
        };
        let d = max_trait_delta(&t, &t2);
        assert!((d - 0.01).abs() < f32::EPSILON);
    }

    // ── Gate tests ─────────────────────────────────────────────

    #[test]
    fn should_create_snapshot_passes_with_high_confidence_and_delta() {
        assert!(should_create_snapshot(0.8, 0.05));
    }

    #[test]
    fn should_create_snapshot_fails_on_low_confidence() {
        assert!(!should_create_snapshot(0.5, 0.10));
    }

    #[test]
    fn should_create_snapshot_fails_on_small_delta() {
        assert!(!should_create_snapshot(0.9, 0.01));
    }

    #[test]
    fn should_create_snapshot_fails_on_both_low() {
        assert!(!should_create_snapshot(0.4, 0.01));
    }

    #[test]
    fn should_create_boundary_confidence_exactly_065() {
        assert!(should_create_snapshot(0.65, 0.05));
        assert!(!should_create_snapshot(0.649999, 0.05));
    }

    // ── source_memory_ids filter tests ──────────────────────────

    #[test]
    fn filter_keeps_only_valid_ids() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let ids = vec![id1, id2, id3];
        let valid = vec![id1, id3];
        let filtered = filter_source_memory_ids(&ids, &valid);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&id1));
        assert!(filtered.contains(&id3));
        assert!(!filtered.contains(&id2));
    }

    #[test]
    fn filter_returns_empty_when_none_valid() {
        let id1 = Uuid::new_v4();
        let ids = vec![id1];
        let valid: Vec<Uuid> = vec![];
        assert!(filter_source_memory_ids(&ids, &valid).is_empty());
    }

    #[test]
    fn filter_deduplicates_valid_ids() {
        let id1 = Uuid::new_v4();
        let ids = vec![id1, id1];
        let valid = vec![id1];
        assert_eq!(filter_source_memory_ids(&ids, &valid), vec![id1]);
    }

    // ── Parse tests ────────────────────────────────────────────

    #[test]
    fn parse_evolution_output_with_fence() {
        let raw = r#"```json
{"trait_values":{"directness":0.6,"warmth":0.5,"proactivity":0.5,"risk_tolerance":0.5,"verbosity":0.5,"formality":0.5,"humor":0.5},"change_reason":"用户偏好更直接","confidence":0.75,"source_memory_ids":[]}
```"#;
        let out = parse_evolution_output(raw).expect("should parse");
        assert!((out.trait_values.directness - 0.6).abs() < f32::EPSILON);
        assert_eq!(out.change_reason, "用户偏好更直接");
        assert!((out.confidence - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_evolution_output_without_fence() {
        let raw = r#"{"trait_values":{"directness":0.5,"warmth":0.55,"proactivity":0.5,"risk_tolerance":0.5,"verbosity":0.5,"formality":0.5,"humor":0.5},"change_reason":"用户更热情","confidence":0.8,"source_memory_ids":[]}"#;
        let out = parse_evolution_output(raw).expect("should parse");
        assert!((out.trait_values.warmth - 0.55).abs() < f32::EPSILON);
        assert!(out.confidence >= 0.0 && out.confidence <= 1.0);
    }

    // ── Cooldown tests ──────────────────────────────────────────

    #[test]
    fn cooldown_allows_first_call() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_evolve_personality_at(&mut map, user, 100));
    }

    #[test]
    fn cooldown_blocks_second_call_within_window() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_evolve_personality_at(&mut map, user, 100));
        assert!(!should_evolve_personality_at(&mut map, user, 100 + 3600)); // 1h < 24h
    }

    #[test]
    fn cooldown_is_per_user() {
        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_evolve_personality_at(&mut map, user_a, 100));
        assert!(should_evolve_personality_at(&mut map, user_b, 100));
        assert!(!should_evolve_personality_at(&mut map, user_a, 200));
    }

    #[test]
    fn cooldown_allows_after_window_expires() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_evolve_personality_at(&mut map, user, 100));
        let after_cooldown = 100 + PERSONALITY_EVOLUTION_COOLDOWN_SECS;
        assert!(should_evolve_personality_at(&mut map, user, after_cooldown));
    }

    #[test]
    fn cooldown_allows_exactly_at_boundary() {
        let user = Uuid::new_v4();
        let mut map = HashMap::new();
        assert!(should_evolve_personality_at(&mut map, user, 100));
        let before_boundary = 100 + PERSONALITY_EVOLUTION_COOLDOWN_SECS - 1;
        let at_boundary = 100 + PERSONALITY_EVOLUTION_COOLDOWN_SECS;
        assert!(!should_evolve_personality_at(
            &mut map,
            user,
            before_boundary
        ));
        assert!(should_evolve_personality_at(&mut map, user, at_boundary));
    }
}
