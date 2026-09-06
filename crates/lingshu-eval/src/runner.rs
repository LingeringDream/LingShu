//! Parallel evaluation driver. Cases are independent, so they fan out across
//! cores; within a case the query order is preserved for reproducible output.

use std::collections::BTreeMap;

use rayon::prelude::*;

use crate::core::{QueryOutcome, Retriever, Strategy};
use crate::corpus::Corpus;
use crate::metrics::MetricBlock;

#[derive(Clone, Debug)]
pub struct EvaluationRun {
    pub overall: MetricBlock,
    pub by_group: BTreeMap<String, MetricBlock>,
    /// Memories dropped by eligibility filtering, summed over all queries.
    #[allow(dead_code)]
    pub total_filtered: usize,
    pub outcomes: Vec<QueryOutcome>,
}

impl EvaluationRun {
    pub fn group_label(&self, key: &str) -> String {
        // LoCoMo encodes question type as 1..5; give it a readable name.
        match key {
            "1" => "multi-hop".to_string(),
            "2" => "temporal".to_string(),
            "3" => "open-domain".to_string(),
            "4" => "single-hop".to_string(),
            "5" => "adversarial".to_string(),
            other => other.to_string(),
        }
    }
}

pub fn evaluate(corpus: &Corpus, strategy: &Strategy) -> EvaluationRun {
    evaluate_with(corpus, &corpus.embeddings, strategy)
}

/// Evaluate with a caller-supplied embedding matrix (used by repeated-experiment
/// robustness runs that perturb the vectors).
pub fn evaluate_with(
    corpus: &Corpus,
    embeddings: &crate::core::Embeddings,
    strategy: &Strategy,
) -> EvaluationRun {
    let per_case: Vec<Vec<QueryOutcome>> = corpus
        .cases
        .par_iter()
        .map(|case| {
            let retriever = Retriever {
                memories: &case.memories,
                embeddings,
                now: case.now,
            };
            case.queries
                .iter()
                .map(|query| retriever.run(query, strategy))
                .collect()
        })
        .collect();

    let mut all: Vec<QueryOutcome> = Vec::new();
    let mut groups: BTreeMap<String, Vec<QueryOutcome>> = BTreeMap::new();
    for outcomes in per_case {
        for outcome in outcomes {
            groups
                .entry(outcome.group.clone())
                .or_default()
                .push(outcome.clone());
            all.push(outcome);
        }
    }

    let total_filtered = all.iter().map(|outcome| outcome.filtered_count).sum();
    let by_group = groups
        .into_iter()
        .map(|(key, outcomes)| (key, MetricBlock::from_outcomes(&outcomes)))
        .collect();

    EvaluationRun {
        overall: MetricBlock::from_outcomes(&all),
        by_group,
        total_filtered,
        outcomes: all,
    }
}
