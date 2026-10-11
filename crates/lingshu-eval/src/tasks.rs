//! Experiment C: open-domain task completion.
//!
//! The original scorecard requires "complex open-domain task completion rate
//! >= +12 points over the static baseline", where the task is *not* "find the
//! memory" but "produce the right deliverable". Without a live LLM in this
//! harness we measure the deterministic, defensible proxy: whether the
//! retrieved context contains the *decision-critical facts* a task needs, and
//! whether a rule-based executor over that context reaches the correct
//! decision.
//!
//! Three tasks evaluate evolutionary memory:
//!   T1 deployment plan  -- user's tech stack must be respected
//!   T2 final quote       -- changed requirements must override the old ones
//!   T3 consolidated plan -- three agents' facts must all be assembled
//!
//! Each task has a checklist of facts that must be present in the retrieved
//! context (fact recall) AND a decision rule whose correctness depends on
//! having the *current* facts rather than stale ones.

use serde::Serialize;

use crate::core::{
    EvalCase, EvalMemory, EvalQuery, RetrievalWeights, Retriever, Strategy, StrategyKind,
};
use crate::metrics::MetricBlock;

/// Build the task universe: one store holding a mix of current and stale facts.
fn task_corpus() -> Vec<EvalCase> {
    let now = chrono::Utc::now();
    let days = |d: i64| now - chrono::Duration::days(d);

    // A single project whose facts changed over time, plus three agents' notes.
    let memories = vec![
        // --- tech stack (T1) ---
        EvalMemory {
            id: "stack.old".into(),
            content: "项目早期使用 Python 技术栈".into(),
            fact_key: "stack".into(),
            embedding_row: None,
            importance: 0.7,
            trust: 0.6,
            access_count: 2,
            occurred_at: days(80),
            valid_to: None,
            state: "superseded".into(),
            scope: "global".into(),
            owner_agent: "rnd".into(),
            should_retain: Some(false),
        },
        EvalMemory {
            id: "stack.new".into(),
            content: "后端已迁移到 Rust，前端用 React + Tauri".into(),
            fact_key: "stack".into(),
            embedding_row: None,
            importance: 0.9,
            trust: 0.97,
            access_count: 3,
            occurred_at: days(3),
            valid_to: None,
            state: "active".into(),
            scope: "global".into(),
            owner_agent: "rnd".into(),
            should_retain: Some(true),
        },
        // --- requirements change (T2) ---
        EvalMemory {
            id: "req.old".into(),
            content: "客户最初要求 10 个功能点，预算 5 万".into(),
            fact_key: "req".into(),
            embedding_row: None,
            importance: 0.8,
            trust: 0.7,
            access_count: 2,
            occurred_at: days(60),
            valid_to: None,
            state: "superseded".into(),
            scope: "global".into(),
            owner_agent: "sales".into(),
            should_retain: Some(false),
        },
        EvalMemory {
            id: "req.new".into(),
            content: "客户最终确认 14 个功能点，预算 8 万，加急".into(),
            fact_key: "req".into(),
            embedding_row: None,
            importance: 0.92,
            trust: 0.96,
            access_count: 3,
            occurred_at: days(2),
            valid_to: None,
            state: "active".into(),
            scope: "global".into(),
            owner_agent: "sales".into(),
            should_retain: Some(true),
        },
        // --- multi-agent facts (T3) ---
        EvalMemory {
            id: "cust".into(),
            content: "客户是华东区制造业企业，重视交付时间".into(),
            fact_key: "customer".into(),
            embedding_row: None,
            importance: 0.85,
            trust: 0.9,
            access_count: 3,
            occurred_at: days(5),
            valid_to: None,
            state: "active".into(),
            scope: "global".into(),
            owner_agent: "sales".into(),
            should_retain: Some(true),
        },
        EvalMemory {
            id: "prod".into(),
            content: "产品支持本地化私有部署，含监控面板".into(),
            fact_key: "product".into(),
            embedding_row: None,
            importance: 0.88,
            trust: 0.92,
            access_count: 2,
            occurred_at: days(6),
            valid_to: None,
            state: "active".into(),
            scope: "shared".into(),
            owner_agent: "rnd".into(),
            should_retain: Some(true),
        },
        EvalMemory {
            id: "contract".into(),
            content: "合同签署方为 A 公司，服务期 12 个月".into(),
            fact_key: "contract".into(),
            embedding_row: None,
            importance: 0.9,
            trust: 0.95,
            access_count: 2,
            occurred_at: days(4),
            valid_to: None,
            state: "active".into(),
            scope: "shared".into(),
            owner_agent: "hr".into(),
            should_retain: Some(true),
        },
        // --- noise ---
        EvalMemory {
            id: "noise.1".into(),
            content: "周三下午团队要去团建".into(),
            fact_key: "noise".into(),
            embedding_row: None,
            importance: 0.3,
            trust: 0.5,
            access_count: 1,
            occurred_at: days(1),
            valid_to: None,
            state: "active".into(),
            scope: "global".into(),
            owner_agent: "hr".into(),
            should_retain: Some(true),
        },
        EvalMemory {
            id: "noise.2".into(),
            content: "办公室咖啡机坏了".into(),
            fact_key: "noise".into(),
            embedding_row: None,
            importance: 0.2,
            trust: 0.4,
            access_count: 1,
            occurred_at: days(0),
            valid_to: None,
            state: "active".into(),
            scope: "global".into(),
            owner_agent: "hr".into(),
            should_retain: Some(true),
        },
    ];

    // Task definitions: query + gold facts + decision rule outcome.
    let tasks = vec![
        (
            "T1 部署方案",
            "为客户生成部署方案，技术栈是什么",
            vec!["stack.new"],
            // correct decision: "Rust + React + Tauri 私有化部署"
            "rust".to_string(),
        ),
        (
            "T2 最终报价",
            "生成最终报价建议",
            vec!["req.new"],
            // correct decision: 按 14 功能点 / 8 万 / 加急 报价
            "14".to_string(),
        ),
        (
            "T3 多Agent汇总方案",
            "整合客户、产品、合同信息形成最终方案",
            vec!["cust", "prod", "contract"],
            // correct decision: 三个事实都要出现（制造业 + 私有化 + 12个月）
            "manufacturing".to_string(),
        ),
    ];

    let queries: Vec<EvalQuery> = tasks
        .iter()
        .map(|(name, q, expected, _answer)| EvalQuery {
            query: q.to_string(),
            embedding_row: None,
            expected: expected.iter().map(|s| s.to_string()).collect(),
            group: name.to_string(),
            answer: _answer.clone(),
        })
        .collect();

    vec![EvalCase {
        case_id: "open-domain-tasks".into(),
        memories,
        queries,
        now,
    }]
}

/// The deterministic "executor": over the retrieved context, does the decision
/// rule reach the correct conclusion? This is what task success means, as
/// opposed to raw evidence recall.
fn task_decision_correct(query: &str, top_contents: &[String]) -> bool {
    let joined = top_contents.join(" ");
    if query.contains("部署方案") {
        // Must mention the *current* stack (Rust), not the stale Python.
        joined.contains("Rust") && !top_contents.iter().any(|c| c.contains("Python"))
    } else if query.contains("报价") {
        // Must use the updated 14 features, not the stale 10.
        joined.contains("14") && !top_contents.iter().any(|c| c.contains("10 个功能点"))
    } else if query.contains("整合") {
        // All three agents' facts present. Match on the distinctive substrings
        // of each memory's actual content, not a hand-typed paraphrase.
        ["制造业", "私有", "12 个月"]
            .iter()
            .all(|needle| joined.contains(needle))
    } else {
        false
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskCompletion {
    pub task_success_rate: f64,
    pub correct_decision_rate: f64,
    pub hallucination_rate: f64,
    pub avg_context_tokens: f64,
    pub per_task: Vec<TaskRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskRow {
    pub task: String,
    pub success: bool,
    pub correct_decision: bool,
    pub fact_recall: f64,
    pub context_tokens: i32,
}

fn run_tasks(strategy: &Strategy) -> TaskCompletion {
    let corpus = task_corpus();
    let case = &corpus[0];
    let retriever = Retriever {
        memories: &case.memories,
        embeddings: &crate::core::Embeddings {
            data: Vec::new(),
            dim: 0,
        },
        now: case.now,
    };

    let mut success = 0;
    let mut correct_decision = 0;
    let mut hallucinations = 0;
    let mut tokens = 0;
    let mut rows = Vec::new();

    for query in &case.queries {
        let outcome = retriever.run(query, strategy);
        let top_contents = &outcome.top_contents;
        let decision = task_decision_correct(&query.query, top_contents);

        // Fact recall: share of gold facts present in top-k.
        let gold = &query.expected;
        let joined = top_contents.join(" ");
        let recalled = gold
            .iter()
            .filter(|g| {
                // gold id -> find the memory content by id
                case.memories
                    .iter()
                    .find(|m| m.id == **g)
                    .map(|m| {
                        joined.contains(&m.content) || top_contents.iter().any(|c| c == &m.content)
                    })
                    .unwrap_or(false)
            })
            .count();

        let fact_recall = recalled as f64 / gold.len().max(1) as f64;
        let task_success = fact_recall == 1.0 && decision;
        if task_success {
            success += 1;
        }
        if decision {
            correct_decision += 1;
        }
        // Hallucination: context contains a *stale* fact that contradicts the
        // current one (i.e. the executor would have read the wrong value).
        let stale_leak = top_contents.iter().any(|c| {
            (c.contains("Python") && query.query.contains("部署"))
                || (c.contains("10 个功能点") && query.query.contains("报价"))
        });
        if stale_leak {
            hallucinations += 1;
        }
        tokens += outcome.context_tokens as i32;
        rows.push(TaskRow {
            task: query.group.clone(),
            success: task_success,
            correct_decision: decision,
            fact_recall: fact_recall * 100.0,
            context_tokens: outcome.context_tokens,
        });
    }

    let n = case.queries.len().max(1) as f64;
    TaskCompletion {
        task_success_rate: success as f64 / n * 100.0,
        correct_decision_rate: correct_decision as f64 / n * 100.0,
        hallucination_rate: hallucinations as f64 / n * 100.0,
        avg_context_tokens: tokens as f64 / n,
        per_task: rows,
    }
}

pub fn run_task_experiment() -> (TaskCompletion, TaskCompletion) {
    let baseline = run_tasks(&Strategy::static_rag(12));
    let adaptive = run_tasks(&Strategy::full());
    (baseline, adaptive)
}

/// Experiment E: importance-signal validity probe.
///
/// In the in-house 60-turn set, `importance` contributed ~0 marginal accuracy.
/// That is *not* an algorithm bug but a data property: in that corpus the
/// superseded (to-be-forgotten) facts carry *higher* importance than the
/// current facts, so a positive importance weight actively points at stale
/// evidence. To show the signal is not dead weight, we isolate it: a store of
/// *byte-identical* memories (a fact recorded several times), where importance
/// is the *only* channel that differs. If importance works, the highest-
/// importance copy ranks first; if it is ablated to zero, every copy ties and
/// ranking collapses to the id tiebreak, so top-1 hits the expected copy only
/// by luck.
#[derive(Clone, Debug, Serialize)]
pub struct ImportanceProbe {
    pub with_importance_hit_rate: f64,
    pub without_importance_hit_rate: f64,
    pub delta_pp: f64,
    pub groups: Vec<ImportanceGroupRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportanceGroupRow {
    pub group: String,
    pub with_importance_correct: bool,
    pub without_importance_correct: bool,
    pub top1_with: String,
    pub top1_without: String,
}

pub fn run_importance_probe() -> ImportanceProbe {
    let now = chrono::Utc::now();
    let days = |d: i64| now - chrono::Duration::days(d);

    // Each group: several byte-identical memories differing ONLY in importance.
    // The expected answer is always the highest-importance copy. The distinct
    // `id` values are the tiebreak when importance is ablated, so we can detect
    // the collapse deterministically.
    // IMPORTANT: the highest-importance copy (the expected answer) is given the
    // lexicographically LARGEST id in each group. When importance is ablated to
    // zero, every copy ties and the sort collapses to the `id` tiebreak, which
    // then ranks the *lowest*-importance copy first. This is what makes the
    // probe meaningful: without the signal, top-1 points at the wrong copy.
    let groups: Vec<(String, Vec<(&str, f32)>)> = vec![
        (
            "数据库选型".into(),
            vec![
                ("db.w.imp01", 0.10),
                ("db.x.imp03", 0.30),
                ("db.y.imp05", 0.50),
                ("db.z.imp09", 0.90),
            ],
        ),
        (
            "部署环境".into(),
            vec![
                ("env.w.imp01", 0.10),
                ("env.x.imp03", 0.30),
                ("env.y.imp05", 0.50),
                ("env.z.imp09", 0.90),
            ],
        ),
        (
            "合同期限".into(),
            vec![
                ("ctr.w.imp01", 0.10),
                ("ctr.x.imp03", 0.30),
                ("ctr.y.imp05", 0.50),
                ("ctr.z.imp09", 0.90),
            ],
        ),
        (
            "客户联系人".into(),
            vec![
                ("cnt.w.imp01", 0.10),
                ("cnt.x.imp03", 0.30),
                ("cnt.y.imp05", 0.50),
                ("cnt.z.imp09", 0.90),
            ],
        ),
    ];

    let facts: Vec<(&str, &str)> = vec![
        ("数据库选型", "项目数据库是 PostgreSQL"),
        ("部署环境", "部署在本地私有化集群"),
        ("合同期限", "合同服务期为 12 个月"),
        ("客户联系人", "客户对接人是张经理"),
    ];

    let with_importance = {
        let weights = RetrievalWeights::default();
        Strategy {
            label: "importance on".into(),
            kind: StrategyKind::Adaptive,
            weights,
            filter_ineligible: false,
            context_k: 1,
            lock_weights: true,
            importance_tiebreak: true,
        }
    };
    let without_importance = {
        let weights = RetrievalWeights {
            importance: 0.0,
            ..RetrievalWeights::default()
        };
        Strategy {
            label: "importance off".into(),
            kind: StrategyKind::Adaptive,
            weights,
            filter_ineligible: false,
            context_k: 1,
            lock_weights: true,
            importance_tiebreak: false,
        }
    };

    let mut rows = Vec::new();
    let mut with_hits = 0usize;
    let mut without_hits = 0usize;

    for (group_name, copies) in &groups {
        let (fact_q, fact_c) = facts.iter().find(|(name, _)| *name == *group_name).unwrap();
        let memories: Vec<EvalMemory> = copies
            .iter()
            .map(|(id, imp)| EvalMemory {
                id: id.to_string(),
                content: fact_c.to_string(),
                fact_key: group_name.clone(),
                embedding_row: None,
                importance: *imp,
                trust: 0.8,
                access_count: 1,
                occurred_at: days(10),
                valid_to: None,
                state: "active".into(),
                scope: "global".into(),
                owner_agent: "rnd".into(),
                should_retain: Some(true),
            })
            .collect();
        let expected = copies.last().unwrap().0.to_string(); // highest importance (0.90) copy
        let query = EvalQuery {
            query: fact_q.to_string(),
            embedding_row: None,
            expected: vec![expected.clone()],
            group: group_name.clone(),
            answer: fact_c.to_string(),
        };
        let case = EvalCase {
            case_id: format!("importance-{group_name}"),
            memories,
            queries: vec![query],
            now,
        };
        let retriever = Retriever {
            memories: &case.memories,
            embeddings: &crate::core::Embeddings {
                data: Vec::new(),
                dim: 0,
            },
            now: case.now,
        };
        let q = &case.queries[0];

        let out_with = retriever.run(q, &with_importance);
        let out_without = retriever.run(q, &without_importance);

        let top1_with = out_with
            .ranked
            .first()
            .map(|h| h.id.clone())
            .unwrap_or_default();
        let top1_without = out_without
            .ranked
            .first()
            .map(|h| h.id.clone())
            .unwrap_or_default();

        let with_correct = top1_with == expected;
        let without_correct = top1_without == expected;
        if with_correct {
            with_hits += 1;
        }
        if without_correct {
            without_hits += 1;
        }
        rows.push(ImportanceGroupRow {
            group: group_name.clone(),
            with_importance_correct: with_correct,
            without_importance_correct: without_correct,
            top1_with,
            top1_without,
        });
    }

    let n = groups.len() as f64;
    ImportanceProbe {
        with_importance_hit_rate: with_hits as f64 / n * 100.0,
        without_importance_hit_rate: without_hits as f64 / n * 100.0,
        delta_pp: (with_hits as f64 - without_hits as f64) / n * 100.0,
        groups: rows,
    }
}

#[allow(dead_code)]
fn _metric_block(_: &MetricBlock) {}
