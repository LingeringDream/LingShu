use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

/// A single model entry within a provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    /// Optional display label (falls back to `name`).
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Built-in provider template (code-defined, not DB-stored).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BuiltInProvider {
    pub provider_id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub needs_api_key: bool,
    pub default_models: Vec<ProviderModel>,
}

/// DB row for a user-saved provider configuration.
#[derive(Debug, Clone, FromRow)]
pub struct LlmProviderRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: String,
    pub display_name: String,
    pub api_key_encrypted: Option<Vec<u8>>,
    pub api_base_url: Option<String>,
    pub models: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API response for a saved provider (never exposes encrypted key).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProviderResponse {
    pub id: Uuid,
    pub provider_id: String,
    pub display_name: String,
    pub has_api_key: bool,
    pub api_base_url: Option<String>,
    pub models: Vec<ProviderModel>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<LlmProviderRow> for ProviderResponse {
    fn from(row: LlmProviderRow) -> Self {
        let models: Vec<ProviderModel> = serde_json::from_value(row.models).unwrap_or_default();
        Self {
            id: row.id,
            provider_id: row.provider_id,
            display_name: row.display_name,
            has_api_key: row.api_key_encrypted.is_some(),
            api_base_url: row.api_base_url,
            models,
            is_active: row.is_active,
            created_at: row.created_at,
        }
    }
}

/// Request body for creating/updating a provider configuration.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateProviderRequest {
    /// Built-in provider ID (e.g. "openai", "deepseek") or "custom".
    pub provider_id: String,
    /// User-chosen display name (e.g. "我的 DeepSeek", "公司 Claude").
    pub display_name: String,
    /// API key (plaintext — encrypted before storage).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Base URL override (required for "custom", optional for built-in).
    #[serde(default)]
    pub api_base_url: Option<String>,
    /// Model list. If omitted, uses the built-in default models.
    #[serde(default)]
    pub models: Option<Vec<ProviderModel>>,
    /// Whether to activate this provider immediately.
    #[serde(default)]
    pub activate: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateProviderRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_base_url: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<ProviderModel>>,
}

/// Connectivity test result.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConnectivityResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: u64,
}

// ── Built-in provider catalog ──────────────────────────────────────

pub fn built_in_providers() -> Vec<BuiltInProvider> {
    vec![
        BuiltInProvider {
            provider_id: "ollama".into(),
            name: "Ollama（本地）".into(),
            base_url: "http://localhost:11434".into(),
            protocol: "ollama".into(),
            needs_api_key: false,
            default_models: vec![],
        },
        BuiltInProvider {
            provider_id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "gpt-4o".into(),
                    name: "gpt-4o".into(),
                    display_name: Some("GPT-4o".into()),
                },
                ProviderModel {
                    id: "gpt-4o-mini".into(),
                    name: "gpt-4o-mini".into(),
                    display_name: Some("GPT-4o Mini".into()),
                },
                ProviderModel {
                    id: "gpt-4.1".into(),
                    name: "gpt-4.1".into(),
                    display_name: Some("GPT-4.1".into()),
                },
                ProviderModel {
                    id: "o3".into(),
                    name: "o3".into(),
                    display_name: Some("o3".into()),
                },
                ProviderModel {
                    id: "o4-mini".into(),
                    name: "o4-mini".into(),
                    display_name: Some("o4-mini".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "anthropic".into(),
            name: "Anthropic (Claude)".into(),
            base_url: "https://api.anthropic.com".into(),
            protocol: "anthropic".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "claude-sonnet-4-20250514".into(),
                    name: "claude-sonnet-4-20250514".into(),
                    display_name: Some("Claude Sonnet 4".into()),
                },
                ProviderModel {
                    id: "claude-3-5-haiku-20241022".into(),
                    name: "claude-3-5-haiku-20241022".into(),
                    display_name: Some("Claude 3.5 Haiku".into()),
                },
                ProviderModel {
                    id: "claude-3-5-sonnet-20241022".into(),
                    name: "claude-3-5-sonnet-20241022".into(),
                    display_name: Some("Claude 3.5 Sonnet".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "google".into(),
            name: "Google (Gemini)".into(),
            base_url: "https://generativelanguage.googleapis.com".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "gemini-2.5-pro".into(),
                    name: "gemini-2.5-pro".into(),
                    display_name: Some("Gemini 2.5 Pro".into()),
                },
                ProviderModel {
                    id: "gemini-2.5-flash".into(),
                    name: "gemini-2.5-flash".into(),
                    display_name: Some("Gemini 2.5 Flash".into()),
                },
                ProviderModel {
                    id: "gemini-2.0-flash".into(),
                    name: "gemini-2.0-flash".into(),
                    display_name: Some("Gemini 2.0 Flash".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "deepseek".into(),
            name: "DeepSeek".into(),
            base_url: "https://api.deepseek.com".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "deepseek-chat".into(),
                    name: "deepseek-chat".into(),
                    display_name: Some("DeepSeek Chat (V3)".into()),
                },
                ProviderModel {
                    id: "deepseek-reasoner".into(),
                    name: "deepseek-reasoner".into(),
                    display_name: Some("DeepSeek Reasoner (R1)".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "openrouter".into(),
            name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "anthropic/claude-sonnet-4".into(),
                    name: "anthropic/claude-sonnet-4".into(),
                    display_name: Some("Claude Sonnet 4 (via OR)".into()),
                },
                ProviderModel {
                    id: "openai/gpt-4o".into(),
                    name: "openai/gpt-4o".into(),
                    display_name: Some("GPT-4o (via OR)".into()),
                },
                ProviderModel {
                    id: "google/gemini-2.5-pro".into(),
                    name: "google/gemini-2.5-pro".into(),
                    display_name: Some("Gemini 2.5 Pro (via OR)".into()),
                },
                ProviderModel {
                    id: "deepseek/deepseek-chat".into(),
                    name: "deepseek/deepseek-chat".into(),
                    display_name: Some("DeepSeek Chat (via OR)".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "siliconflow".into(),
            name: "硅基流动 (SiliconFlow)".into(),
            base_url: "https://api.siliconflow.cn".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "Qwen/Qwen2.5-72B-Instruct".into(),
                    name: "Qwen/Qwen2.5-72B-Instruct".into(),
                    display_name: Some("Qwen 2.5 72B".into()),
                },
                ProviderModel {
                    id: "deepseek-ai/DeepSeek-V3".into(),
                    name: "deepseek-ai/DeepSeek-V3".into(),
                    display_name: Some("DeepSeek V3".into()),
                },
                ProviderModel {
                    id: "Pro/deepseek-ai/DeepSeek-R1".into(),
                    name: "Pro/deepseek-ai/DeepSeek-R1".into(),
                    display_name: Some("DeepSeek R1 (Pro)".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "qwen".into(),
            name: "通义千问 (Qwen)".into(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "qwen-max".into(),
                    name: "qwen-max".into(),
                    display_name: Some("Qwen Max".into()),
                },
                ProviderModel {
                    id: "qwen-plus".into(),
                    name: "qwen-plus".into(),
                    display_name: Some("Qwen Plus".into()),
                },
                ProviderModel {
                    id: "qwen-turbo".into(),
                    name: "qwen-turbo".into(),
                    display_name: Some("Qwen Turbo".into()),
                },
                ProviderModel {
                    id: "qwen3-235b-a22b".into(),
                    name: "qwen3-235b-a22b".into(),
                    display_name: Some("Qwen3 235B".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "zhipu".into(),
            name: "智谱 AI (GLM)".into(),
            base_url: "https://open.bigmodel.cn/api/paas".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "glm-4-plus".into(),
                    name: "glm-4-plus".into(),
                    display_name: Some("GLM-4 Plus".into()),
                },
                ProviderModel {
                    id: "glm-4-flash".into(),
                    name: "glm-4-flash".into(),
                    display_name: Some("GLM-4 Flash".into()),
                },
                ProviderModel {
                    id: "glm-4-long".into(),
                    name: "glm-4-long".into(),
                    display_name: Some("GLM-4 Long".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "moonshot".into(),
            name: "Moonshot (Kimi)".into(),
            base_url: "https://api.moonshot.cn".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "moonshot-v1-128k".into(),
                    name: "moonshot-v1-128k".into(),
                    display_name: Some("Moonshot v1 128K".into()),
                },
                ProviderModel {
                    id: "moonshot-v1-32k".into(),
                    name: "moonshot-v1-32k".into(),
                    display_name: Some("Moonshot v1 32K".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "yi".into(),
            name: "零一万物 (Yi)".into(),
            base_url: "https://api.lingyiwanwu.com".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "yi-lightning".into(),
                    name: "yi-lightning".into(),
                    display_name: Some("Yi Lightning".into()),
                },
                ProviderModel {
                    id: "yi-large".into(),
                    name: "yi-large".into(),
                    display_name: Some("Yi Large".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "qianfan".into(),
            name: "百度千帆".into(),
            base_url: "https://qianfan.baidubce.com/v2".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "ernie-4.0-8k".into(),
                    name: "ernie-4.0-8k".into(),
                    display_name: Some("ERNIE 4.0".into()),
                },
                ProviderModel {
                    id: "ernie-3.5-8k".into(),
                    name: "ernie-3.5-8k".into(),
                    display_name: Some("ERNIE 3.5".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "hunyuan".into(),
            name: "腾讯混元".into(),
            base_url: "https://api.hunyuan.cloud.tencent.com/v1".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "hunyuan-turbos-latest".into(),
                    name: "hunyuan-turbos-latest".into(),
                    display_name: Some("混元 Turbo".into()),
                },
                ProviderModel {
                    id: "hunyuan-large-latest".into(),
                    name: "hunyuan-large-latest".into(),
                    display_name: Some("混元 Large".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "doubao".into(),
            name: "字节豆包".into(),
            base_url: "https://ark.cn-beijing.volces.com/api/v3".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "doubao-pro-256k".into(),
                    name: "doubao-pro-256k".into(),
                    display_name: Some("豆包 Pro 256K".into()),
                },
                ProviderModel {
                    id: "doubao-lite-32k".into(),
                    name: "doubao-lite-32k".into(),
                    display_name: Some("豆包 Lite 32K".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "mistral".into(),
            name: "Mistral AI".into(),
            base_url: "https://api.mistral.ai".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "mistral-large-latest".into(),
                    name: "mistral-large-latest".into(),
                    display_name: Some("Mistral Large".into()),
                },
                ProviderModel {
                    id: "mistral-small-latest".into(),
                    name: "mistral-small-latest".into(),
                    display_name: Some("Mistral Small".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "groq".into(),
            name: "Groq".into(),
            base_url: "https://api.groq.com/openai".into(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![
                ProviderModel {
                    id: "llama-3.3-70b-versatile".into(),
                    name: "llama-3.3-70b-versatile".into(),
                    display_name: Some("Llama 3.3 70B".into()),
                },
                ProviderModel {
                    id: "mixtral-8x7b-32768".into(),
                    name: "mixtral-8x7b-32768".into(),
                    display_name: Some("Mixtral 8x7B".into()),
                },
            ],
        },
        BuiltInProvider {
            provider_id: "custom".into(),
            name: "自定义 (OpenAI 兼容)".into(),
            base_url: String::new(),
            protocol: "openai_compatible".into(),
            needs_api_key: true,
            default_models: vec![],
        },
    ]
}

/// Find a built-in provider by ID.
pub fn find_built_in(provider_id: &str) -> Option<BuiltInProvider> {
    built_in_providers()
        .into_iter()
        .find(|p| p.provider_id == provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_providers_list_is_non_empty() {
        let providers = built_in_providers();
        assert!(
            providers.len() >= 10,
            "expected at least 10 built-in providers"
        );
    }

    #[test]
    fn find_built_in_returns_known_providers() {
        assert!(find_built_in("ollama").is_some());
        assert!(find_built_in("openai").is_some());
        assert!(find_built_in("anthropic").is_some());
        assert!(find_built_in("deepseek").is_some());
        assert!(find_built_in("custom").is_some());
        assert!(find_built_in("nonexistent").is_none());
    }

    #[test]
    fn custom_provider_has_no_default_models() {
        let custom = find_built_in("custom").unwrap();
        assert!(custom.default_models.is_empty());
    }

    #[test]
    fn ollama_does_not_require_api_key() {
        let ollama = find_built_in("ollama").unwrap();
        assert!(!ollama.needs_api_key);
    }

    #[test]
    fn all_built_in_providers_have_unique_ids() {
        let providers = built_in_providers();
        let mut ids: Vec<_> = providers.iter().map(|p| &p.provider_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), providers.len());
    }
}
