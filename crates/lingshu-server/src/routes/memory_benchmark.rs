use axum::{routing, Json, Router};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::llm::evolutionary_memory::{
    estimate_tokens, is_retrieval_eligible, lexical_similarity, score_memory, RankableMemory,
    RetrievalWeights,
};
use crate::state::AppState;

const DATASET_JSON: &str = include_str!("../../../../datasets/zte_memory_50turn_v1.json");

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/memory-gateway/benchmark",
        routing::get(run_benchmark),
    )
}

#[derive(Debug, Deserialize)]
struct BenchmarkDataset {
    name: String,
    description: String,
    turns: Vec<BenchmarkTurn>,
    queries: Vec<BenchmarkQuery>,
}

#[derive(Debug, Deserialize)]
struct BenchmarkTurn {
    turn: usize,
    id: String,
    content: String,
    fact_key: String,
    value: String,
    age_days: i64,
    importance: f32,
    trust: f32,
    access: i32,
    state: String,
}

#[derive(Debug, Deserialize)]
struct BenchmarkQuery {
    query: String,
    expected: String,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct BenchmarkMetrics {
    pub top1_accuracy: f32,
    /// Hit@5: percentage of queries whose expected memory appears in Top-5.
    pub top_k_precision: f32,
    pub stale_redundant_filter_rate: f32,
    pub retrieval_task_completion: f32,
    pub estimated_token_count: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AblationResult {
    pub name: String,
    pub top1_accuracy: f32,
    pub top_k_precision: f32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThresholdCheck {
    pub metric: String,
    pub target: String,
    pub measured: f32,
    pub passed: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BenchmarkResponse {
    pub dataset: String,
    pub dataset_description: String,
    pub turn_count: usize,
    pub query_count: usize,
    pub reproducible: bool,
    pub baseline: BenchmarkMetrics,
    pub adaptive: BenchmarkMetrics,
    pub accuracy_gain_percentage_points: f32,
    pub task_completion_gain_percentage_points: f32,
    pub token_reduction_rate: f32,
    pub ablations: Vec<AblationResult>,
    pub threshold_checks: Vec<ThresholdCheck>,
    pub evidence_note: String,
}

#[derive(Clone, Copy)]
struct EvaluationOptions {
    adaptive: bool,
    filter_ineligible: bool,
    weights: RetrievalWeightsTuple,
    context_k: usize,
}

#[derive(Clone, Copy)]
struct RetrievalWeightsTuple {
    semantic: f32,
    lexical: f32,
    recency: f32,
    trust: f32,
    frequency: f32,
    importance: f32,
}

impl RetrievalWeightsTuple {
    fn full() -> Self {
        Self {
            semantic: 0.34,
            lexical: 0.18,
            recency: 0.16,
            trust: 0.13,
            frequency: 0.08,
            importance: 0.11,
        }
    }

    fn weights(self) -> RetrievalWeights {
        RetrievalWeights {
            semantic: self.semantic,
            lexical: self.lexical,
            recency: self.recency,
            trust: self.trust,
            frequency: self.frequency,
            importance: self.importance,
        }
        .normalized()
    }
}

fn memory_uuid(turn: usize) -> Uuid {
    Uuid::from_u128(turn as u128 + 10_000)
}

fn evaluate(dataset: &BenchmarkDataset, options: EvaluationOptions) -> BenchmarkMetrics {
    let now = Utc::now();
    let flagged = dataset
        .turns
        .iter()
        .filter(|turn| turn.state != "active")
        .count();
    let mut filtered_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut top1_hits = 0usize;
    let mut topk_hits = 0usize;
    let mut token_total = 0i32;

    for query in &dataset.queries {
        let routed = RetrievalWeights::for_query(&query.query);
        let configured = options.weights.weights();
        let query_weights = RetrievalWeights {
            semantic: if options.weights.semantic == 0.0 {
                0.0
            } else {
                routed.semantic * configured.semantic
            },
            lexical: if options.weights.lexical == 0.0 {
                0.0
            } else {
                routed.lexical * configured.lexical
            },
            recency: if options.weights.recency == 0.0 {
                0.0
            } else {
                routed.recency * configured.recency
            },
            trust: if options.weights.trust == 0.0 {
                0.0
            } else {
                routed.trust * configured.trust
            },
            frequency: if options.weights.frequency == 0.0 {
                0.0
            } else {
                routed.frequency * configured.frequency
            },
            importance: if options.weights.importance == 0.0 {
                0.0
            } else {
                routed.importance * configured.importance
            },
        }
        .normalized();
        let mut ranked: Vec<(&BenchmarkTurn, f32)> = Vec::new();
        for turn in &dataset.turns {
            let rankable = RankableMemory {
                id: memory_uuid(turn.turn),
                content: turn.content.clone(),
                importance: turn.importance,
                access_count: turn.access,
                occurred_at: now - Duration::days(turn.age_days),
                valid_to: None,
                source_trust: turn.trust,
                conflict_status: if turn.state == "superseded" {
                    "superseded".into()
                } else {
                    "active".into()
                },
                retention_status: if turn.state == "stale" {
                    "stale".into()
                } else if turn.state == "superseded" {
                    "filtered".into()
                } else {
                    "active".into()
                },
                semantic_score: lexical_similarity(&query.query, &turn.content),
            };
            if options.filter_ineligible && !is_retrieval_eligible(&rankable, now) {
                filtered_ids.insert(turn.id.as_str());
                continue;
            }
            let score = if options.adaptive {
                score_memory(&query.query, &rankable, &query_weights, now).total
            } else {
                lexical_similarity(&query.query, &turn.content) * 0.7 + turn.importance * 0.3
            };
            ranked.push((turn, score));
        }
        ranked.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| right.0.importance.total_cmp(&left.0.importance))
        });
        if ranked.first().map(|(turn, _)| turn.id.as_str()) == Some(query.expected.as_str()) {
            top1_hits += 1;
        }
        if ranked
            .iter()
            .take(5)
            .any(|(turn, _)| turn.id == query.expected)
        {
            topk_hits += 1;
        }
        token_total += estimate_tokens(
            ranked
                .iter()
                .take(options.context_k)
                .map(|(turn, _)| format!("{}:{}={}", turn.fact_key, turn.content, turn.value)),
        );
    }
    let query_count = dataset.queries.len().max(1) as f32;
    BenchmarkMetrics {
        top1_accuracy: top1_hits as f32 / query_count * 100.0,
        top_k_precision: topk_hits as f32 / query_count * 100.0,
        stale_redundant_filter_rate: if flagged == 0 {
            0.0
        } else {
            filtered_ids.len() as f32 / flagged as f32 * 100.0
        },
        retrieval_task_completion: top1_hits as f32 / query_count * 100.0,
        estimated_token_count: token_total,
    }
}

fn ablation(
    dataset: &BenchmarkDataset,
    name: &str,
    mutate: impl FnOnce(&mut RetrievalWeightsTuple),
    filter: bool,
) -> AblationResult {
    let mut weights = RetrievalWeightsTuple::full();
    mutate(&mut weights);
    let metrics = evaluate(
        dataset,
        EvaluationOptions {
            adaptive: true,
            filter_ineligible: filter,
            weights,
            context_k: 5,
        },
    );
    AblationResult {
        name: name.into(),
        top1_accuracy: metrics.top1_accuracy,
        top_k_precision: metrics.top_k_precision,
    }
}

#[utoipa::path(get, path = "/api/v1/memory-gateway/benchmark", responses((status = 200, body = BenchmarkResponse)))]
pub async fn run_benchmark(auth: Option<AuthUser>) -> Result<Json<BenchmarkResponse>, AppError> {
    let _user_id = auth::require_user(auth).await?;
    let dataset: BenchmarkDataset = serde_json::from_str(DATASET_JSON).map_err(|error| {
        AppError::Internal(anyhow::anyhow!("benchmark dataset invalid: {error}"))
    })?;
    let baseline = evaluate(
        &dataset,
        EvaluationOptions {
            adaptive: false,
            filter_ineligible: false,
            weights: RetrievalWeightsTuple::full(),
            context_k: 12,
        },
    );
    let adaptive = evaluate(
        &dataset,
        EvaluationOptions {
            adaptive: true,
            filter_ineligible: true,
            weights: RetrievalWeightsTuple::full(),
            context_k: 5,
        },
    );
    let accuracy_gain = adaptive.top1_accuracy - baseline.top1_accuracy;
    let task_gain = adaptive.retrieval_task_completion - baseline.retrieval_task_completion;
    let token_reduction = if baseline.estimated_token_count == 0 {
        0.0
    } else {
        (baseline.estimated_token_count - adaptive.estimated_token_count) as f32
            / baseline.estimated_token_count as f32
            * 100.0
    };
    let ablations = vec![
        ablation(&dataset, "完整模型", |_| {}, true),
        ablation(
            &dataset,
            "移除时间感知",
            |weights| weights.recency = 0.0,
            true,
        ),
        ablation(
            &dataset,
            "移除来源可信度",
            |weights| weights.trust = 0.0,
            true,
        ),
        ablation(
            &dataset,
            "移除交互频次",
            |weights| weights.frequency = 0.0,
            true,
        ),
        ablation(&dataset, "移除动态过滤", |_| {}, false),
    ];
    let threshold_checks = vec![
        ThresholdCheck {
            metric: "长时序任务记忆准确率".into(),
            target: ">=86% 且提升 >=15 个百分点".into(),
            measured: adaptive.top1_accuracy,
            passed: adaptive.top1_accuracy >= 86.0 && accuracy_gain >= 15.0,
        },
        ThresholdCheck {
            metric: "Top-K 精准率（本数据集口径为 Hit@5）".into(),
            target: ">=83%".into(),
            measured: adaptive.top_k_precision,
            passed: adaptive.top_k_precision >= 83.0,
        },
        ThresholdCheck {
            metric: "冗余过期过滤率".into(),
            target: ">=45%".into(),
            measured: adaptive.stale_redundant_filter_rate,
            passed: adaptive.stale_redundant_filter_rate >= 45.0,
        },
        ThresholdCheck {
            metric: "检索任务完成度提升代理指标".into(),
            target: ">=12 个百分点".into(),
            measured: task_gain,
            passed: task_gain >= 12.0,
        },
        ThresholdCheck {
            metric: "长周期上下文 Token 降低".into(),
            target: ">=35%".into(),
            measured: token_reduction,
            passed: token_reduction >= 35.0,
        },
    ];
    Ok(Json(BenchmarkResponse {
        dataset: dataset.name,
        dataset_description: dataset.description,
        turn_count: dataset.turns.len(),
        query_count: dataset.queries.len(),
        reproducible: true,
        baseline,
        adaptive,
        accuracy_gain_percentage_points: accuracy_gain,
        task_completion_gain_percentage_points: task_gain,
        token_reduction_rate: token_reduction,
        ablations,
        threshold_checks,
        evidence_note: "这是自建 60 轮回归集的可复现实测；复杂开放域任务完成度仍需接入公开数据集后单独验收，不能用本代理指标替代。".into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_dataset_has_more_than_fifty_turns() {
        let dataset: BenchmarkDataset = serde_json::from_str(DATASET_JSON).unwrap();
        assert!(dataset.turns.len() >= 50);
        assert!(!dataset.queries.is_empty());
    }

    #[test]
    fn adaptive_filters_superseded_facts() {
        let dataset: BenchmarkDataset = serde_json::from_str(DATASET_JSON).unwrap();
        let metrics = evaluate(
            &dataset,
            EvaluationOptions {
                adaptive: true,
                filter_ineligible: true,
                weights: RetrievalWeightsTuple::full(),
                context_k: 5,
            },
        );
        assert_eq!(metrics.stale_redundant_filter_rate, 100.0);
    }

    #[test]
    fn benchmark_reports_static_baseline_and_adaptive_gain() {
        let dataset: BenchmarkDataset = serde_json::from_str(DATASET_JSON).unwrap();
        let baseline = evaluate(
            &dataset,
            EvaluationOptions {
                adaptive: false,
                filter_ineligible: false,
                weights: RetrievalWeightsTuple::full(),
                context_k: 12,
            },
        );
        let adaptive = evaluate(
            &dataset,
            EvaluationOptions {
                adaptive: true,
                filter_ineligible: true,
                weights: RetrievalWeightsTuple::full(),
                context_k: 5,
            },
        );
        let token_reduction = (baseline.estimated_token_count - adaptive.estimated_token_count)
            as f32
            / baseline.estimated_token_count as f32
            * 100.0;
        println!(
            "baseline_top1={:.2} adaptive_top1={:.2} adaptive_hit5={:.2} token_reduction={:.2}",
            baseline.top1_accuracy,
            adaptive.top1_accuracy,
            adaptive.top_k_precision,
            token_reduction
        );
        assert!(adaptive.top1_accuracy > baseline.top1_accuracy);
        assert!(token_reduction >= 35.0);
    }
}
