//! LingShu competition evaluation harness.
//!
//! Produces the evidence tables for the official scorecard. Everything is
//! deterministic: embeddings are frozen on disk, the scoring policy is linked
//! from the server crate, and no network or model download happens at run time.
//!
//! Modes:
//!   (default)   -- run Experiment A/B/D on the available corpora, print tables
//!   --json <path> -- write the full structured record to a JSON file

mod bonus;
mod core;
mod corpus;
mod governance;
mod metrics;
mod report;
mod runner;
mod tasks;

use crate::core::{RetrievalWeights, Strategy, StrategyKind};
use crate::corpus::Corpus;
use crate::report::{CorpusSummary, ExperimentRecord, MetricRow};
use crate::runner::{evaluate, evaluate_with, EvaluationRun};

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().sum::<f64>() / v.len() as f64
}

fn stddev(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

fn print_block(label: &str, run: &EvaluationRun) {
    let m = &run.overall;
    println!(
        "{label:<26} acc={:>6.2}  P@1={:>6.2}  P@3={:>6.2}  P@5={:>6.2}  \
         R@5={:>6.2}  Hit@5={:>6.2}  MRR={:>6.2}  Sup@5={:>6.2}  tok={:>7.1}",
        m.accuracy,
        m.p_at_1,
        m.p_at_3,
        m.p_at_5,
        m.recall_at_5,
        m.hit_at_5,
        m.mrr,
        m.answer_support_at_5,
        m.avg_context_tokens
    );
}

fn print_groups(run: &EvaluationRun) {
    for (key, block) in &run.by_group {
        println!(
            "    {:<12} n={:<5} acc={:>6.2}  P@5={:>6.2}  R@5={:>6.2}  Hit@5={:>6.2}",
            run.group_label(key),
            block.query_count,
            block.accuracy,
            block.p_at_5,
            block.recall_at_5,
            block.hit_at_5
        );
    }
}

fn corpus_summary(corpus: &Corpus) -> CorpusSummary {
    CorpusSummary {
        name: corpus.name.clone(),
        provenance: corpus.provenance.clone(),
        case_count: corpus.cases.len(),
        memory_count: corpus.cases.iter().map(|c| c.memories.len()).sum(),
        query_count: corpus.cases.iter().map(|c| c.queries.len()).sum(),
        embedding_dim: corpus.embeddings.dim,
    }
}

fn query_evidence(run: &EvaluationRun) -> Vec<serde_json::Value> {
    run.outcomes.iter().enumerate().map(|(index, outcome)| serde_json::json!({
        "query_index": index, "expected": outcome.expected, "group": outcome.group,
        "retrieved_ids": outcome.ranked.iter().take(5).map(|hit| &hit.id).collect::<Vec<_>>(),
        "context_tokens": outcome.context_tokens, "filtered_count": outcome.filtered_count,
    })).collect()
}

/// Ablation arms over the in-house (fact-extraction) corpus, where each signal
/// has real, documented discriminating power.
fn ablation_arms() -> Vec<(&'static str, RetrievalWeights, bool)> {
    let full = RetrievalWeights::default();
    let zero = RetrievalWeights {
        semantic: 0.0,
        lexical: 0.0,
        recency: 0.0,
        trust: 0.0,
        frequency: 0.0,
        importance: 0.0,
    };
    vec![
        ("LingShu Full", full.clone(), true),
        (
            "-time decay",
            RetrievalWeights {
                recency: 0.0,
                ..full.clone()
            },
            true,
        ),
        (
            "-source trust",
            RetrievalWeights {
                trust: 0.0,
                ..full.clone()
            },
            true,
        ),
        (
            "-frequency",
            RetrievalWeights {
                frequency: 0.0,
                ..full.clone()
            },
            true,
        ),
        (
            "-importance",
            RetrievalWeights {
                importance: 0.0,
                ..full.clone()
            },
            true,
        ),
        (
            "semantic only",
            RetrievalWeights {
                semantic: 1.0,
                ..zero.clone()
            },
            true,
        ),
        ("no dynamic filter", full, false),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Child-process probe mode: materialise `n` synthetic rows, print RSS, exit.
    // Spawned by `bonus::run_rss` to isolate each scenario's working set.
    if let Some(pos) = args.iter().position(|a| a == "--rss-probe") {
        let n: usize = args.get(pos + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let dim = 384usize;
        let mut data: Vec<f32> = Vec::with_capacity(n * dim);
        data.resize(n * dim, 0.5);
        for slot in data.iter_mut().step_by(dim) {
            *slot = 1.0;
        }
        println!("{}", bonus::probe_rss_bytes());
        return;
    }

    let root = corpus::workspace_root();

    // Focused corpus mode avoids unrelated public-corpus and engineering runs.
    // Same Top-K for every arm; dataset identity never changes scoring.
    if let Some(pos) = args.iter().position(|a| a == "--zte-dataset") {
        let path = args.get(pos + 1).expect("--zte-dataset requires a path");
        let data = corpus::load_zte_path(std::path::Path::new(path)).expect("load custom corpus");
        let mut rows = Vec::new();
        let static_run = evaluate(&data, &Strategy::static_rag(5));
        print_block("Static RAG (K=5)", &static_run);
        rows.push(serde_json::json!({"label": "Static RAG", "metrics": static_run.overall,
                                    "by_group": static_run.by_group, "queries": query_evidence(&static_run)}));
        for (label, weights, filter) in ablation_arms() {
            let strategy = Strategy {
                label: label.into(),
                kind: StrategyKind::Adaptive,
                importance_tiebreak: weights.importance != 0.0,
                weights,
                filter_ineligible: filter,
                context_k: 5,
                lock_weights: false,
            };
            let run = evaluate(&data, &strategy);
            print_block(label, &run);
            rows.push(serde_json::json!({"label": label, "metrics": run.overall,
                                       "by_group": run.by_group, "queries": query_evidence(&run)}));
        }
        let record = serde_json::json!({"schema_version": 2, "context_k": 5,
                                       "corpus": corpus_summary(&data), "rows": rows});
        if let Some(pos) = args.iter().position(|a| a == "--json") {
            let path = args.get(pos + 1).expect("--json requires a path");
            std::fs::write(path, serde_json::to_string_pretty(&record).unwrap())
                .expect("write output");
        }
        return;
    }

    let json_out = args
        .iter()
        .position(|a| a == "--json")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let repeats: usize = args
        .iter()
        .position(|a| a == "--repeats")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    // ---------- Experiment A: public long-horizon retrieval ----------
    let locomo = corpus::load_locomo(&root).expect("LoCoMo corpus missing; run build script");
    let static_rag = evaluate(&locomo, &Strategy::static_rag(12));
    let full = evaluate(&locomo, &Strategy::full());
    let tuned = {
        // The one honest tunable: drop recency on this open-ended English corpus
        // (confirmed by the channel probe and the dev sweep). Kept as a separate
        // row, never folded into the "full" claim.
        let weights = RetrievalWeights {
            semantic: 1.0,
            lexical: 0.25,
            recency: 0.0,
            trust: 0.25,
            frequency: 0.0,
            importance: 0.0,
        };
        let strategy = Strategy {
            label: "LingShu (recency off, tuned)".into(),
            kind: StrategyKind::Adaptive,
            importance_tiebreak: weights.importance != 0.0,
            weights,
            filter_ineligible: true,
            context_k: 5,
            lock_weights: true,
        };
        evaluate(&locomo, &strategy)
    };

    // ---------- Experiment A/B/D: in-house fact-extraction corpus ----------
    let zte = corpus::load_zte(&root).expect("zte dataset missing");
    let zte_static = evaluate(&zte, &Strategy::static_rag(12));
    let zte_full = evaluate(&zte, &Strategy::full());

    println!("=== Experiment A: public long-horizon retrieval (LoCoMo) ===");
    print_block("Static RAG (K=12)", &static_rag);
    print_groups(&static_rag);
    println!();
    print_block("LingShu Full (K=5)", &full);
    print_groups(&full);
    println!();
    print_block("LingShu tuned (no recency)", &tuned);
    println!();

    println!("=== Experiment A/B/D: in-house 60-turn (zte) ===");
    print_block("Static RAG (K=12)", &zte_static);
    println!();
    print_block("LingShu Full (K=5)", &zte_full);
    println!();

    println!("=== Experiment D: ablation (in-house) ===");
    for (label, weights, filter) in ablation_arms() {
        let strategy = Strategy {
            label: label.into(),
            kind: StrategyKind::Adaptive,
            importance_tiebreak: weights.importance != 0.0,
            weights,
            filter_ineligible: filter,
            context_k: 5,
            lock_weights: false,
        };
        let run = evaluate(&zte, &strategy);
        println!(
            "  {:<22} acc={:>6.2}  P@5={:>6.2}  R@5={:>6.2}  Hit@5={:>6.2}  tok={:>6.1}",
            label,
            run.overall.accuracy,
            run.overall.p_at_5,
            run.overall.recall_at_5,
            run.overall.hit_at_5,
            run.overall.avg_context_tokens
        );
    }
    println!();

    // ---------- Repeated-experiment robustness (mean ± stddev) ----------
    let repeat_stats: Option<serde_json::Value> = if repeats > 1 {
        println!("=== Repeated experiment (n={repeats}) ===");

        // (a) LoCoMo: perturb embeddings with σ=0.01 (embedding-seed jitter).
        let mut locomo_static: Vec<f64> = Vec::with_capacity(repeats);
        let mut locomo_adapt: Vec<f64> = Vec::with_capacity(repeats);
        for i in 0..repeats {
            let emb = corpus::perturb_embeddings(&locomo.embeddings, 0x5EED + i as u64, 0.01);
            locomo_static.push(
                evaluate_with(&locomo, &emb, &Strategy::static_rag(12))
                    .overall
                    .accuracy,
            );
            locomo_adapt.push(
                evaluate_with(&locomo, &emb, &Strategy::full())
                    .overall
                    .accuracy,
            );
        }

        // (b) zte: perturb the confidence features (trust/importance/access) with
        //     σ=0.02 -- the signals that drive the fact-extraction scenario.
        let mut zte_static: Vec<f64> = Vec::with_capacity(repeats);
        let mut zte_adapt: Vec<f64> = Vec::with_capacity(repeats);
        for i in 0..repeats {
            let mut z = zte.clone();
            corpus::perturb_features(&mut z, 0x9EED + i as u64, 0.02);
            zte_static.push(evaluate(&z, &Strategy::static_rag(12)).overall.accuracy);
            zte_adapt.push(evaluate(&z, &Strategy::full()).overall.accuracy);
        }

        let stat = |v: &Vec<f64>| {
            let n = v.len() as f64;
            let mean = v.iter().sum::<f64>() / n;
            let var = if n > 1.0 {
                v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
            } else {
                0.0
            };
            serde_json::json!({"mean": mean, "stddev": var.sqrt(), "runs": v})
        };
        println!(
            "  [LoCoMo]  Static {:.2}±{:.2} | LingShu {:.2}±{:.2}",
            mean(&locomo_static),
            stddev(&locomo_static),
            mean(&locomo_adapt),
            stddev(&locomo_adapt)
        );
        println!(
            "  [zte]     Static {:.2}±{:.2} | LingShu {:.2}±{:.2}",
            mean(&zte_static),
            stddev(&zte_static),
            mean(&zte_adapt),
            stddev(&zte_adapt)
        );
        Some(serde_json::json!({
            "repeats": repeats,
            "locomo": {
                "perturbation": "embedding σ=0.01",
                "static_rag_accuracy": stat(&locomo_static),
                "lingshu_accuracy": stat(&locomo_adapt),
            },
            "zte": {
                "perturbation": "feature σ=0.02",
                "static_rag_accuracy": stat(&zte_static),
                "lingshu_accuracy": stat(&zte_adapt),
            },
        }))
    } else {
        None
    };

    if let Some(path) = json_out {
        let record = ExperimentRecord {
            corpus: corpus_summary(&locomo),
            rows: vec![
                MetricRow::new(
                    "Static RAG (K=12)",
                    StrategyKind::Static,
                    &static_rag.overall,
                ),
                MetricRow::new("LingShu Full (K=5)", StrategyKind::Adaptive, &full.overall),
                MetricRow::new("LingShu tuned", StrategyKind::Adaptive, &tuned.overall),
            ],
            channel_probe: Vec::new(),
        };
        let mut all = serde_json::Map::new();
        all.insert("schema_version".into(), serde_json::json!(2));
        all.insert(
            "locomo".into(),
            serde_json::to_value(&record).expect("serialize locomo record"),
        );
        let zte_record = serde_json::json!({
            "corpus": corpus_summary(&zte),
            "static": &zte_static.overall,
            "adaptive": &zte_full.overall,
            "static_by_group": &zte_static.by_group,
            "adaptive_by_group": &zte_full.by_group,
            "ablation": ablation_arms().iter().map(|(label, weights, filter)| {
                let strategy = Strategy {
                    label: (*label).into(),
                    kind: StrategyKind::Adaptive,
                    importance_tiebreak: weights.importance != 0.0,
                    weights: weights.clone(),
                    filter_ineligible: *filter,
                    context_k: 5,
                    lock_weights: false,
                };
                let run = evaluate(&zte, &strategy);
                serde_json::json!({
                    "label": label,
                    "metrics": &run.overall,
                })
            }).collect::<Vec<_>>(),
        });
        all.insert("zte".into(), zte_record);

        let (governance, multi_agent) = governance::run_all();
        all.insert(
            "governance".into(),
            serde_json::to_value(&governance).unwrap(),
        );
        all.insert(
            "multi_agent".into(),
            serde_json::to_value(&multi_agent).unwrap(),
        );
        all.insert(
            "latency".into(),
            serde_json::to_value(bonus::run_latency()).unwrap(),
        );
        all.insert(
            "footprint".into(),
            serde_json::to_value(bonus::run_footprint()).unwrap(),
        );
        all.insert(
            "rss".into(),
            serde_json::to_value(bonus::run_rss()).unwrap(),
        );
        let (task_static, task_adaptive) = tasks::run_task_experiment();
        all.insert(
            "task_static".into(),
            serde_json::to_value(&task_static).unwrap(),
        );
        all.insert(
            "task_adaptive".into(),
            serde_json::to_value(&task_adaptive).unwrap(),
        );
        all.insert(
            "importance_probe".into(),
            serde_json::to_value(tasks::run_importance_probe()).unwrap(),
        );
        if let Some(repeat_stats) = repeat_stats {
            all.insert("repeat".into(), repeat_stats);
        }

        let json = serde_json::to_string_pretty(&all).expect("serialize");
        std::fs::write(&path, json).expect("write json");
        println!("wrote {}", path);
    }
}
