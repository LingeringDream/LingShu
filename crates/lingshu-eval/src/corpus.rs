//! Corpus loaders. Every loader normalises into the same `EvalCase` shape so the
//! experiments never branch on dataset identity.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::core::{Embeddings, EvalCase, EvalMemory, EvalQuery};

pub fn workspace_root() -> PathBuf {
    // crates/lingshu-eval -> workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn parse_time(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[derive(Debug, Deserialize)]
struct LocomoMemory {
    row: usize,
    id: String,
    content: String,
    #[serde(default)]
    speaker: String,
    #[serde(default)]
    session: usize,
    occurred_at: Option<String>,
    importance: f32,
    trust: f32,
    #[serde(default)]
    access_count: i32,
    #[serde(default = "default_state")]
    state: String,
}

fn default_state() -> String {
    "active".into()
}

#[derive(Debug, Deserialize)]
struct LocomoQuery {
    row: usize,
    query: String,
    expected: Vec<String>,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    category: u32,
}

#[derive(Debug, Deserialize)]
struct LocomoCase {
    case_id: String,
    memories: Vec<LocomoMemory>,
    queries: Vec<LocomoQuery>,
}

#[derive(Debug, Deserialize)]
struct LocomoCorpus {
    name: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    embedding_model: String,
    cases: Vec<LocomoCase>,
}

pub struct Corpus {
    pub name: String,
    pub provenance: String,
    pub cases: Vec<EvalCase>,
    pub embeddings: Embeddings,
}

impl Clone for Corpus {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            provenance: self.provenance.clone(),
            cases: self.cases.clone(),
            embeddings: Embeddings {
                data: self.embeddings.data.clone(),
                dim: self.embeddings.dim,
            },
        }
    }
}

/// Perturb the confidence features (trust / importance / access_count) of every
/// memory with deterministic noise, for repeated-experiment robustness on the
/// in-house fact-extraction corpus (which has no embedding matrix).
pub fn perturb_features(corpus: &mut Corpus, seed: u64, sigma: f32) {
    let mut state = seed | 1;
    let mut gauss = || {
        let mut g = 0.0f32;
        for _ in 0..4 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = (state >> 11) as f32 / (1u64 << 53) as f32;
            g += u - 0.5;
        }
        g * sigma
    };
    for case in corpus.cases.iter_mut() {
        for memory in case.memories.iter_mut() {
            memory.trust = (memory.trust + gauss()).clamp(0.0, 1.0);
            memory.importance = (memory.importance + gauss()).clamp(0.0, 1.0);
            let acc_delta = gauss();
            if acc_delta >= 0.5 {
                memory.access_count += 1;
            } else if acc_delta <= -0.5 {
                memory.access_count = (memory.access_count - 1).max(0);
            }
        }
    }
}

/// Apply isotropic Gaussian-like noise to the embedding matrix, for repeated-
/// experiment robustness. Uses a deterministic LCG so a given seed always
/// reproduces the same perturbation (no external RNG dependency).
///
/// `sigma` is the standard deviation added per coordinate. Because rows are
/// L2-normalised, a small `sigma` (e.g. 0.01) models embedding-training-seed /
/// floating-point jitter without changing rankings materially -- exactly what a
/// mean±stddev repeat should isolate.
pub fn perturb_embeddings(embeddings: &Embeddings, seed: u64, sigma: f32) -> Embeddings {
    let mut data = embeddings.data.clone();
    let mut state = seed | 1; // LCG must not start at 0
    for value in data.iter_mut() {
        // Box-Muller-free approximation via sum of uniforms (CLT), deterministic.
        let mut gauss = 0.0f32;
        for _ in 0..4 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = (state >> 11) as f32 / (1u64 << 53) as f32; // [0,1)
            gauss += u - 0.5;
        }
        *value += gauss * sigma;
    }
    Embeddings {
        data,
        dim: embeddings.dim,
    }
}

/// Read the row-major float32 matrix written by `eval/build_locomo_corpus.py`.
/// Header is `[n_rows: u32 LE][dim: u32 LE]` followed by `n_rows * dim` f32.
pub fn load_embeddings(path: &Path) -> Result<Embeddings, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if bytes.len() < 8 {
        return Err("embedding file shorter than header".into());
    }
    let n_rows = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let dim = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let expected = 8 + n_rows * dim * 4;
    if bytes.len() < expected {
        return Err(format!(
            "embedding file truncated: expected {expected} bytes, got {}",
            bytes.len()
        ));
    }
    let mut data = vec![0.0f32; n_rows * dim];
    for (slot, chunk) in data.iter_mut().zip(bytes[8..expected].chunks_exact(4)) {
        *slot = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    Ok(Embeddings { data, dim })
}

#[derive(Debug, Deserialize)]
struct ZteTurn {
    #[allow(dead_code)]
    turn: usize,
    id: String,
    content: String,
    fact_key: String,
    #[allow(dead_code)]
    value: String,
    age_days: i64,
    importance: f32,
    trust: f32,
    #[serde(default)]
    access: i32,
    state: String,
}

#[derive(Debug, Deserialize)]
struct ZteQuery {
    query: String,
    expected: String,
    #[serde(default)]
    answer: String,
    /// Optional difficulty tag. The "fact-change" queries (a fact was superseded
    /// and the current value must win) are the core long-horizon scenario; the
    /// rest are direct current-fact lookups. Defaults to "fact".
    #[serde(default = "default_group")]
    group: String,
}

fn default_group() -> String {
    "fact".to_string()
}

#[derive(Debug, Deserialize)]
struct ZteDataset {
    name: String,
    description: String,
    turns: Vec<ZteTurn>,
    queries: Vec<ZteQuery>,
}

/// Load the in-house 60-turn regression set. This is the *fact-extraction*
/// scenario where recency/trust/importance genuinely carry signal (unlike the
/// open-ended LoCoMo chat logs). No embedding matrix: the corpus is Chinese and
/// tiny, so the lexical fallback is the semantic signal.
pub fn load_zte(root: &Path) -> Result<Corpus, String> {
    let path = root.join("datasets/zte_memory_60turn_v2.json");
    load_zte_path(&path)
}

pub fn load_zte_path(path: &Path) -> Result<Corpus, String> {
    let raw = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let parsed: ZteDataset =
        serde_json::from_str(&raw).map_err(|e| format!("parse zte dataset: {e}"))?;

    let now = parse_time("2026-09-06T00:00:00Z");
    let mut memories = Vec::with_capacity(parsed.turns.len());
    for turn in parsed.turns {
        memories.push(EvalMemory {
            id: turn.id,
            // Natural-language content is what the scoring policy reads; the
            // original benchmark ranks on `turn.content`, not on fact_key/value.
            content: turn.content,
            fact_key: turn.fact_key,
            embedding_row: None,
            importance: turn.importance,
            trust: turn.trust,
            access_count: turn.access,
            occurred_at: now - chrono::Duration::days(turn.age_days),
            valid_to: None,
            state: turn.state,
            scope: "global".into(),
            owner_agent: "user".into(),
            should_retain: None,
        });
    }
    let queries = parsed
        .queries
        .into_iter()
        .map(|q| EvalQuery {
            query: q.query,
            embedding_row: None,
            expected: vec![q.expected],
            group: q.group,
            answer: q.answer,
        })
        .collect();

    Ok(Corpus {
        name: parsed.name.clone(),
        provenance: parsed.description,
        cases: vec![EvalCase {
            case_id: parsed.name,
            memories,
            queries,
            now,
        }],
        embeddings: Embeddings {
            data: Vec::new(),
            dim: 0,
        },
    })
}

pub fn load_locomo(root: &Path) -> Result<Corpus, String> {
    let json_path = root.join("datasets/public/locomo_corpus.json");
    let bin_path = root.join("datasets/public/locomo_emb.f32");
    let raw =
        fs::read_to_string(&json_path).map_err(|e| format!("read {}: {e}", json_path.display()))?;
    let parsed: LocomoCorpus =
        serde_json::from_str(&raw).map_err(|e| format!("parse locomo corpus: {e}"))?;
    let embeddings = load_embeddings(&bin_path)?;

    let mut cases = Vec::new();
    for case in parsed.cases {
        let mut memories = Vec::with_capacity(case.memories.len());
        let mut newest: Option<DateTime<Utc>> = None;
        for memory in case.memories {
            let occurred_at = memory
                .occurred_at
                .as_deref()
                .map(parse_time)
                .unwrap_or_else(Utc::now);
            newest = Some(match newest {
                Some(current) => current.max(occurred_at),
                None => occurred_at,
            });
            memories.push(EvalMemory {
                id: memory.id,
                content: memory.content,
                fact_key: format!("s{}.t{}", memory.session, memory.speaker),
                embedding_row: Some(memory.row),
                importance: memory.importance,
                trust: memory.trust,
                access_count: memory.access_count,
                occurred_at,
                valid_to: None,
                state: memory.state,
                scope: "global".into(),
                owner_agent: memory.speaker,
                should_retain: None,
            });
        }
        let queries = case
            .queries
            .into_iter()
            .map(|q| EvalQuery {
                query: q.query,
                embedding_row: Some(q.row),
                expected: q.expected,
                group: q.category.to_string(),
                answer: q.answer,
            })
            .collect();
        cases.push(EvalCase {
            case_id: case.case_id,
            memories,
            queries,
            now: newest.unwrap_or_else(Utc::now),
        });
    }

    Ok(Corpus {
        name: parsed.name,
        provenance: format!("{} | {}", parsed.source, parsed.embedding_model),
        cases,
        embeddings,
    })
}
