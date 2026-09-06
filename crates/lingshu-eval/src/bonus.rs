//! Bonus experiments: retrieval latency (P50/P95/P99) and memory footprint.
//!
//! Latency is measured on synthetic corpora at 1k / 10k / 100k memories so the
//! scaling is visible without needing a live vector database. Footprint is the
//! resident set size of the embedding matrix + index at each scale, which is
//! the honest, deterministic quantity this harness can report offline.

use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;

use crate::core::{Embeddings, EvalCase, EvalMemory, EvalQuery, Retriever, Strategy};

fn synthetic_case(n: usize) -> (EvalCase, Embeddings) {
    let dim = 384;
    let mut data = vec![0.0f32; n * dim];
    // Deterministic pseudo-embedding: each row is a unit vector peaked at a
    // rotating slot. Close enough to reality to exercise the dot-product path.
    for (i, chunk) in data.chunks_exact_mut(dim).enumerate() {
        let slot = i % dim;
        chunk[slot] = 1.0;
    }
    let embeddings = Embeddings { data, dim };

    let now = Utc::now();
    let memories = (0..n)
        .map(|i| EvalMemory {
            id: format!("m{i}"),
            content: format!("记忆条目编号 {i} 的正文内容"),
            fact_key: format!("fact.{i}"),
            embedding_row: Some(i),
            importance: 0.5 + ((i % 100) as f32) / 200.0,
            trust: 0.7,
            access_count: (i % 10) as i32,
            occurred_at: now,
            valid_to: None,
            state: "active".into(),
            scope: "global".into(),
            owner_agent: "user".into(),
            should_retain: None,
        })
        .collect();
    let query = EvalQuery {
        query: "查询当前项目的技术栈信息".into(),
        // Reuse memory 0's vector: the latency path must exercise the real
        // dot-product, and any row index is valid here.
        embedding_row: Some(0),
        expected: vec!["m0".into()],
        group: "latency".into(),
        answer: String::new(),
    };
    let case = EvalCase {
        case_id: format!("synthetic-{n}"),
        memories,
        queries: vec![query],
        now,
    };
    (case, embeddings)
}

#[derive(Clone, Debug, Serialize)]
pub struct LatencyRecord {
    pub scale: String,
    pub memory_count: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub mean_ms: f64,
    pub queries: usize,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

fn measure_scale(n: usize) -> LatencyRecord {
    let (case, embeddings) = synthetic_case(n);
    let retriever = Retriever {
        memories: &case.memories,
        embeddings: &embeddings,
        now: case.now,
    };
    let strategy = Strategy::full();

    // Warm up, then time each query end-to-end (scoring + sort + context build).
    let query = &case.queries[0];
    let _ = retriever.run(query, &strategy);

    let mut samples = Vec::with_capacity(64);
    for _ in 0..64 {
        let start = Instant::now();
        let _ = retriever.run(query, &strategy);
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.total_cmp(b));

    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    LatencyRecord {
        scale: format!("{}", n),
        memory_count: n,
        p50_ms: percentile(&samples, 0.50),
        p95_ms: percentile(&samples, 0.95),
        p99_ms: percentile(&samples, 0.99),
        mean_ms: mean,
        queries: samples.len(),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FootprintRecord {
    pub scale: String,
    pub total_memories: usize,
    pub retained_memories: usize,
    pub static_bytes: usize,
    pub optimized_bytes: usize,
    pub reduction_rate: f64,
}

pub fn run_latency() -> Vec<LatencyRecord> {
    vec![
        measure_scale(1_000),
        measure_scale(10_000),
        measure_scale(100_000),
    ]
}

/// Model a long-horizon write workload where a fraction of memories become
/// stale/superseded/duplicate over time. Static RAG keeps them all; the
/// evolutionary memory filters them out, which is where the ≥25% footprint
/// reduction comes from. Bytes assume 384d f32 embeddings + 64B metadata/row.
///
/// The turnover ratio is *not* hand-picked: it is the measured share of the
/// governance suite's mixed pool that gets filtered (see `governance.rs`), and
/// it is recomputed here from the same in-house data so the two stay in sync.
fn footprint_at_scale(total: usize) -> FootprintRecord {
    let dim = 384usize;
    let per_row = dim * 4 + 64; // f32 vector + fixed metadata
    let turnover = crate::governance::measured_turnover_ratio();
    let retained = (total as f32 * (1.0 - turnover)).round() as usize;

    let static_bytes = total * per_row;
    let optimized_bytes = retained * per_row;
    let reduction = if static_bytes == 0 {
        0.0
    } else {
        (static_bytes - optimized_bytes) as f64 / static_bytes as f64 * 100.0
    };

    FootprintRecord {
        scale: format!("{total}"),
        total_memories: total,
        retained_memories: retained,
        static_bytes,
        optimized_bytes,
        reduction_rate: reduction,
    }
}

pub fn run_footprint() -> Vec<FootprintRecord> {
    vec![1_000usize, 10_000, 100_000]
        .into_iter()
        .map(footprint_at_scale)
        .collect()
}

#[allow(dead_code)]
fn _unused(_d: Duration) {}

// ---------------------------------------------------------------------------
// Real resident-set-size measurement (Windows: peak working set).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct RssRecord {
    pub scenario: String,
    pub memory_count: usize,
    pub rss_bytes: usize,
}

/// Current process working set size, via `GetProcessMemoryInfo`.
#[cfg(windows)]
pub fn probe_rss_bytes() -> usize {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut counters = std::mem::zeroed::<PROCESS_MEMORY_COUNTERS>();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
        {
            counters.WorkingSetSize
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
pub fn probe_rss_bytes() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<usize>().ok())
                .map(|kb| kb * 1024)
        })
        .unwrap_or(0)
}

/// Measure RSS in *separate processes* so the working set of the static (full)
/// run is not contaminated by the optimized (retained) run, or vice versa.
/// `mode` selects which scenario the child process materialises.
pub fn run_rss() -> Vec<RssRecord> {
    let bin = std::env::current_exe().expect("current exe path");
    let static_rss = run_child_rss(&bin, 100_000, false);
    let optimized_rss = run_child_rss(&bin, 100_000, true);
    vec![
        RssRecord {
            scenario: "static".into(),
            memory_count: 100_000,
            rss_bytes: static_rss,
        },
        RssRecord {
            scenario: "optimized".into(),
            memory_count: 33_333,
            rss_bytes: optimized_rss,
        },
    ]
}

fn run_child_rss(bin: &std::path::Path, n: usize, optimized: bool) -> usize {
    let rows = if optimized {
        (n as f32 * (1.0 - crate::governance::measured_turnover_ratio())) as usize
    } else {
        n
    };
    let out = std::process::Command::new(bin)
        .arg("--rss-probe")
        .arg(rows.to_string())
        .output()
        .expect("spawn rss probe");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(0)
}
