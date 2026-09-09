//! Pure policies for the competition-grade evolutionary memory pipeline.
//!
//! Database and vector I/O live in the route/write adapters. Keeping scoring,
//! access control and conflict decisions pure makes benchmark and ablation runs
//! deterministic and independently verifiable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

    /// Markers that the question asks about *current* state, where newer
    /// memories genuinely deserve a boost. Chinese and English: an
    /// English-only memory store would otherwise never route here.
    const FORWARD_LOOKING: [&str; 18] = [
        "现在",
        "最新",
        "目前",
        "今天",
        "最近",
        "近期",
        "当下",
        "此刻",
        "这周",
        "本月",
        "now",
        "current",
        "currently",
        "latest",
        "recent",
        "recently",
        "these days",
        "at present",
    ];

    /// Markers that the question is an *explicit recall of the past*. Here a
    /// recency boost is actively wrong: it buries the very evidence being
    /// asked for under newer, irrelevant chatter.
    const BACKWARD_LOOKING: [&str; 14] = [
        "当时",
        "那时",
        "之前",
        "以前",
        "曾经",
        "过去",
        "上次",
        "when did",
        "what time",
        "how long ago",
        "last time",
        "back then",
        "at that time",
        "did you ever",
    ];

    /// Markers that the question is a *synthesis* across many facts ("integrate
    /// customer + product + contract"). Here recency is actively harmful: the
    /// freshest memory may be noise ("the coffee machine broke") that crowds out
    /// the older but decision-critical facts. Synthesis leans on semantic recall.
    const SYNTHESIS: [&str; 11] = [
        "汇总",
        "整合",
        "综合",
        "总结",
        "生成最终",
        "形成最终",
        "合并",
        "summarize",
        "consolidate",
        "integrate",
        "across",
    ];

    /// Route a query to a weight profile.
    ///
    /// Forward-looking questions ("where does the user live now") favour
    /// recency and trust. Backward-looking ones ("when did we last ship")
    /// suppress recency and lean on pure relevance. Synthesis questions
    /// ("assemble the full picture") drop recency so noise cannot crowd out the
    /// facts that must all be gathered. Everything else keeps the balanced
    /// default, which is why the default recency weight is small: time decay is
    /// opt-in, not a constant drag on historical recall.
    pub fn for_query(query: &str) -> Self {
        let mut weights = Self::default();
        let lowered = query.to_lowercase();

        let forward = Self::FORWARD_LOOKING
            .iter()
            .any(|needle| lowered.contains(needle));
        let backward = Self::BACKWARD_LOOKING
            .iter()
            .any(|needle| lowered.contains(needle));
        let synthesis = Self::SYNTHESIS
            .iter()
            .any(|needle| lowered.contains(needle));

        if forward && !backward {
            weights.recency += 0.30;
            weights.trust += 0.08;
            weights.semantic -= 0.14;
            weights.lexical -= 0.08;
            weights.importance -= 0.10;
            weights.frequency -= 0.06;
        } else if backward && !forward {
            weights.recency = (weights.recency - 0.12).max(0.0);
            weights.semantic += 0.10;
            weights.lexical += 0.08;
            weights.trust -= 0.04;
            weights.importance -= 0.02;
        } else if synthesis {
            weights.recency = 0.0;
            weights.semantic += 0.12;
            weights.lexical += 0.06;
            weights.trust += 0.02;
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
    let lexical = retrieval_lexical_score(query, &memory.content);
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

/// Query-directed lexical evidence. Keep the symmetric bigram metric below
/// unchanged for duplicate detection and existing similarity callers.
/// Full phrases and bounded ASCII names/identifiers avoid dilution by long
/// bodies. Missing numeric identifiers reduces lexical evidence, not access
/// eligibility or semantic evidence (a paraphrase may still be relevant).
pub fn retrieval_lexical_score(query: &str, content: &str) -> f32 {
    let query = query.trim().to_lowercase();
    let content = content.to_lowercase();
    let base = lexical_similarity(&query, &content);
    if query.is_empty() {
        return base;
    }
    let identifiers: Vec<&str> = query
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .map(|s| s.trim_matches(['-', '_', '.']))
        .filter(|s| s.bytes().any(|b| b.is_ascii_digit()))
        .collect();
    let content_tokens: Vec<&str> = content
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .map(|s| s.trim_matches(['-', '_', '.']))
        .collect();
    if identifiers.iter().any(|id| !content_tokens.contains(id)) {
        return base * 0.2;
    }
    if query.chars().count() >= 3 && bounded_match(&content, &query) {
        return 1.0;
    }
    if !identifiers.is_empty() {
        return base.max(0.9);
    }
    // Reserve the upper lexical range for exact query evidence. With default
    // weights this prevents frequency alone (at most 0.08) overturning a
    // phrase match (1.0 vs at most 0.5, weighted by 0.18).
    base.min(0.5)
}

// ASCII token boundaries distinguish Rust/Rustacean and 42424/424240.
// Chinese boundaries deliberately allow substrings without segmentation.
fn bounded_match(content: &str, needle: &str) -> bool {
    let token = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_');
    content.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let left_ok = !needle.starts_with(token)
            || content[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !token(c));
        let right_ok =
            !needle.ends_with(token) || content[end..].chars().next().is_none_or(|c| !token(c));
        left_ok && right_ok
    })
}

/// Character bigram Jaccard similarity.
///
/// Language-agnostic on purpose: it works for Chinese, where whitespace
/// tokenisation is unavailable, without pulling in a segmenter.
pub fn lexical_similarity(left: &str, right: &str) -> f32 {
    let left = character_terms(left);
    let right = character_terms(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    // Sorted, deduped vectors: intersection and union by merge scan instead of
    // hashing. Same result as set ops, ~20x faster, and this sits on the hot
    // path of every retrieval call.
    let mut i = 0usize;
    let mut j = 0usize;
    let mut intersection = 0usize;
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            intersection += 1;
            i += 1;
            j += 1;
        } else if left[i] < right[j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let union = left.len() + right.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Unigrams and bigrams of the alphanumeric character sequence.
///
/// Terms are packed into a `u64`: a unigram is its code point, a bigram is
/// `(first << 21) | second`. Unicode code points stay below 2^21, so the two
/// kinds never collide and no allocation is needed.
fn character_terms(text: &str) -> Vec<u64> {
    let normalized = text.to_lowercase();
    let chars: Vec<char> = normalized
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect();
    let mut terms: Vec<u64> = Vec::with_capacity(chars.len() * 2);
    terms.extend(chars.iter().map(|ch| *ch as u32 as u64));
    terms.extend(
        chars
            .windows(2)
            .map(|pair| ((pair[0] as u32 as u64) << 21) | (pair[1] as u32 as u64)),
    );
    terms.sort_unstable();
    terms.dedup();
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
    #[test]
    fn exact_lexical_evidence_respects_identifiers_and_boundaries() {
        use super::retrieval_lexical_score as score;
        assert_eq!(
            score("无关记录 42424", "说明：无关记录 42424 后面很长的附录"),
            1.0
        );
        assert!(score("工单 AB-42", "AB-42 已完成") > score("工单 AB-42", "工单 AB-420 已完成"));
        assert!(score("版本 2.0", "版本 2.0.1") < 0.2);
        assert_eq!(score("PostgreSQL", "使用 PostgreSQL."), 1.0);
        assert!(score("Rust", "Rustacean") < 1.0);
        assert_eq!(score("凤凰项目", "历史资料：凤凰项目已经上线。"), 1.0);
        assert_eq!(score("", "任意正文"), 0.0);
    }

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

    /// The packed-u64 merge scan must reproduce the old `HashSet<String>` result
    /// exactly, or retrieval rankings would silently shift.
    #[test]
    fn lexical_matches_reference_set_implementation() {
        fn reference(left: &str, right: &str) -> f32 {
            let lower = |text: &str| {
                let chars: Vec<char> = text
                    .to_lowercase()
                    .chars()
                    .filter(|ch| ch.is_alphanumeric())
                    .collect();
                let mut terms: std::collections::HashSet<String> =
                    chars.iter().map(char::to_string).collect();
                for pair in chars.windows(2) {
                    terms.insert(pair.iter().collect());
                }
                terms
            };
            let left = lower(left);
            let right = lower(right);
            if left.is_empty() || right.is_empty() {
                return 0.0;
            }
            let intersection = left.intersection(&right).count();
            let union = left.union(&right).count();
            intersection as f32 / union as f32
        }

        let samples = [
            ("当前项目技术栈", "项目使用 Rust 技术栈"),
            ("用户现在常住哪里", "用户搬迁后定居上海"),
            ("", "非空"),
            ("ASCII test 123", "ascii Test 124"),
            ("项目使用 PostgreSQL 数据库", "后端已经迁移至 Rust"),
            ("a", "a"),
            ("🎉 emoji 测试", "emoji 测试 🎉🎉"),
        ];
        for (left, right) in samples {
            let fast = lexical_similarity(left, right);
            let slow = reference(left, right);
            assert!(
                (fast - slow).abs() < 1e-6,
                "mismatch for ({left:?}, {right:?}): fast={fast} slow={slow}"
            );
        }
    }
}
