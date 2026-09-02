//! Pure policies for the competition-grade evolutionary memory pipeline.
//!
//! Database and vector I/O live in the route/write adapters. Keeping scoring,
//! access control and conflict decisions pure makes benchmark and ablation runs
//! deterministic and independently verifiable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RetrievalWeights {
    pub semantic: f32,
    pub lexical: f32,
    pub recency: f32,
    pub trust: f32,
    pub frequency: f32,
    pub importance: f32,
}

impl Default for RetrievalWeights {
    fn default() -> Self {
        Self {
            semantic: 0.34,
            lexical: 0.18,
            recency: 0.16,
            trust: 0.13,
            frequency: 0.08,
            importance: 0.11,
        }
    }
}

impl RetrievalWeights {
    pub fn normalized(mut self) -> Self {
        self.semantic = self.semantic.max(0.0);
        self.lexical = self.lexical.max(0.0);
        self.recency = self.recency.max(0.0);
        self.trust = self.trust.max(0.0);
        self.frequency = self.frequency.max(0.0);
        self.importance = self.importance.max(0.0);
        let total = self.semantic
            + self.lexical
            + self.recency
            + self.trust
            + self.frequency
            + self.importance;
        if total <= f32::EPSILON {
            return Self::default();
        }
        self.semantic /= total;
        self.lexical /= total;
        self.recency /= total;
        self.trust /= total;
        self.frequency /= total;
        self.importance /= total;
        self
    }

    /// Temporal questions intentionally favor current facts; preference and
    /// experience questions keep more semantic weight.
    pub fn for_query(query: &str) -> Self {
        let mut weights = Self::default();
        let temporal = ["现在", "最新", "目前", "今天", "最近", "current", "latest"];
        if temporal.iter().any(|needle| query.contains(needle)) {
            weights.recency += 0.30;
            weights.trust += 0.08;
            weights.semantic -= 0.14;
            weights.lexical -= 0.08;
            weights.importance -= 0.10;
            weights.frequency -= 0.06;
        }
        weights.normalized()
    }
}

#[derive(Debug, Clone)]
pub struct RankableMemory {
    pub id: Uuid,
    pub content: String,
    pub importance: f32,
    pub access_count: i32,
    pub occurred_at: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub source_trust: f32,
    pub conflict_status: String,
    pub retention_status: String,
    pub semantic_score: f32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScoreBreakdown {
    pub semantic: f32,
    pub lexical: f32,
    pub recency: f32,
    pub trust: f32,
    pub frequency: f32,
    pub importance: f32,
    pub total: f32,
}

pub fn is_retrieval_eligible(memory: &RankableMemory, now: DateTime<Utc>) -> bool {
    memory.conflict_status == "active"
        && memory.retention_status == "active"
        && memory.valid_to.map(|until| until >= now).unwrap_or(true)
}

pub fn score_memory(
    query: &str,
    memory: &RankableMemory,
    weights: &RetrievalWeights,
    now: DateTime<Utc>,
) -> ScoreBreakdown {
    let weights = weights.clone().normalized();
    let age_days = now
        .signed_duration_since(memory.occurred_at)
        .num_seconds()
        .max(0) as f32
        / 86_400.0;
    let recency = 2.0_f32.powf(-age_days / 30.0).clamp(0.0, 1.0);
    let frequency = ((memory.access_count.max(0) as f32 + 1.0).ln() / 5.0_f32.ln()).clamp(0.0, 1.0);
    let lexical = lexical_similarity(query, &memory.content);
    let semantic = memory.semantic_score.clamp(0.0, 1.0);
    let trust = memory.source_trust.clamp(0.0, 1.0);
    let importance = memory.importance.clamp(0.0, 1.0);
    let total = semantic * weights.semantic
        + lexical * weights.lexical
        + recency * weights.recency
        + trust * weights.trust
        + frequency * weights.frequency
        + importance * weights.importance;

    ScoreBreakdown {
        semantic,
        lexical,
        recency,
        trust,
        frequency,
        importance,
        total,
    }
}

pub fn lexical_similarity(left: &str, right: &str) -> f32 {
    let left = character_terms(left);
    let right = character_terms(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count() as f32;
    let union = left.union(&right).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn character_terms(text: &str) -> HashSet<String> {
    let normalized = text.to_lowercase();
    let chars: Vec<char> = normalized
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect();
    let mut terms: HashSet<String> = chars.iter().map(char::to_string).collect();
    for pair in chars.windows(2) {
        terms.insert(pair.iter().collect());
    }
    terms
}

pub fn can_read_memory(
    requesting_agent: &str,
    owner_agent: &str,
    scope: &str,
    has_explicit_grant: bool,
) -> bool {
    requesting_agent == owner_agent
        || scope == "global"
        || (scope == "shared" && has_explicit_grant)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    ChallengerWins,
    IncumbentWins,
    Disputed,
}

#[derive(Debug, Clone)]
pub struct ConflictEvidence {
    pub source_trust: f32,
    pub access_count: i32,
    pub importance: f32,
    pub occurred_at: DateTime<Utc>,
}

pub fn conflict_evidence_score(evidence: &ConflictEvidence, newest: DateTime<Utc>) -> f32 {
    let age_days = newest
        .signed_duration_since(evidence.occurred_at)
        .num_seconds()
        .max(0) as f32
        / 86_400.0;
    let recency = 2.0_f32.powf(-age_days / 30.0).clamp(0.0, 1.0);
    let frequency =
        ((evidence.access_count.max(0) as f32 + 1.0).ln() / 5.0_f32.ln()).clamp(0.0, 1.0);
    evidence.source_trust.clamp(0.0, 1.0) * 0.55
        + recency * 0.25
        + frequency * 0.10
        + evidence.importance.clamp(0.0, 1.0) * 0.10
}

pub fn resolve_conflict(
    incumbent: &ConflictEvidence,
    challenger: &ConflictEvidence,
) -> (ConflictResolution, f32) {
    let newest = incumbent.occurred_at.max(challenger.occurred_at);
    let incumbent_score = conflict_evidence_score(incumbent, newest);
    let challenger_score = conflict_evidence_score(challenger, newest);
    let delta = challenger_score - incumbent_score;
    let resolution = if delta > 0.08 {
        ConflictResolution::ChallengerWins
    } else if delta < -0.08 {
        ConflictResolution::IncumbentWins
    } else {
        ConflictResolution::Disputed
    };
    (resolution, delta)
}

pub fn estimate_tokens(contents: impl IntoIterator<Item = String>) -> i32 {
    let chars: usize = contents
        .into_iter()
        .map(|content| content.chars().count())
        .sum();
    ((chars as f32) / 1.8).ceil() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn temporal_query_increases_recency_weight() {
        let regular = RetrievalWeights::for_query("用户喜欢什么编辑器");
        let temporal = RetrievalWeights::for_query("用户现在喜欢什么编辑器");
        assert!(temporal.recency > regular.recency);
    }

    #[test]
    fn private_memory_is_isolated_but_global_is_visible() {
        assert!(!can_read_memory("planner", "writer", "private", false));
        assert!(can_read_memory("planner", "writer", "global", false));
        assert!(can_read_memory("planner", "writer", "shared", true));
    }

    #[test]
    fn expired_and_superseded_memories_are_filtered() {
        let now = Utc::now();
        let base = RankableMemory {
            id: Uuid::new_v4(),
            content: "旧地址".into(),
            importance: 0.8,
            access_count: 1,
            occurred_at: now,
            valid_to: Some(now - Duration::seconds(1)),
            source_trust: 0.9,
            conflict_status: "active".into(),
            retention_status: "active".into(),
            semantic_score: 0.9,
        };
        assert!(!is_retrieval_eligible(&base, now));
    }

    #[test]
    fn trusted_recent_challenger_supersedes_old_fact() {
        let now = Utc::now();
        let incumbent = ConflictEvidence {
            source_trust: 0.6,
            access_count: 1,
            importance: 0.7,
            occurred_at: now - Duration::days(80),
        };
        let challenger = ConflictEvidence {
            source_trust: 0.95,
            access_count: 2,
            importance: 0.8,
            occurred_at: now,
        };
        assert_eq!(
            resolve_conflict(&incumbent, &challenger).0,
            ConflictResolution::ChallengerWins
        );
    }

    #[test]
    fn lexical_similarity_supports_chinese_without_spaces() {
        assert!(lexical_similarity("当前项目技术栈", "项目使用 Rust 技术栈") > 0.15);
    }
}
