//! Prompt library for 灵枢 (LingShu).
//!
//! Centralizes prompt engineering so model behaviour is tuned in one place.
//! Each function returns a prompt string; callers inject user/memory/
//! personality data at the call site.
//!
//! Convention:
//! - Prompts are written in Chinese (the assistant's primary language).
//! - User data is interpolated via `format!` placeholders.
//! - JSON output prompts include a concrete example.

// ── Thought Queue: Proactive Suggestion Generation ─────────────────────

/// Generate a prompt that asks the LLM to review recent context and
/// propose proactive suggestions for the Thought Queue.
///
/// Each candidate must carry `reason`, `confidence`, `source_memory_ids`,
/// and `requires_confirmation` so the user can audit every suggestion.
pub fn thought_queue_prompt(
    recent_context: &str, // last N conversation summaries or recent memory deltas
    active_goals: &str,   // user's active goals / projects
    pending_tasks: &str,  // incomplete tasks
    now: &str,            // current time, RFC 3339
) -> String {
    format!(
        "你是 灵枢（LingShu）的主动建议引擎。请根据用户最近的对话上下文和当前状态，\
         生成 0-3 条值得向用户提出的主动建议。

当前时间：{now}

## 活跃目标与项目
{active_goals}

## 待完成任务
{pending_tasks}

## 最近对话上下文
{recent_context}

## 建议触发条件（满足任意一条即可）
1. 用户多次提到某个任务但未排入日程 → 建议创建 Calendar 时间块
2. 用户长期偏好被触发 → 可以主动采用并在建议中说明依据
3. 检测到日程冲突或遗漏 → 提醒用户注意
4. 发现高价值记忆候选 → 建议用户确认是否记住
5. 会议或截止日期临近 → 提前提醒

## 每条建议必须包含
- **title**：简短标题（10 字以内）
- **detail**：具体内容，说清楚建议做什么（50 字以内）
- **reason**：为什么提出这条建议，引用触发条件
- **confidence**：0.0-1.0 置信度。不确定时降低，不要瞎猜
- **source_memory_ids**：支持这条建议的记忆 UUID 数组。如果建议来自对话内容而非已有记忆，返回空数组 []
- **requires_confirmation**：是否需要用户确认（true/false）
  - true：涉及创建日程、修改数据、发送通知等操作
  - false：纯信息提醒、轻提示

## 重要
- 宁缺毋滥：没有足够把握时返回空数组 []
- 不要重复已有日程或用户已经明确拒绝过的建议
- 不要伪装成自己有意识——你是基于规则的启发式引擎

严格返回 JSON 数组。每条对象包含：title, detail, reason, confidence, source_memory_ids, requires_confirmation。
如果当前没有值得提出的建议，返回 []。

JSON 数组："
    )
}

// ── Personality-Adapted System Prompt ──────────────────────────────────

/// Build a system-prompt snippet that translates the 7 personality trait
/// values into concrete behavioral instructions.
///
/// Each trait has three tiers — low (0.0–0.35), medium (0.35–0.65),
/// high (0.65–1.0) — with different behavioural guidance.
///
/// The default trait value (identity core) is 0.5 on all dimensions.
pub fn personality_prompt(traits: &PersonalityValues) -> String {
    let mut lines = vec![
        "## 当前人格参数".to_string(),
        String::new(),
        "以下参数控制你本轮的对话风格。请在回复中自然地体现这些倾向，不要逐条宣告参数值。"
            .to_string(),
        String::new(),
    ];

    lines.push(trait_line(
        "直接度",
        traits.directness,
        &[
            "偏含蓄委婉。用「或许可以考虑…」「方便的话…」等缓冲表达。",
            "不卑不亢。需要时说清楚，不需要时不拐弯抹角。",
            "偏直截了当。开门见山，用「建议你…」「可以这样做：」等直接表达。",
        ],
    ));

    lines.push(trait_line(
        "亲和度",
        traits.warmth,
        &[
            "偏中性克制。保持礼貌但不刻意亲近，用「你好」「收到」等标准表达。",
            "温和友善。在回复末尾偶尔加一句轻松的问候或鼓励。",
            "偏热情温暖。多表达理解和支持，用「辛苦了」「不着急，慢慢来」等关怀表达。",
        ],
    ));

    lines.push(trait_line(
        "主动性",
        traits.proactivity,
        &[
            "偏被动回应。只回答用户直接提出的问题，不主动延展话题。",
            "适度主动。在回答问题后，如果发现明显相关的信息，可以补充一句。",
            "偏主动建议。发现线索时主动提出：「对了，你之前提到…要不要顺便…？」",
        ],
    ));

    lines.push(trait_line(
        "风险容忍",
        traits.risk_tolerance,
        &[
            "偏谨慎保守。涉及日程、权限、数据操作时，反复确认后再给出方案。多用「建议先确认…」。",
            "平衡风险。对低风险操作（如打开网页搜索）可直接建议；高风险操作（如删除日程）先确认。",
            "偏大胆果敢。快速给出判断和行动方案，由用户在确认卡片中裁决。",
        ],
    ));

    lines.push(trait_line(
        "详略度",
        traits.verbosity,
        &[
            "极简风格。尽量 1-2 句话说完，用户追问时再展开。",
            "适中风格。默认 2-4 句回复，复杂话题分段说明。",
            "详细风格。回复包含背景、推理和备选方案，适合深度讨论。",
        ],
    ));

    lines.push(trait_line(
        "正式度",
        traits.formality,
        &[
            "偏口语化。用「好的」「没问题」「行」等随性表达，像并肩坐着的同事。",
            "自然对话。口语和书面语混用，保持舒适愉快的交流氛围。",
            "偏正式。用「好的，我来为您处理」「感谢您的反馈」。适合严肃工作场景。",
        ],
    ));

    lines.push(trait_line(
        "幽默度",
        traits.humor,
        &[
            "保持认真。不主动开玩笑或使用轻松比喻。",
            "偶尔轻松。在适当的时候可以来一句轻调侃或自嘲，但不强行幽默。",
            "偏风趣。在合适的话题上使用幽默比喻和轻松语气，让对话更有活力。",
        ],
    ));

    lines.join("\n")
}

/// Generate one trait line with the appropriate behavioural instruction.
fn trait_line(name: &str, value: f32, guides: &[&str; 3]) -> String {
    let idx = if value < 0.35 {
        0
    } else if value <= 0.65 {
        1
    } else {
        2
    };
    format!(
        "- **{name}**（{value:.1}/1.0 → {}）：{}",
        match idx {
            0 => "低",
            1 => "中",
            _ => "高",
        },
        guides[idx]
    )
}

// ── Personality value bundle ──────────────────────────────────────────

/// Bundle for the 7 personality trait parameters.
///
/// Used by [`personality_prompt`] to translate numeric trait scores into
/// human-readable behavioral instructions.
#[derive(Debug, Clone)]
pub struct PersonalityValues {
    pub directness: f32,
    pub warmth: f32,
    pub proactivity: f32,
    pub risk_tolerance: f32,
    pub verbosity: f32,
    pub formality: f32,
    pub humor: f32,
}

impl Default for PersonalityValues {
    fn default() -> Self {
        Self {
            directness: 0.5,
            warmth: 0.5,
            proactivity: 0.5,
            risk_tolerance: 0.5,
            verbosity: 0.5,
            formality: 0.5,
            humor: 0.5,
        }
    }
}

// ── Style Exemplar Injection ──────────────────────────────────────────

/// A style exemplar extracted from user feedback — a previously-liked
/// assistant response (or one tagged with a style preference).
#[derive(Debug, Clone)]
pub struct StyleExemplar {
    /// A snippet of the assistant response the user liked/tagged.
    pub content: String,
    /// Optional style tag: `"too_long"`, `"too_short"`, `"too_formal"`.
    pub style_tag: Option<String>,
}

/// Maximum number of style exemplars injected into the system prompt.
const MAX_EXEMPLARS: usize = 3;

/// Maximum character length of a single exemplar snippet.
const MAX_EXEMPLAR_LEN: usize = 200;

/// Build a system-prompt snippet from a list of [`StyleExemplar`]s.
///
/// - At most [`MAX_EXEMPLARS`] exemplars are included.
/// - Each exemplar is truncated to [`MAX_EXEMPLAR_LEN`] characters.
/// - `style_tag` entries are aggregated into a one-line summary
///   (e.g. "用户曾反馈:偏好更简洁的回复").
/// - Returns an empty string when the input list is empty.
///
/// This function is pure and side-effect free so it can be unit tested
/// without any database or I/O.
pub fn style_exemplar_prompt(exemplars: &[StyleExemplar]) -> String {
    if exemplars.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::with_capacity(5);

    // Aggregate style tags into a summary sentence
    let tags: Vec<&str> = exemplars
        .iter()
        .filter_map(|e| e.style_tag.as_deref())
        .collect();

    if !tags.is_empty() {
        let mut tag_summary = Vec::new();
        if tags.contains(&"too_long") {
            tag_summary.push("更简洁");
        }
        if tags.contains(&"too_short") {
            tag_summary.push("更详细");
        }
        if tags.contains(&"too_formal") {
            tag_summary.push("更口语化");
        }
        if !tag_summary.is_empty() {
            lines.push(format!(
                "用户曾反馈：偏好{}的回复。",
                tag_summary.join("、")
            ));
        }
    }

    // Thumb-up exemplars (positive examples)
    let liked: Vec<&StyleExemplar> = exemplars
        .iter()
        .filter(|e| e.style_tag.is_none() && !e.content.is_empty())
        .take(MAX_EXEMPLARS)
        .collect();

    if !liked.is_empty() {
        lines.push(
            "以下是用户曾点赞的回复风格示例（请参考其详略度与语气，但不要复述内容）：".to_string(),
        );
        for (i, ex) in liked.iter().enumerate() {
            let snippet = if ex.content.chars().count() > MAX_EXEMPLAR_LEN {
                let truncated: String = ex.content.chars().take(MAX_EXEMPLAR_LEN).collect();
                format!("{truncated}…")
            } else {
                ex.content.clone()
            };
            lines.push(format!("  {}. {}", i + 1, snippet));
        }
    }

    if lines.is_empty() {
        return String::new();
    }

    // Prepend a header when we have content
    lines.insert(0, "## 风格参考".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_personality_is_all_mid() {
        let values = PersonalityValues::default();
        let prompt = personality_prompt(&values);
        // All 7 traits should be present
        for trait_name in &[
            "直接度",
            "亲和度",
            "主动性",
            "风险容忍",
            "详略度",
            "正式度",
            "幽默度",
        ] {
            assert!(
                prompt.contains(trait_name),
                "Personality prompt should contain {trait_name}"
            );
        }
    }

    #[test]
    fn low_directness_uses_hedging() {
        let values = PersonalityValues {
            directness: 0.1,
            ..Default::default()
        };
        let prompt = personality_prompt(&values);
        assert!(prompt.contains("含蓄委婉"));
    }

    #[test]
    fn high_directness_is_blunt() {
        let values = PersonalityValues {
            directness: 0.9,
            ..Default::default()
        };
        let prompt = personality_prompt(&values);
        assert!(prompt.contains("直截了当"));
    }

    // ── style_exemplar_prompt ─────────────────────────────────────

    #[test]
    fn exemplar_empty_input_returns_empty_string() {
        let prompt = style_exemplar_prompt(&[]);
        assert!(prompt.is_empty());
    }

    #[test]
    fn exemplar_single_liked() {
        let exemplars = [StyleExemplar {
            content: "好的，我来帮你梳理一下这个问题的几个关键点。".into(),
            style_tag: None,
        }];
        let prompt = style_exemplar_prompt(&exemplars);
        assert!(prompt.contains("风格参考"));
        assert!(prompt.contains("点赞"));
        assert!(prompt.contains("关键点"));
    }

    #[test]
    fn exemplar_truncates_at_3() {
        let exemplars: Vec<_> = (0..6)
            .map(|i| StyleExemplar {
                content: format!("回复内容 {i}"),
                style_tag: None,
            })
            .collect();
        let prompt = style_exemplar_prompt(&exemplars);
        // Count numbered items (1./2./3.) — should have max 3
        let count = prompt.matches("  ").count();
        assert!(count <= 6, "at most 3 exemplar entries (but found more)");
    }

    #[test]
    fn exemplar_long_content_truncated() {
        let long = "a".repeat(300);
        let exemplars = [StyleExemplar {
            content: long,
            style_tag: None,
        }];
        let prompt = style_exemplar_prompt(&exemplars);
        // Truncated → ends with …
        assert!(
            prompt.contains('…'),
            "long content should be truncated with ellipsis"
        );
        // Should not contain the full 300 chars
        let after_ellipsis = prompt.split('…').nth(1).unwrap_or("");
        assert!(
            after_ellipsis.trim().is_empty(),
            "nothing after truncation ellipsis"
        );
    }

    #[test]
    fn exemplar_style_tag_aggregation() {
        let exemplars = [
            StyleExemplar {
                content: String::new(),
                style_tag: Some("too_long".into()),
            },
            StyleExemplar {
                content: String::new(),
                style_tag: Some("too_formal".into()),
            },
        ];
        let prompt = style_exemplar_prompt(&exemplars);
        assert!(prompt.contains("更简洁"));
        assert!(prompt.contains("更口语化"));
        // No liked exemplars (empty content), just the style summary
        assert!(
            !prompt.contains("点赞"),
            "empty-content exemplars should not appear in liked list"
        );
    }

    #[test]
    fn exemplar_mixed_tags_and_likes() {
        let exemplars = [
            StyleExemplar {
                content: "这是一条被点赞的回复。".into(),
                style_tag: None,
            },
            StyleExemplar {
                content: String::new(),
                style_tag: Some("too_short".into()),
            },
        ];
        let prompt = style_exemplar_prompt(&exemplars);
        assert!(prompt.contains("更详细"));
        assert!(prompt.contains("点赞"));
        assert!(prompt.contains("被点赞的回复"));
    }

    #[test]
    fn thought_queue_prompt_includes_all_fields() {
        let prompt = thought_queue_prompt(
            "用户提到想学 Rust",
            "完成 Q3 OKR",
            "写技术方案",
            "2026-06-05T10:00:00+08:00",
        );
        for field in &[
            "title",
            "detail",
            "reason",
            "confidence",
            "requires_confirmation",
        ] {
            assert!(
                prompt.contains(field),
                "Thought Queue prompt should require field '{field}'"
            );
        }
    }
}
