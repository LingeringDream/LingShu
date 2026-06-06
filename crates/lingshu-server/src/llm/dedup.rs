//! Near-duplicate detection for memory content.
//!
//! Pure text similarity — no embedding, no Qdrant, no external dependencies.
//! The [`DEDUP_SIMILARITY_THRESHOLD`] const can be replaced with a cosine
//! similarity threshold when embeddings become available later.

use std::collections::HashSet;

/// Jaccard similarity threshold for near-duplicate detection.
/// Tuned for short Chinese memory sentences (1-3 sentences typical).
/// Can be replaced with a cosine threshold when embeddings arrive.
pub const DEDUP_SIMILARITY_THRESHOLD: f32 = 0.82;

// ── Public API ────────────────────────────────────────────────────

/// Check whether two memory texts are duplicates.
/// Returns `true` when exact normalized texts match, or Jaccard ≥ threshold.
pub fn is_duplicate(a: &str, b: &str, threshold: f32) -> bool {
    let na = normalize_memory_text(a);
    let nb = normalize_memory_text(b);
    if na == nb {
        return true;
    }
    jaccard_similarity(&na, &nb) >= threshold
}

/// Normalize and tokenize, then compute Jaccard similarity.
/// Returns a value in [0.0, 1.0] where 1.0 = identical token sets.
pub fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let ta = tokenize(a);
    let tb = tokenize(b);

    let set_a: HashSet<&str> = ta.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = tb.iter().map(|s| s.as_str()).collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        return 1.0; // both empty
    }

    intersection as f32 / union as f32
}

/// Normalize memory text for comparison: lowercase, collapse whitespace,
/// strip common CJK + ASCII punctuation.
pub fn normalize_memory_text(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());

    for ch in lower.chars() {
        if is_stripped_punct(ch) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }

    // Collapse whitespace
    let collapsed: Vec<&str> = out.split_whitespace().collect();
    collapsed.join(" ")
}

/// Tokenize text: Chinese characters become individual tokens; ASCII words
/// are split on whitespace. Mixed text yields a flat token list.
pub fn tokenize(text: &str) -> Vec<String> {
    let text = normalize_memory_text(text);
    let mut tokens: Vec<String> = Vec::new();
    let mut buf = String::new();

    for ch in text.chars() {
        if is_cjk(ch) {
            // flush pending word buffer
            if !buf.is_empty() {
                tokens.push(buf.clone());
                buf.clear();
            }
            tokens.push(ch.to_string());
        } else if ch.is_whitespace() {
            if !buf.is_empty() {
                tokens.push(buf.clone());
                buf.clear();
            }
        } else {
            // ASCII / digit / other
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        tokens.push(buf);
    }

    tokens
}

// ── Helpers ───────────────────────────────────────────────────────

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{2F800}'..='\u{2FA1F}' // CJK Ext A/B + Compatibility
            | '\u{3000}'..='\u{303F}' // CJK Symbols
            | '\u{FF00}'..='\u{FFEF}' // Halfwidth/Fullwidth
            | '\u{FE30}'..='\u{FE4F}' // CJK Compatibility Forms
    )
}

fn is_stripped_punct(ch: char) -> bool {
    matches!(
        ch,
        // CJK punctuation
        '\u{3002}' // 。
            | '\u{FF0C}' // ，
            | '\u{FF01}' // ！
            | '\u{FF1F}' // ？
            | '\u{FF1B}' // ；
            | '\u{FF1A}' // ：
            | '\u{201C}' // "
            | '\u{201D}' // "
            | '\u{2018}' // '
            | '\u{2019}' // '
            | '\u{FF08}' // （
            | '\u{FF09}' // ）
            | '\u{3010}' // 【
            | '\u{3011}' // 】
            | '\u{300A}' // 《
            | '\u{300B}' // 》
            | '\u{3001}' // 、
            | '\u{2026}' // …
            | '\u{FF5E}' // ～
            | '\u{2016}' // ‖
            | '\u{2014}' // —
            | '\u{2013}' // –
            | '\u{3003}' // 〃
            | '\u{FF5B}' // ｛
            | '\u{FF5D}' // ｝
            | '\u{300C}' // 「
            | '\u{300D}' // 」
            | '\u{300E}' // 『
            | '\u{300F}' // 』
            | '\u{3014}' // 〔
            | '\u{3015}' // 〕
            | '\u{FF3B}' // ［
            | '\u{FF3D}' // ］
            | '\u{FF5F}' // ｟
            | '\u{FF60}' // ｠
    ) || matches!(
        ch,
        // ASCII punctuation
        '.'
            | ','
            | '!'
            | '?'
            | ';'
            | ':'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '\''
            | '"'
            | '`'
            // Dashes & misc
            | '-'
            | '_'
            | '/'
            | '\\'
            | '|'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '+'
            | '='
            | '<'
            | '>'
            | '~'
    )
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercase_and_whitespace() {
        let a = normalize_memory_text("I Like  Rust  ");
        let b = normalize_memory_text("i like rust");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_strips_cjk_punctuation() {
        let a = normalize_memory_text("我喜欢用 Rust，特别是写后端服务。");
        let b = normalize_memory_text("我喜欢用 Rust 特别是写后端服务");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_strips_ascii_punctuation() {
        let a = normalize_memory_text("Hello, world! This is a test.");
        let b = normalize_memory_text("hello world this is a test");
        assert_eq!(a, b);
    }

    #[test]
    fn identical_text_similarity_is_one() {
        let sim = jaccard_similarity("我喜欢安静的环境", "我喜欢安静的环境");
        assert!((sim - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn different_text_similarity_is_low() {
        let sim = jaccard_similarity("我喜欢安静的环境", "明天下午三点开会讨论预算");
        assert!(
            sim < 0.5,
            "very different texts should have low similarity, got {sim}"
        );
    }

    #[test]
    fn near_duplicate_crosses_threshold() {
        // One extra word inserted — Jaccard should stay high enough
        let a = "我们团队有五个人";
        let b = "我们的团队有五个人";
        let sim = jaccard_similarity(a, b);
        assert!(
            sim >= DEDUP_SIMILARITY_THRESHOLD,
            "near-duplicate (addition) should exceed threshold, got {sim}"
        );
    }

    #[test]
    fn exact_duplicate_detected() {
        assert!(is_duplicate(
            "我习惯每天早上喝咖啡",
            "我习惯每天早上喝咖啡",
            DEDUP_SIMILARITY_THRESHOLD
        ));
    }

    #[test]
    fn punctuation_only_diff_detected_as_duplicate() {
        assert!(is_duplicate(
            "我习惯每天早上喝咖啡。",
            "我习惯每天早上喝咖啡",
            DEDUP_SIMILARITY_THRESHOLD
        ));
    }

    #[test]
    fn tokenize_cjk_splits_per_character() {
        let tokens = tokenize("我喜欢Rust");
        // Should yield: 我, 喜, 欢, rust
        assert!(tokens.contains(&"我".to_string()));
        assert!(tokens.contains(&"喜".to_string()));
        assert!(tokens.contains(&"欢".to_string()));
        assert!(tokens.contains(&"rust".to_string()));
    }

    #[test]
    fn tokenize_ascii_splits_by_word() {
        let tokens = tokenize("hello world from rust");
        assert_eq!(tokens.len(), 4);
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
    }

    #[test]
    fn empty_strings_are_similar() {
        let sim = jaccard_similarity("", "");
        assert!((sim - 1.0).abs() < f32::EPSILON);
    }
}
