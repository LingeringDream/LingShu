//! Metric definitions for the official scorecard.
//!
//! `p_at_k` is standard precision: hits@K / K. Historical v1 output instead
//! used hits@K / min(K, |gold|); that metric is now named `capped_recall_at_k`.
//! `p_at_k_strict` remains an alias for consumers of the historical schema.

use crate::core::QueryOutcome;
use serde::Serialize;

pub const K_VALUES: [usize; 3] = [1, 3, 5];

#[derive(Clone, Debug, Default, Serialize)]
pub struct MetricBlock {
    /// Top-1 exact match rate; the "long-horizon memory accuracy" headline.
    pub accuracy: f64,
    pub p_at_1: f64,
    pub p_at_3: f64,
    pub p_at_5: f64,
    pub capped_recall_at_1: f64,
    pub capped_recall_at_3: f64,
    pub capped_recall_at_5: f64,
    pub p_at_1_strict: f64,
    pub p_at_3_strict: f64,
    pub p_at_5_strict: f64,
    pub recall_at_5: f64,
    pub hit_at_5: f64,
    pub mrr: f64,
    /// Share of queries whose reference answer string is present in the
    /// retrieved context. Literal coverage is a diagnostic, not a ceiling on
    /// generation: paraphrases and multi-hop inference need not contain it.
    pub answer_support_at_5: f64,
    pub answer_support_at_10: f64,
    /// Average tokens of the assembled context (lower is better).
    pub avg_context_tokens: f64,
    pub query_count: usize,
}

impl MetricBlock {
    pub fn from_outcomes(outcomes: &[QueryOutcome]) -> Self {
        let n = outcomes.len();
        if n == 0 {
            return Self::default();
        }
        let mut accuracy = 0.0f64;
        let mut p = [0.0f64; 3];
        let mut p_strict = [0.0f64; 3];
        let mut recall5 = 0.0f64;
        let mut hit5 = 0.0f64;
        let mut mrr = 0.0f64;
        let mut tokens = 0i64;
        let mut support5 = 0.0f64;
        let mut support10 = 0.0f64;

        for outcome in outcomes {
            let gold: std::collections::HashSet<&str> =
                outcome.expected.iter().map(String::as_str).collect();
            let ids: Vec<&str> = outcome.ranked.iter().map(|hit| hit.id.as_str()).collect();

            if ids.first().is_some_and(|top| gold.contains(top)) {
                accuracy += 1.0;
            }

            for (idx, k) in K_VALUES.iter().enumerate() {
                let window = &ids[..ids.len().min(*k)];
                let hits = window
                    .iter()
                    .filter(|id| gold.contains(*id))
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                let denominator = (*k).min(gold.len().max(1));
                p[idx] += hits as f64 / denominator as f64;
                p_strict[idx] += hits as f64 / *k as f64;
            }

            let hits5 = ids
                .iter()
                .take(5)
                .filter(|id| gold.contains(*id))
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len();
            recall5 += hits5 as f64 / gold.len().max(1) as f64;
            if hits5 > 0 {
                hit5 += 1.0;
            }

            if let Some(rank) = ids
                .iter()
                .position(|id| gold.contains(id))
                .map(|position| position + 1)
            {
                mrr += 1.0 / rank as f64;
            }
            tokens += outcome.context_tokens as i64;

            if answer_supported(
                &outcome.answer,
                &outcome.top_contents[..outcome.top_contents.len().min(5)],
            ) {
                support5 += 1.0;
            }
            if answer_supported(
                &outcome.answer,
                &outcome.top_contents[..outcome.top_contents.len().min(10)],
            ) {
                support10 += 1.0;
            }
        }

        let n = n as f64;
        Self {
            accuracy: accuracy / n * 100.0,
            p_at_1: p_strict[0] / n * 100.0,
            p_at_3: p_strict[1] / n * 100.0,
            p_at_5: p_strict[2] / n * 100.0,
            capped_recall_at_1: p[0] / n * 100.0,
            capped_recall_at_3: p[1] / n * 100.0,
            capped_recall_at_5: p[2] / n * 100.0,
            p_at_1_strict: p_strict[0] / n * 100.0,
            p_at_3_strict: p_strict[1] / n * 100.0,
            p_at_5_strict: p_strict[2] / n * 100.0,
            recall_at_5: recall5 / n * 100.0,
            hit_at_5: hit5 / n * 100.0,
            mrr: mrr / n * 100.0,
            answer_support_at_5: support5 / n * 100.0,
            answer_support_at_10: support10 / n * 100.0,
            avg_context_tokens: tokens as f64 / n,
            query_count: outcomes.len(),
        }
    }
}

/// Is the reference answer literally recoverable from this context?
///
/// Answers shorter than 4 characters are matched with an alphanumeric boundary
/// check, otherwise "2" would trivially match inside "2023".
pub fn answer_supported(answer: &str, context: &[String]) -> bool {
    let needle = normalize_answer(answer);
    if needle.is_empty() {
        return false;
    }
    let strict = needle.chars().count() < 4;
    for text in context {
        let haystack = if strict {
            text.chars()
                .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
                .flat_map(char::to_lowercase)
                .collect::<String>()
        } else {
            normalize_answer(text)
        };
        if haystack.is_empty() {
            continue;
        }
        if !strict {
            if haystack.contains(&needle) {
                return true;
            }
        } else {
            // Scan every occurrence so the boundary test can reject "2" in "2023".
            let mut cursor = 0usize;
            while let Some(offset) = haystack[cursor..].find(&needle) {
                let start = cursor + offset;
                let end = start + needle.len();
                let before_ok = start == 0
                    || !haystack[..start]
                        .chars()
                        .next_back()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric());
                let after_ok = end >= haystack.len()
                    || !haystack[end..]
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric());
                if before_ok && after_ok {
                    return true;
                }
                cursor = start + haystack[start..].chars().next().unwrap().len_utf8();
            }
        }
    }
    false
}

fn normalize_answer(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

/// Mean and sample standard deviation over repeated runs.
#[derive(Clone, Debug, Serialize)]
pub struct RepeatStats {
    pub mean: f64,
    pub stddev: f64,
    pub runs: Vec<f64>,
}

impl RepeatStats {
    #[allow(dead_code)]
    pub fn new(runs: Vec<f64>) -> Self {
        let n = runs.len().max(1) as f64;
        let mean = runs.iter().sum::<f64>() / n;
        let variance = if runs.len() > 1 {
            runs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        Self {
            mean,
            stddev: variance.sqrt(),
            runs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Hit;

    #[test]
    fn precision_uses_k_even_when_there_is_only_one_gold() {
        let outcome = QueryOutcome {
            ranked: ["noise", "gold", "x", "y", "z"]
                .iter()
                .map(|id| Hit {
                    id: (*id).into(),
                    score: 0.0,
                })
                .collect(),
            expected: vec!["gold".into()],
            group: "test".into(),
            context_tokens: 0,
            filtered_count: 0,
            top_contents: vec![],
            answer: String::new(),
        };
        let block = MetricBlock::from_outcomes(&[outcome]);
        assert_eq!(block.p_at_5, 20.0);
        assert_eq!(block.capped_recall_at_5, 100.0);
        assert_eq!(block.recall_at_5, 100.0);
        assert_eq!(block.mrr, 50.0);
    }

    #[test]
    fn rejected_short_unicode_match_advances_by_character() {
        assert!(!answer_supported("猫", &["a猫b".into()]));
        assert!(answer_supported("杭州", &["用户住在杭州。".into()]));
        assert!(answer_supported("2", &["there are 2 cats".into()]));
        assert!(!answer_supported("2", &["in 2023".into()]));
    }
}
