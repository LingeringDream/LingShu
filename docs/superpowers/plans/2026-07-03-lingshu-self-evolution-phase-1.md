# LingShu Self Evolution Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first self-evolution feedback loop: record evolution runs, include user feedback signals in personality evolution, expose recent evolution history, and show the latest evolution reason in Personality Center.

**Architecture:** Add a small backend evolution ledger and evidence layer around the existing personality evolution engine. Keep the existing `personality_snapshots` table as the source of active personality traits, and use the new `evolution_runs` table for auditability, skipped reasons, and UI visibility.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, Utoipa OpenAPI, React, TypeScript, Vitest, Testing Library.

---

## Scope

This plan implements Phase 1 from `docs/superpowers/specs/2026-07-03-lingshu-self-evolution-design.md`.

Included:

- Evolution ledger table and Rust model.
- Personality evolution evidence from `reply_*` and `personality_slider_changed` signal events.
- Ledger writes for created and skipped personality evolution outcomes.
- Failure ledger writes from the manual route and chat background path.
- `GET /api/v1/evolution/runs`.
- Personality Center display of the latest personality evolution run.
- Focused backend and frontend tests.
- OpenAPI contract regeneration.

Deferred to separate plans:

- Thought category weights and suggestion policy.
- Task/calendar preference learning.
- Rule candidate approval, rejection, and rollback.
- Desktop pet policy changes.

## File Structure

- Create `crates/lingshu-server/migrations/20260703000023_create_evolution_runs_table.sql`
  - Defines the append-only evolution ledger.
- Create `crates/lingshu-server/src/models/evolution.rs`
  - Defines `EvolutionRun`, `EvolutionScope`, and `EvolutionRunStatus`.
- Modify `crates/lingshu-server/src/models/mod.rs`
  - Exposes the new model module.
- Create `crates/lingshu-server/src/evolution.rs`
  - Loads feedback evidence and records evolution ledger rows.
- Modify `crates/lingshu-server/src/main.rs`
  - Registers the new domain module and route.
- Create `crates/lingshu-server/src/routes/evolution.rs`
  - Implements `GET /api/v1/evolution/runs`.
- Modify `crates/lingshu-server/src/routes/mod.rs`
  - Registers OpenAPI path and schema entries.
- Modify `crates/lingshu-server/src/routes/personality.rs`
  - Records `failed` evolution runs when manual evolution errors.
- Modify `crates/lingshu-server/src/routes/chat.rs`
  - Records `failed` evolution runs when background auto-evolution errors.
- Modify `crates/lingshu-server/src/llm/personality.rs`
  - Adds style feedback evidence to the prompt and records `applied` / `skipped` runs.
- Modify `frontend/src/components/personality/PersonalityCenter.tsx`
  - Fetches and displays the latest personality evolution run.
- Create `frontend/src/components/personality/__tests__/PersonalityCenter.test.tsx`
  - Verifies the new UI state and request behavior.
- Modify `openapi.json`
  - Regenerated from live server code after route/schema changes.

---

### Task 1: Evolution Ledger Schema And Model

**Files:**
- Create: `crates/lingshu-server/migrations/20260703000023_create_evolution_runs_table.sql`
- Create: `crates/lingshu-server/src/models/evolution.rs`
- Modify: `crates/lingshu-server/src/models/mod.rs`

- [ ] **Step 1: Add model tests first**

Create `crates/lingshu-server/src/models/evolution.rs` with the tests and the minimum type shell needed for those tests:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[serde(rename_all = "snake_case")]
pub enum EvolutionScope {
    #[sqlx(rename = "personality")]
    Personality,
}

impl EvolutionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Personality => "personality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[serde(rename_all = "snake_case")]
pub enum EvolutionRunStatus {
    #[sqlx(rename = "applied")]
    Applied,
    #[sqlx(rename = "skipped")]
    Skipped,
    #[sqlx(rename = "failed")]
    Failed,
    #[sqlx(rename = "review_required")]
    ReviewRequired,
}

impl EvolutionRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::ReviewRequired => "review_required",
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct EvolutionRun {
    pub id: Uuid,
    pub user_id: Uuid,
    pub scope: EvolutionScope,
    pub status: EvolutionRunStatus,
    pub evidence_summary: serde_json::Value,
    pub proposed_changes: serde_json::Value,
    pub applied_changes: serde_json::Value,
    pub skipped_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_serializes_to_snake_case() {
        assert_eq!(EvolutionScope::Personality.as_str(), "personality");
        assert_eq!(
            serde_json::to_string(&EvolutionScope::Personality).unwrap(),
            "\"personality\""
        );
    }

    #[test]
    fn run_status_serializes_to_snake_case() {
        assert_eq!(EvolutionRunStatus::Applied.as_str(), "applied");
        assert_eq!(EvolutionRunStatus::Skipped.as_str(), "skipped");
        assert_eq!(EvolutionRunStatus::Failed.as_str(), "failed");
        assert_eq!(EvolutionRunStatus::ReviewRequired.as_str(), "review_required");
    }
}
```

- [ ] **Step 2: Expose the model module**

Modify `crates/lingshu-server/src/models/mod.rs`:

```rust
pub mod audit_log;
pub mod calendar;
pub mod conversation;
pub mod evolution;
pub mod integration;
pub mod memory;
pub mod message;
pub mod personality;
pub mod project;
pub mod project_member;
pub mod provider;
pub mod task;
pub mod task_dependency;
pub mod thought;
pub mod user;
```

- [ ] **Step 3: Run the focused model tests**

Run:

```bash
cargo test -p lingshu-server models::evolution -- --nocapture
```

Expected: PASS, with both `models::evolution::tests::*` tests passing.

- [ ] **Step 4: Add the migration**

Create `crates/lingshu-server/migrations/20260703000023_create_evolution_runs_table.sql`:

```sql
-- Append-only ledger for self-evolution decisions.
-- Personality snapshots remain the source of active trait values; this table
-- records why an evolution run applied, skipped, failed, or required review.

CREATE TABLE evolution_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope VARCHAR(40) NOT NULL
        CHECK (scope IN ('personality')),
    status VARCHAR(20) NOT NULL
        CHECK (status IN ('applied', 'skipped', 'failed', 'review_required')),
    evidence_summary JSONB NOT NULL DEFAULT '{}',
    proposed_changes JSONB NOT NULL DEFAULT '[]',
    applied_changes JSONB NOT NULL DEFAULT '[]',
    skipped_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_evolution_runs_user_scope_created
    ON evolution_runs(user_id, scope, created_at DESC);

CREATE INDEX idx_evolution_runs_status
    ON evolution_runs(user_id, status, created_at DESC);
```

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/lingshu-server/migrations/20260703000023_create_evolution_runs_table.sql \
  crates/lingshu-server/src/models/evolution.rs \
  crates/lingshu-server/src/models/mod.rs
git commit -m "feat: add evolution run ledger"
```

---

### Task 2: Evolution Evidence And Ledger Helpers

**Files:**
- Create: `crates/lingshu-server/src/evolution.rs`
- Modify: `crates/lingshu-server/src/main.rs`

- [ ] **Step 1: Write pure evidence tests**

Create `crates/lingshu-server/src/evolution.rs` with these definitions and tests:

```rust
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::evolution::{EvolutionRun, EvolutionRunStatus, EvolutionScope};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyleFeedbackSummary {
    pub window_days: i32,
    pub reply_thumb_up: i64,
    pub reply_thumb_down: i64,
    pub too_long: i64,
    pub too_short: i64,
    pub too_formal: i64,
    pub personality_slider_changed: i64,
}

impl StyleFeedbackSummary {
    pub fn has_feedback(&self) -> bool {
        self.reply_thumb_up > 0
            || self.reply_thumb_down > 0
            || self.too_long > 0
            || self.too_short > 0
            || self.too_formal > 0
            || self.personality_slider_changed > 0
    }

    pub fn sample_count(&self) -> i64 {
        self.reply_thumb_up
            + self.reply_thumb_down
            + self.too_long
            + self.too_short
            + self.too_formal
            + self.personality_slider_changed
    }
}

pub fn personality_evidence_summary(
    memory_count: usize,
    style: &StyleFeedbackSummary,
) -> serde_json::Value {
    serde_json::json!({
        "memory_count": memory_count,
        "style_feedback": style,
    })
}

pub fn personality_changes_json(
    trait_values: &crate::models::personality::PersonalityTraits,
    confidence: f32,
    max_delta: f32,
    change_reason: &str,
) -> serde_json::Value {
    serde_json::json!([{
        "type": "personality_traits",
        "trait_values": trait_values,
        "confidence": confidence,
        "max_delta": max_delta,
        "change_reason": change_reason,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::personality::PersonalityTraits;

    #[test]
    fn style_feedback_detects_any_signal() {
        let empty = StyleFeedbackSummary { window_days: 30, ..Default::default() };
        assert!(!empty.has_feedback());
        assert_eq!(empty.sample_count(), 0);

        let with_tag = StyleFeedbackSummary { window_days: 30, too_long: 2, ..Default::default() };
        assert!(with_tag.has_feedback());
        assert_eq!(with_tag.sample_count(), 2);
    }

    #[test]
    fn personality_evidence_summary_contains_memory_count_and_feedback() {
        let style = StyleFeedbackSummary {
            window_days: 30,
            reply_thumb_down: 1,
            too_long: 3,
            ..Default::default()
        };
        let json = personality_evidence_summary(4, &style);
        assert_eq!(json["memory_count"], 4);
        assert_eq!(json["style_feedback"]["too_long"], 3);
        assert_eq!(json["style_feedback"]["reply_thumb_down"], 1);
    }

    #[test]
    fn personality_changes_json_contains_a_single_change() {
        let traits = PersonalityTraits { verbosity: 0.4, ..Default::default() };
        let json = personality_changes_json(&traits, 0.8, 0.1, "用户多次反馈回答太长");
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["type"], "personality_traits");
        assert_eq!(json[0]["trait_values"]["verbosity"], 0.4);
        assert_eq!(json[0]["confidence"], 0.8);
    }
}
```

- [ ] **Step 2: Run tests to verify the pure helpers pass**

Run:

```bash
cargo test -p lingshu-server evolution::tests -- --nocapture
```

Expected: PASS for the three pure helper tests.

- [ ] **Step 3: Add database helper functions**

Append this code to `crates/lingshu-server/src/evolution.rs` after the pure helpers and before the test module:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
struct StyleFeedbackRow {
    event_type: String,
    tag: Option<String>,
    count: i64,
}

pub async fn load_style_feedback_summary(
    db: &PgPool,
    user_id: Uuid,
    window_days: i32,
) -> anyhow::Result<StyleFeedbackSummary> {
    let rows: Vec<StyleFeedbackRow> = sqlx::query_as(
        "SELECT event_type, metadata->>'tag' AS tag, COUNT(*) AS count \
         FROM signal_events \
         WHERE user_id = $1 \
           AND created_at >= NOW() - ($2::int * INTERVAL '1 day') \
           AND event_type IN (
             'reply_thumb_up',
             'reply_thumb_down',
             'reply_style_tag',
             'personality_slider_changed'
           ) \
         GROUP BY event_type, metadata->>'tag'",
    )
    .bind(user_id)
    .bind(window_days)
    .fetch_all(db)
    .await?;

    let mut summary = StyleFeedbackSummary {
        window_days,
        ..Default::default()
    };

    for row in rows {
        match (row.event_type.as_str(), row.tag.as_deref()) {
            ("reply_thumb_up", _) => summary.reply_thumb_up += row.count,
            ("reply_thumb_down", _) => summary.reply_thumb_down += row.count,
            ("personality_slider_changed", _) => {
                summary.personality_slider_changed += row.count;
            }
            ("reply_style_tag", Some("too_long")) => summary.too_long += row.count,
            ("reply_style_tag", Some("too_short")) => summary.too_short += row.count,
            ("reply_style_tag", Some("too_formal")) => summary.too_formal += row.count,
            _ => {}
        }
    }

    Ok(summary)
}

pub async fn record_evolution_run(
    db: &PgPool,
    user_id: Uuid,
    scope: EvolutionScope,
    status: EvolutionRunStatus,
    evidence_summary: serde_json::Value,
    proposed_changes: serde_json::Value,
    applied_changes: serde_json::Value,
    skipped_reason: Option<&str>,
) -> anyhow::Result<EvolutionRun> {
    let run: EvolutionRun = sqlx::query_as(
        "INSERT INTO evolution_runs \
         (user_id, scope, status, evidence_summary, proposed_changes, applied_changes, skipped_reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
    )
    .bind(user_id)
    .bind(scope.as_str())
    .bind(status.as_str())
    .bind(&evidence_summary)
    .bind(&proposed_changes)
    .bind(&applied_changes)
    .bind(skipped_reason)
    .fetch_one(db)
    .await?;

    Ok(run)
}

pub async fn record_personality_failure(
    db: &PgPool,
    user_id: Uuid,
    error: &anyhow::Error,
) {
    let evidence = serde_json::json!({
        "error": error.to_string(),
    });

    if let Err(record_error) = record_evolution_run(
        db,
        user_id,
        EvolutionScope::Personality,
        EvolutionRunStatus::Failed,
        evidence,
        serde_json::Value::Array(Vec::new()),
        serde_json::Value::Array(Vec::new()),
        Some("error"),
    )
    .await
    {
        tracing::warn!(
            %user_id,
            error = %record_error,
            "Failed to record personality evolution failure"
        );
    }
}
```

- [ ] **Step 4: Register the domain module**

Modify `crates/lingshu-server/src/main.rs` near the other modules:

```rust
mod auth;
mod cache;
mod config;
mod cooldown;
mod crypto;
mod db;
mod error;
mod evolution;
mod llm;
mod models;
mod patch;
mod routes;
mod state;
mod telemetry;
```

- [ ] **Step 5: Re-run focused helper tests**

Run:

```bash
cargo test -p lingshu-server evolution::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add crates/lingshu-server/src/evolution.rs crates/lingshu-server/src/main.rs
git commit -m "feat: add evolution evidence helpers"
```

---

### Task 3: Feedback-Aware Personality Evolution

**Files:**
- Modify: `crates/lingshu-server/src/llm/personality.rs`

- [ ] **Step 1: Update prompt tests to require style feedback**

Modify the tests in `crates/lingshu-server/src/llm/personality.rs` so they call the new prompt signature:

```rust
fn sample_style_feedback() -> crate::evolution::StyleFeedbackSummary {
    crate::evolution::StyleFeedbackSummary {
        window_days: 30,
        reply_thumb_down: 1,
        too_long: 2,
        personality_slider_changed: 1,
        ..Default::default()
    }
}

#[test]
fn prompt_contains_style_feedback_summary() {
    let prompt = build_personality_evolution_prompt(
        &PersonalityTraits::default(),
        &[],
        &sample_style_feedback(),
    );
    assert!(prompt.contains("最近 30 天反馈"));
    assert!(prompt.contains("reply_thumb_down: 1"));
    assert!(prompt.contains("too_long: 2"));
    assert!(prompt.contains("personality_slider_changed: 1"));
}
```

Update existing prompt tests from:

```rust
let prompt = build_personality_evolution_prompt(&PersonalityTraits::default(), &[]);
```

to:

```rust
let style = crate::evolution::StyleFeedbackSummary { window_days: 30, ..Default::default() };
let prompt = build_personality_evolution_prompt(&PersonalityTraits::default(), &[], &style);
```

- [ ] **Step 2: Run prompt tests and confirm failure**

Run:

```bash
cargo test -p lingshu-server llm::personality::tests::prompt_contains_style_feedback_summary -- --nocapture
```

Expected: FAIL because `build_personality_evolution_prompt` still accepts two arguments and does not include the style feedback section.

- [ ] **Step 3: Change the prompt signature and body**

Modify `build_personality_evolution_prompt` in `crates/lingshu-server/src/llm/personality.rs`:

```rust
pub fn build_personality_evolution_prompt(
    current: &PersonalityTraits,
    memories: &[PersonalityMemoryContext],
    style_feedback: &crate::evolution::StyleFeedbackSummary,
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

    let style_section = format!(
        "- reply_thumb_up: {reply_thumb_up}\n\
         - reply_thumb_down: {reply_thumb_down}\n\
         - too_long: {too_long}\n\
         - too_short: {too_short}\n\
         - too_formal: {too_formal}\n\
         - personality_slider_changed: {slider}",
        reply_thumb_up = style_feedback.reply_thumb_up,
        reply_thumb_down = style_feedback.reply_thumb_down,
        too_long = style_feedback.too_long,
        too_short = style_feedback.too_short,
        too_formal = style_feedback.too_formal,
        slider = style_feedback.personality_slider_changed,
    );

    format!(
        r###"你是灵枢（LingShu）的人格演化引擎。请根据用户长期记忆和近期反馈判断 7 个人格 trait 是否需要小幅调整。

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

## 最近 {window_days} 天反馈
{style_section}

## 调整规则
- **directness**：用户偏好直接了当还是委婉含蓄？经常使用「建议」「或许」→ 降低；频繁表达明确意见 → 提高。
- **warmth**：用户表现热情友善还是冷静疏离？频繁感谢/鼓励 → 提高；语气简短冷淡 → 降低。
- **proactivity**：用户喜欢助手主动建议还是被动回应？经常问「还有什么…」→ 提高；只说具体需求或多次忽略主动建议 → 降低。
- **risk_tolerance**：用户倾向冒险还是谨慎？常说「试试」「没关系」→ 提高；反复要求确认 → 降低。
- **verbosity**：用户喜欢详细还是简洁？要求展开/解释或 too_short 较多 → 提高；too_long 较多或要求精简 → 降低。
- **formality**：用户偏好正式还是随性？too_formal 较多 → 降低；使用敬语/正式表达 → 提高。
- **humor**：用户喜欢幽默还是严肃？主动调侃/轻松氛围 → 提高；始终严肃 → 降低。

## 重要约束
- 没有足够证据时，保持当前值不变。
- 每轮调整应保守，每个 trait 变化建议不超过 ±0.1。
- 不要根据单一记忆或单一反馈过度调整。
- 手动 slider 反馈权重大于自动推断，但仍要保守。
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
        window_days = style_feedback.window_days,
    )
}
```

- [ ] **Step 4: Load style evidence and record skipped runs**

In `evolve_and_save_personality`, replace the early memory-only flow with:

```rust
let current = load_active_traits(db, user_id).await;
let memories = load_evolution_memories(db, user_id).await?;
let style_feedback = crate::evolution::load_style_feedback_summary(db, user_id, 30).await?;
let evidence_summary =
    crate::evolution::personality_evidence_summary(memories.len(), &style_feedback);

if memories.is_empty() && !style_feedback.has_feedback() {
    let _ = crate::evolution::record_evolution_run(
        db,
        user_id,
        crate::models::evolution::EvolutionScope::Personality,
        crate::models::evolution::EvolutionRunStatus::Skipped,
        evidence_summary,
        serde_json::Value::Array(Vec::new()),
        serde_json::Value::Array(Vec::new()),
        Some("no evidence"),
    )
    .await;

    return Ok(PersonalityEvolutionOutcome {
        created: false,
        reason: "no evidence".to_string(),
        snapshot: None,
    });
}

let prompt = build_personality_evolution_prompt(&current, &memories, &style_feedback);
```

- [ ] **Step 5: Record low-confidence and no-change skips**

Replace the `if !should_create_snapshot(confidence, max_delta)` block with:

```rust
if !should_create_snapshot(confidence, max_delta) {
    let reason = if confidence < MIN_CONFIDENCE {
        format!("confidence {confidence:.3} below threshold {MIN_CONFIDENCE}")
    } else {
        format!("max trait delta {max_delta:.3} below threshold {MIN_MEANINGFUL_DELTA}")
    };

    let proposed_changes = crate::evolution::personality_changes_json(
        &clamped,
        confidence,
        max_delta,
        &output.change_reason,
    );

    let _ = crate::evolution::record_evolution_run(
        db,
        user_id,
        crate::models::evolution::EvolutionScope::Personality,
        crate::models::evolution::EvolutionRunStatus::Skipped,
        evidence_summary,
        proposed_changes,
        serde_json::Value::Array(Vec::new()),
        Some(&reason),
    )
    .await;

    return Ok(PersonalityEvolutionOutcome {
        created: false,
        reason,
        snapshot: None,
    });
}
```

- [ ] **Step 6: Record applied runs after snapshot creation**

After the transaction commit and before memory reference telemetry:

```rust
let applied_changes = crate::evolution::personality_changes_json(
    &clamped,
    confidence,
    max_delta,
    &output.change_reason,
);

let _ = crate::evolution::record_evolution_run(
    db,
    user_id,
    crate::models::evolution::EvolutionScope::Personality,
    crate::models::evolution::EvolutionRunStatus::Applied,
    evidence_summary,
    applied_changes.clone(),
    applied_changes,
    None,
)
.await;
```

- [ ] **Step 7: Run personality unit tests**

Run:

```bash
cargo test -p lingshu-server llm::personality::tests -- --nocapture
```

Expected: PASS for existing clamp, parse, prompt, and cooldown tests.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add crates/lingshu-server/src/llm/personality.rs
git commit -m "feat: include feedback in personality evolution"
```

---

### Task 4: Failure Ledger Writes From Evolution Callers

**Files:**
- Modify: `crates/lingshu-server/src/routes/personality.rs`
- Modify: `crates/lingshu-server/src/routes/chat.rs`

- [ ] **Step 1: Update manual personality evolution error path**

In `crates/lingshu-server/src/routes/personality.rs`, replace the direct `.map_err(AppError::Internal)?` call with an explicit match:

```rust
let outcome = match crate::llm::personality::evolve_and_save_personality(
    &state.db,
    &state.llm,
    &settings.model,
    user_id,
)
.await
{
    Ok(outcome) => outcome,
    Err(error) => {
        crate::evolution::record_personality_failure(&state.db, user_id, &error).await;
        return Err(AppError::Internal(error));
    }
};
```

- [ ] **Step 2: Update background personality evolution error path**

In `crates/lingshu-server/src/routes/chat.rs`, inside the `Err(e)` branch for `evolve_and_save_personality`, add the failure record call before logging:

```rust
Err(e) => {
    crate::evolution::record_personality_failure(&db, user_id, &e).await;
    tracing::warn!(%user_id, %e, "Personality evolution failed");
}
```

- [ ] **Step 3: Run targeted backend tests**

Run:

```bash
cargo test -p lingshu-server routes::personality::tests llm::personality::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit Task 4**

Run:

```bash
git add crates/lingshu-server/src/routes/personality.rs crates/lingshu-server/src/routes/chat.rs
git commit -m "feat: record personality evolution failures"
```

---

### Task 5: Evolution Runs API

**Files:**
- Create: `crates/lingshu-server/src/routes/evolution.rs`
- Modify: `crates/lingshu-server/src/routes/mod.rs`
- Modify: `crates/lingshu-server/src/main.rs`

- [ ] **Step 1: Create route tests for parameter helpers**

Create `crates/lingshu-server/src/routes/evolution.rs`:

```rust
use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::evolution::{EvolutionRun, EvolutionRunStatus};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/evolution/runs", get(list_evolution_runs))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListEvolutionRunsParams {
    pub scope: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EvolutionRunResponse {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub scope: String,
    pub status: String,
    pub evidence_summary: serde_json::Value,
    pub proposed_changes: serde_json::Value,
    pub applied_changes: serde_json::Value,
    pub skipped_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<EvolutionRun> for EvolutionRunResponse {
    fn from(run: EvolutionRun) -> Self {
        Self {
            id: run.id,
            user_id: run.user_id,
            scope: run.scope.as_str().to_string(),
            status: run.status.as_str().to_string(),
            evidence_summary: run.evidence_summary,
            proposed_changes: run.proposed_changes,
            applied_changes: run.applied_changes,
            skipped_reason: run.skipped_reason,
            created_at: run.created_at,
        }
    }
}

fn clamped_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(20).clamp(1, 100)
}

fn normalize_scope(scope: Option<String>) -> Result<Option<String>, AppError> {
    match scope.as_deref() {
        None | Some("") => Ok(None),
        Some("personality") => Ok(Some("personality".to_string())),
        Some(other) => Err(AppError::Validation(format!(
            "Invalid evolution scope '{other}'. Allowed: personality"
        ))),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/evolution/runs",
    params(ListEvolutionRunsParams),
    responses((status = 200, body = Vec<EvolutionRunResponse>))
)]
async fn list_evolution_runs(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    axum::extract::Query(params): axum::extract::Query<ListEvolutionRunsParams>,
) -> Result<Json<Vec<EvolutionRunResponse>>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let limit = clamped_limit(params.limit);
    let scope = normalize_scope(params.scope)?;

    let runs: Vec<EvolutionRun> = if let Some(scope) = scope {
        sqlx::query_as(
            "SELECT * FROM evolution_runs \
             WHERE user_id = $1 AND scope = $2 \
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(user_id)
        .bind(scope)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT * FROM evolution_runs \
             WHERE user_id = $1 \
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(runs.into_iter().map(EvolutionRunResponse::from).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_defaults_and_clamps() {
        assert_eq!(clamped_limit(None), 20);
        assert_eq!(clamped_limit(Some(0)), 1);
        assert_eq!(clamped_limit(Some(50)), 50);
        assert_eq!(clamped_limit(Some(500)), 100);
    }

    #[test]
    fn scope_accepts_personality_and_empty_values() {
        assert_eq!(normalize_scope(None).unwrap(), None);
        assert_eq!(normalize_scope(Some(String::new())).unwrap(), None);
        assert_eq!(
            normalize_scope(Some("personality".to_string())).unwrap(),
            Some("personality".to_string())
        );
    }

    #[test]
    fn scope_rejects_unknown_values() {
        let err = normalize_scope(Some("pet".to_string())).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn response_uses_snake_case_strings() {
        let run = EvolutionRun {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            scope: crate::models::evolution::EvolutionScope::Personality,
            status: EvolutionRunStatus::Skipped,
            evidence_summary: serde_json::json!({"memory_count": 0}),
            proposed_changes: serde_json::json!([]),
            applied_changes: serde_json::json!([]),
            skipped_reason: Some("no evidence".to_string()),
            created_at: chrono::Utc::now(),
        };
        let response = EvolutionRunResponse::from(run);
        assert_eq!(response.scope, "personality");
        assert_eq!(response.status, "skipped");
        assert_eq!(response.skipped_reason.as_deref(), Some("no evidence"));
    }
}
```

- [ ] **Step 2: Run route helper tests**

Run:

```bash
cargo test -p lingshu-server routes::evolution::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Register module and router**

Modify `crates/lingshu-server/src/routes/mod.rs`:

```rust
pub mod audit;
pub mod auth;
pub mod calendar;
pub mod chat;
pub mod conversations;
pub mod evolution;
pub mod integrations;
pub mod memories;
pub mod permissions;
pub mod personality;
pub mod project_members;
pub mod projects;
pub mod providers;
pub mod sessions;
pub mod settings;
pub mod signals;
pub mod system;
pub mod task_dependencies;
pub mod tasks;
pub mod thoughts;
pub mod users;
```

Add `evolution::list_evolution_runs` to the `paths(...)` list.

Add this schema entry to `components(schemas(...))`:

```rust
evolution::EvolutionRunResponse,
```

Do not add `ListEvolutionRunsParams` to `components(schemas(...))`; it is documented through `params(ListEvolutionRunsParams)`.

Modify `crates/lingshu-server/src/main.rs` router merge list:

```rust
.merge(routes::evolution::router())
```

Place it near the other authenticated API routers, for example after `.merge(routes::personality::router())`.

- [ ] **Step 4: Run OpenAPI route tests**

Run:

```bash
cargo test -p lingshu-server routes::tests::openapi_spec_has_minimum_paths routes::tests::openapi_spec_has_health_check -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit Task 5 without OpenAPI JSON**

Run:

```bash
git add crates/lingshu-server/src/routes/evolution.rs \
  crates/lingshu-server/src/routes/mod.rs \
  crates/lingshu-server/src/main.rs
git commit -m "feat: expose evolution run history"
```

---

### Task 6: Personality Center Evolution Visibility

**Files:**
- Modify: `frontend/src/components/personality/PersonalityCenter.tsx`
- Create: `frontend/src/components/personality/__tests__/PersonalityCenter.test.tsx`

- [ ] **Step 1: Write frontend tests first**

Create `frontend/src/components/personality/__tests__/PersonalityCenter.test.tsx`:

```tsx
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { PersonalityCenter } from '../PersonalityCenter';
import { apiFetch } from '../../../lib/api';

vi.mock('../../../lib/api', () => ({
  apiFetch: vi.fn(),
}));

const mockedApiFetch = vi.mocked(apiFetch);

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: () => Promise.resolve(body),
  } as Response;
}

describe('PersonalityCenter', () => {
  it('shows the latest personality evolution run', async () => {
    mockedApiFetch.mockImplementation(async (input) => {
      if (input === '/api/v1/personality/snapshots') {
        return jsonResponse([
          {
            id: 'snapshot-1',
            user_id: 'user-1',
            trait_values: {
              directness: 0.5,
              warmth: 0.5,
              proactivity: 0.5,
              risk_tolerance: 0.5,
              verbosity: 0.4,
              formality: 0.5,
              humor: 0.5,
            },
            change_reason: 'auto-evolution: 用户多次反馈回答太长',
            source_memory_ids: [],
            is_active: true,
            created_at: '2026-07-03T00:00:00Z',
          },
        ]);
      }
      if (input === '/api/v1/personality/snapshots/active') {
        return jsonResponse({
          id: 'snapshot-1',
          user_id: 'user-1',
          trait_values: {
            directness: 0.5,
            warmth: 0.5,
            proactivity: 0.5,
            risk_tolerance: 0.5,
            verbosity: 0.4,
            formality: 0.5,
            humor: 0.5,
          },
          change_reason: 'auto-evolution: 用户多次反馈回答太长',
          source_memory_ids: [],
          is_active: true,
          created_at: '2026-07-03T00:00:00Z',
        });
      }
      if (input === '/api/v1/evolution/runs?scope=personality&limit=1') {
        return jsonResponse([
          {
            id: 'run-1',
            user_id: 'user-1',
            scope: 'personality',
            status: 'applied',
            evidence_summary: {
              memory_count: 2,
              style_feedback: {
                window_days: 30,
                reply_thumb_up: 0,
                reply_thumb_down: 1,
                too_long: 3,
                too_short: 0,
                too_formal: 0,
                personality_slider_changed: 0,
              },
            },
            proposed_changes: [],
            applied_changes: [],
            skipped_reason: null,
            created_at: '2026-07-03T00:01:00Z',
          },
        ]);
      }
      if (input === '/api/v1/settings/role-prompt') {
        return jsonResponse({ role_prompt: '' });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<PersonalityCenter />);

    expect(await screen.findByText('最近一次自动调整')).toBeInTheDocument();
    expect(screen.getByText('已应用')).toBeInTheDocument();
    expect(screen.getByText('反馈样本: 4')).toBeInTheDocument();
  });

  it('does not block the page when evolution history is unavailable', async () => {
    mockedApiFetch.mockImplementation(async (input) => {
      if (input === '/api/v1/personality/snapshots') return jsonResponse([]);
      if (input === '/api/v1/personality/snapshots/active') return jsonResponse({}, false, 404);
      if (input === '/api/v1/evolution/runs?scope=personality&limit=1') {
        return jsonResponse({}, false, 404);
      }
      if (input === '/api/v1/settings/role-prompt') return jsonResponse({ role_prompt: '' });
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<PersonalityCenter />);

    await waitFor(() => {
      expect(screen.getByText(/人格中心/)).toBeInTheDocument();
    });
    expect(screen.queryByText('最近一次自动调整')).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run frontend test and confirm failure**

Run:

```bash
cd frontend && npm test -- PersonalityCenter.test.tsx
```

Expected: FAIL because `PersonalityCenter` does not fetch or render evolution runs.

- [ ] **Step 3: Add types and status labels**

In `frontend/src/components/personality/PersonalityCenter.tsx`, add these interfaces after `Snapshot`:

```tsx
interface StyleFeedbackSummary {
  window_days?: number;
  reply_thumb_up?: number;
  reply_thumb_down?: number;
  too_long?: number;
  too_short?: number;
  too_formal?: number;
  personality_slider_changed?: number;
}

interface EvolutionRun {
  id: string;
  user_id: string;
  scope: string;
  status: string;
  evidence_summary: {
    memory_count?: number;
    style_feedback?: StyleFeedbackSummary;
  };
  proposed_changes: unknown[];
  applied_changes: unknown[];
  skipped_reason: string | null;
  created_at: string;
}

const EVOLUTION_STATUS_LABELS: Record<string, string> = {
  applied: '已应用',
  skipped: '已跳过',
  failed: '失败',
  review_required: '需确认',
};

function feedbackSampleCount(style?: StyleFeedbackSummary): number {
  if (!style) return 0;
  return (
    (style.reply_thumb_up ?? 0) +
    (style.reply_thumb_down ?? 0) +
    (style.too_long ?? 0) +
    (style.too_short ?? 0) +
    (style.too_formal ?? 0) +
    (style.personality_slider_changed ?? 0)
  );
}
```

- [ ] **Step 4: Fetch latest evolution run**

Inside `PersonalityCenter`, add state:

```tsx
const [latestEvolution, setLatestEvolution] = useState<EvolutionRun | null>(null);
```

In `fetchSnapshots`, add the third request:

```tsx
const [snapRes, activeRes, evolutionRes] = await Promise.all([
  apiFetch('/api/v1/personality/snapshots'),
  apiFetch('/api/v1/personality/snapshots/active'),
  apiFetch('/api/v1/evolution/runs?scope=personality&limit=1'),
]);
```

After active snapshot handling, add:

```tsx
if (evolutionRes.ok) {
  const runs: EvolutionRun[] = await evolutionRes.json();
  setLatestEvolution(runs[0] ?? null);
}
```

Keep 404 handling non-blocking by only reading `evolutionRes` when `ok` is true.

- [ ] **Step 5: Render the latest evolution summary**

Place this section after `RolePromptSection` and before the trait editor:

```tsx
{latestEvolution && (
  <section style={{ marginBottom: 24 }}>
    <h4 style={{ margin: '0 0 12px' }}>最近一次自动调整</h4>
    <div
      style={{
        padding: '10px 12px',
        border: '1px solid var(--color-border, #ddd)',
        borderRadius: 6,
        fontSize: 13,
        background: 'var(--bg-secondary, #fafafa)',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6 }}>
        <strong>{EVOLUTION_STATUS_LABELS[latestEvolution.status] ?? latestEvolution.status}</strong>
        <span style={{ color: 'var(--color-secondary-text, #888)' }}>
          {new Date(latestEvolution.created_at).toLocaleString('zh-CN')}
        </span>
      </div>
      {latestEvolution.skipped_reason && (
        <div style={{ color: 'var(--color-secondary-text, #888)', marginBottom: 4 }}>
          原因: {latestEvolution.skipped_reason}
        </div>
      )}
      <div style={{ color: 'var(--color-secondary-text, #888)' }}>
        记忆证据: {latestEvolution.evidence_summary.memory_count ?? 0}
      </div>
      <div style={{ color: 'var(--color-secondary-text, #888)' }}>
        反馈样本: {feedbackSampleCount(latestEvolution.evidence_summary.style_feedback)}
      </div>
    </div>
  </section>
)}
```

- [ ] **Step 6: Run frontend test**

Run:

```bash
cd frontend && npm test -- PersonalityCenter.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit Task 6**

Run:

```bash
git add frontend/src/components/personality/PersonalityCenter.tsx \
  frontend/src/components/personality/__tests__/PersonalityCenter.test.tsx
git commit -m "feat: show personality evolution history"
```

---

### Task 7: OpenAPI Contract And Full Verification

**Files:**
- Modify: `openapi.json`

- [ ] **Step 1: Regenerate OpenAPI JSON**

Run:

```bash
cargo run -q -p lingshu-server -- --dump-openapi > openapi.json
```

Expected: `openapi.json` contains `/api/v1/evolution/runs`.

- [ ] **Step 2: Run backend tests**

Run:

```bash
cargo test -p lingshu-server evolution::tests models::evolution::tests routes::evolution::tests llm::personality::tests routes::personality::tests routes::tests::openapi_spec_matches_committed_file -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run frontend tests**

Run:

```bash
cd frontend && npm test -- PersonalityCenter.test.tsx
```

Expected: PASS.

- [ ] **Step 4: Run frontend type check**

Run:

```bash
cd frontend && npm run type-check
```

Expected: PASS.

- [ ] **Step 5: Inspect changed files**

Run:

```bash
git status --short
git diff --check
```

Expected:

- `git diff --check` exits 0.
- Changed files are the files listed in this plan plus `openapi.json`.
- Existing unrelated chat component modifications remain separate if they were present before this work.

- [ ] **Step 6: Commit verification and OpenAPI**

Run:

```bash
git add openapi.json
git commit -m "chore: update openapi for evolution runs"
```

---

## Execution Notes

- Do not stage or revert the existing unrelated changes in:
  - `frontend/src/components/chat/ChatInput.tsx`
  - `frontend/src/components/chat/MessageBubble.tsx`
  - `frontend/src/components/chat/__tests__/ChatInput.test.tsx`
  - `frontend/src/components/chat/__tests__/MessageBubble.test.tsx`
- Keep all new automatic evolution writes best-effort. A ledger write failure must not fail a chat response.
- Keep manual personality evolution strict. If the LLM evolution call fails, the API should still return the existing error, after a best-effort `failed` ledger write.
- The new UI section is informational only. It does not approve, reject, or roll back changes.
