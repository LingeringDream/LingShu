use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
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
    insert_evolution_run(
        db,
        user_id,
        scope,
        status,
        evidence_summary,
        proposed_changes,
        applied_changes,
        skipped_reason,
    )
    .await
}

pub async fn record_evolution_run_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    scope: EvolutionScope,
    status: EvolutionRunStatus,
    evidence_summary: serde_json::Value,
    proposed_changes: serde_json::Value,
    applied_changes: serde_json::Value,
    skipped_reason: Option<&str>,
) -> anyhow::Result<EvolutionRun> {
    insert_evolution_run(
        &mut **tx,
        user_id,
        scope,
        status,
        evidence_summary,
        proposed_changes,
        applied_changes,
        skipped_reason,
    )
    .await
}

async fn insert_evolution_run<'e, E>(
    executor: E,
    user_id: Uuid,
    scope: EvolutionScope,
    status: EvolutionRunStatus,
    evidence_summary: serde_json::Value,
    proposed_changes: serde_json::Value,
    applied_changes: serde_json::Value,
    skipped_reason: Option<&str>,
) -> anyhow::Result<EvolutionRun>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
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
    .fetch_one(executor)
    .await?;

    Ok(run)
}

pub async fn record_personality_failure(
    db: &PgPool,
    user_id: Uuid,
    error: &anyhow::Error,
) -> anyhow::Result<EvolutionRun> {
    let evidence = serde_json::json!({
        "error": error.to_string(),
    });

    record_evolution_run(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::personality::PersonalityTraits;

    #[test]
    fn style_feedback_detects_any_signal() {
        let empty = StyleFeedbackSummary {
            window_days: 30,
            ..Default::default()
        };
        assert!(!empty.has_feedback());
        assert_eq!(empty.sample_count(), 0);

        let with_tag = StyleFeedbackSummary {
            window_days: 30,
            too_long: 2,
            ..Default::default()
        };
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
        let traits = PersonalityTraits {
            verbosity: 0.4,
            ..Default::default()
        };
        let json = personality_changes_json(&traits, 0.8, 0.1, "用户多次反馈回答太长");
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["type"], "personality_traits");
        assert!((json[0]["trait_values"]["verbosity"].as_f64().unwrap() - 0.4).abs() < 1e-6);
        assert!((json[0]["confidence"].as_f64().unwrap() - 0.8).abs() < 1e-6);
    }
}
