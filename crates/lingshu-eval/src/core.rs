//! Shared types and the retrieval engine used by every experiment.
//!
//! The scoring policy itself is *not* reimplemented here. It is linked straight
//! out of the server crate via `#[path]`, so the numbers this harness prints are
//! produced by exactly the same code that answers real queries in production.
//! Changing the policy changes both, and there is no second copy to drift.
//!
//! Several struct fields are retained for auditability (provenance, owner, scope)
//! even when the current experiments do not read them; silence the dead-code
//! lint rather than drop information that a future experiment may need.
#![allow(dead_code)]

#[path = "../../lingshu-server/src/llm/evolutionary_memory.rs"]
pub mod evolutionary_memory;

use chrono::{DateTime, Utc};
use serde::Serialize;

pub use evolutionary_memory::{
    estimate_tokens, is_retrieval_eligible, lexical_similarity, score_memory, RankableMemory,
    RetrievalWeights,
};

/// One storable unit, the common shape every corpus is normalised into.
#[derive(Clone, Debug)]
pub struct EvalMemory {
    pub id: String,
    pub content: String,
    pub fact_key: String,
    /// Row in the shared embedding matrix; `None` when the corpus has no vectors.
    pub embedding_row: Option<usize>,
    pub importance: f32,
    pub trust: f32,
    pub access_count: i32,
    pub occurred_at: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    /// `active` | `superseded` | `stale` | `duplicate`
    pub state: String,
    /// `private` | `shared` | `global`
    pub scope: String,
    pub owner_agent: String,
    /// Ground truth for governance sets: should this survive retrieval?
    pub should_retain: Option<bool>,
}

impl EvalMemory {
    pub fn eligible_state(&self) -> bool {
        self.state == "active"
    }
}

/// A scored query with one or more gold memory ids.
#[derive(Clone, Debug)]
pub struct EvalQuery {
    pub query: String,
    pub embedding_row: Option<usize>,
    pub expected: Vec<String>,
    /// Optional grouping (LoCoMo category, task id, agent id ...).
    pub group: String,
    /// Reference answer text, when the corpus provides one. Used for the
    /// answer-support metric, not for ranking.
    pub answer: String,
}

/// A self-contained retrieval universe: one user / one dialogue store.
#[derive(Clone, Debug)]
pub struct EvalCase {
    pub case_id: String,
    pub memories: Vec<EvalMemory>,
    pub queries: Vec<EvalQuery>,
    /// The instant queries are asked. Defaults to the newest memory timestamp.
    pub now: DateTime<Utc>,
}

/// Row-major float32 embedding matrix shared by all corpora.
#[derive(Debug)]
pub struct Embeddings {
    pub data: Vec<f32>,
    pub dim: usize,
}

impl Embeddings {
    pub fn row(&self, row: usize) -> &[f32] {
        let start = row * self.dim;
        &self.data[start..start + self.dim]
    }

    /// Cosine similarity; rows are L2-normalised at encode time, so this is a dot product.
    pub fn cosine(&self, left: usize, right: usize) -> f32 {
        let a = self.row(left);
        let b = self.row(right);
        let mut sum = 0.0f32;
        for (x, y) in a.iter().zip(b.iter()) {
            sum += x * y;
        }
        sum.clamp(0.0, 1.0)
    }
}

/// Which retrieval policy to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyKind {
    /// Static RAG: embed everything once, rank by cosine, no decay, no filtering.
    Static,
    /// LingShu: query-adaptive multi-signal ranking with eligibility filtering.
    Adaptive,
}

/// A complete retrieval configuration. Used for baselines, ablations and the
/// full model alike so every arm differs only by the fields it names.
#[derive(Clone, Debug)]
pub struct Strategy {
    pub label: String,
    pub kind: StrategyKind,
    pub weights: RetrievalWeights,
    pub filter_ineligible: bool,
    pub context_k: usize,
    /// Overrides the query-adaptive route when set (ablation "no routing").
    pub lock_weights: bool,
    /// Whether the final sort applies the importance-descending tiebreak.
    /// Production keeps this true; a clean `-importance` ablation sets it false
    /// so the signal is *fully* removed (both the scoring channel and the
    /// tiebreak), otherwise the tiebreak silently keeps importance alive.
    pub importance_tiebreak: bool,
}

impl Strategy {
    pub fn static_rag(context_k: usize) -> Self {
        Self {
            label: "Static RAG".into(),
            kind: StrategyKind::Static,
            weights: RetrievalWeights::default(),
            filter_ineligible: false,
            context_k,
            lock_weights: true,
            importance_tiebreak: true,
        }
    }

    pub fn full() -> Self {
        Self {
            label: "LingShu Full".into(),
            kind: StrategyKind::Adaptive,
            weights: RetrievalWeights::default(),
            filter_ineligible: true,
            context_k: 5,
            lock_weights: false,
            importance_tiebreak: true,
        }
    }
}

/// One retrieved hit, kept for auditing and token accounting.
#[derive(Clone, Debug)]
pub struct Hit {
    pub id: String,
    pub score: f32,
}

/// Outcome of a single query.
#[derive(Clone, Debug)]
pub struct QueryOutcome {
    pub ranked: Vec<Hit>,
    pub expected: Vec<String>,
    pub group: String,
    pub context_tokens: i32,
    pub filtered_count: usize,
    /// Text of the assembled context, for answer-support measurement.
    pub top_contents: Vec<String>,
    pub answer: String,
}

pub struct Retriever<'a> {
    pub memories: &'a [EvalMemory],
    pub embeddings: &'a Embeddings,
    pub now: DateTime<Utc>,
}

impl<'a> Retriever<'a> {
    /// Semantic score for one memory. Falls back to lexical overlap when the
    /// corpus carries no vectors -- the documented degradation path.
    fn semantic(&self, query: &EvalQuery, memory: &EvalMemory) -> f32 {
        match (query.embedding_row, memory.embedding_row) {
            (Some(q), Some(m)) => self.embeddings.cosine(q, m),
            _ => lexical_similarity(&query.query, &memory.content),
        }
    }

    pub fn run(&self, query: &EvalQuery, strategy: &Strategy) -> QueryOutcome {
        let mut filtered_count = 0usize;
        let mut scored: Vec<(&EvalMemory, f32)> = Vec::with_capacity(self.memories.len());
        // Deterministic pseudo-id: `RankableMemory::id` is only carried for
        // tracing, so deriving it from the slot keeps the run reproducible and
        // avoids 10M+ random UUID allocations.
        let synthetic_id = uuid::Uuid::from_u128(0x5EE5_0000_0000_0000);

        for memory in self.memories {
            let semantic = self.semantic(query, memory);
            let rankable = RankableMemory {
                id: synthetic_id,
                content: memory.content.clone(),
                importance: memory.importance,
                access_count: memory.access_count,
                occurred_at: memory.occurred_at,
                valid_to: memory.valid_to,
                source_trust: memory.trust,
                conflict_status: if memory.state == "superseded" || memory.state == "duplicate" {
                    memory.state.clone()
                } else {
                    "active".into()
                },
                retention_status: if memory.state == "stale" {
                    "stale".into()
                } else if memory.state == "superseded" || memory.state == "duplicate" {
                    "filtered".into()
                } else {
                    "active".into()
                },
                semantic_score: semantic,
            };

            if strategy.filter_ineligible && !is_retrieval_eligible(&rankable, self.now) {
                filtered_count += 1;
                continue;
            }

            let score = match strategy.kind {
                StrategyKind::Static => {
                    // Static RAG ranks on embedding similarity alone. This is the
                    // strongest honest baseline: it uses the same encoder, so any
                    // gain comes from the memory policy, not from a better vector.
                    semantic
                }
                StrategyKind::Adaptive => {
                    // Mirrors the production route: a query-adaptive prior
                    // multiplied by the configured (possibly ablated) weights,
                    // then renormalised. A channel ablated to zero stays zero.
                    let configured = strategy.weights.clone().normalized();
                    let routed = if strategy.lock_weights {
                        RetrievalWeights::default()
                    } else {
                        RetrievalWeights::for_query(&query.query)
                    };
                    let merged = RetrievalWeights {
                        semantic: if strategy.weights.semantic == 0.0 {
                            0.0
                        } else {
                            routed.semantic * configured.semantic
                        },
                        lexical: if strategy.weights.lexical == 0.0 {
                            0.0
                        } else {
                            routed.lexical * configured.lexical
                        },
                        recency: if strategy.weights.recency == 0.0 {
                            0.0
                        } else {
                            routed.recency * configured.recency
                        },
                        trust: if strategy.weights.trust == 0.0 {
                            0.0
                        } else {
                            routed.trust * configured.trust
                        },
                        frequency: if strategy.weights.frequency == 0.0 {
                            0.0
                        } else {
                            routed.frequency * configured.frequency
                        },
                        importance: if strategy.weights.importance == 0.0 {
                            0.0
                        } else {
                            routed.importance * configured.importance
                        },
                    }
                    .normalized();
                    score_memory(&query.query, &rankable, &merged, self.now).total
                }
            };
            scored.push((memory, score));
        }

        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| {
                    if strategy.importance_tiebreak {
                        right.0.importance.total_cmp(&left.0.importance)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .then_with(|| left.0.id.cmp(&right.0.id))
        });

        let ranked: Vec<Hit> = scored
            .iter()
            .map(|(memory, score)| Hit {
                id: memory.id.clone(),
                score: *score,
            })
            .collect();

        let top_contents: Vec<String> = scored
            .iter()
            .take(strategy.context_k)
            .map(|(memory, _)| memory.content.clone())
            .collect();
        let context_tokens = estimate_tokens(top_contents.iter().cloned());

        QueryOutcome {
            ranked,
            expected: query.expected.clone(),
            group: query.group.clone(),
            context_tokens,
            filtered_count,
            top_contents,
            answer: query.answer.clone(),
        }
    }
}
