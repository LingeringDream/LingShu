//! Serialisable experiment records for the final report.

use serde::Serialize;

use crate::core::StrategyKind;
use crate::metrics::MetricBlock;

#[derive(Serialize)]
pub struct MetricRow {
    pub label: String,
    pub kind: String,
    pub metrics: MetricBlock,
}

impl MetricRow {
    pub fn new(label: &str, kind: StrategyKind, block: &MetricBlock) -> Self {
        Self {
            label: label.to_string(),
            kind: match kind {
                StrategyKind::Static => "static".to_string(),
                StrategyKind::Adaptive => "adaptive".to_string(),
            },
            metrics: block.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct CorpusSummary {
    pub name: String,
    pub provenance: String,
    pub case_count: usize,
    pub memory_count: usize,
    pub query_count: usize,
    pub embedding_dim: usize,
}

#[derive(Serialize)]
pub struct ExperimentRecord {
    pub corpus: CorpusSummary,
    pub rows: Vec<MetricRow>,
    pub channel_probe: Vec<MetricRow>,
}
