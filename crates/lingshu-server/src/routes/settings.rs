use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;
use uuid::Uuid;

use crate::models::provider::LlmProviderRow;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/settings/llm",
            get(get_llm_settings).patch(update_llm_settings),
        )
        .route(
            "/api/v1/settings/role-prompt",
            get(get_role_prompt).patch(update_role_prompt),
        )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSettings {
    /// LLM provider: "ollama" (local) or "openai" (cloud / OpenAI-compatible)
    #[serde(default = "default_provider")]
    pub provider: String,
    /// API key for cloud providers. Empty = use server default from env.
    #[serde(default)]
    pub api_key: String,
    /// API base URL for OpenAI-compatible providers (e.g. https://api.openai.com).
    /// Empty = use server default from env.
    #[serde(default)]
    pub api_base_url: String,
    /// Model name (e.g. "gemma4:e4b", "qwen2.5:7b", "gpt-4o")
    pub model: String,
    /// Generation temperature (0.0–2.0)
    pub temperature: f32,
    /// Max output tokens per generation.
    /// Cloud models may support 64 K–128 K+ output tokens; raise this to match.
    pub max_tokens: u32,
    /// Number of recent conversation turns to load as context.
    /// Default 20 is fine for small local models. Large-context cloud models
    /// (e.g. 1 M-token Gemini, 200 K Claude) can handle hundreds of turns —
    /// raise this field to take full advantage of the model's context window.
    #[serde(default = "default_context_messages")]
    pub context_messages: u32,
}

fn default_provider() -> String {
    "ollama".into()
}

fn default_context_messages() -> u32 {
    20
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            api_key: String::new(),
            api_base_url: String::new(),
            model: String::new(),
            temperature: 0.7,
            max_tokens: 4096,
            context_messages: 20,
        }
    }
}

impl LlmSettings {
    /// Return a copy with the API key masked for safe external consumption.
    pub fn masked(&self) -> Self {
        let mut s = self.clone();
        if !s.api_key.is_empty() {
            s.api_key = mask_api_key(&s.api_key);
        }
        s
    }

    /// Whether this provider speaks an OpenAI-compatible HTTP API. Ollama is the
    /// only built-in provider with a native (non-OpenAI) protocol; every other
    /// provider — including `custom` and any unknown id — is OpenAI-compatible.
    pub fn uses_openai_compatible_api(&self) -> bool {
        !provider_uses_ollama_protocol(&self.provider)
    }

    /// Resolve the LLM client for this user by layering the per-user provider's
    /// base URL + API key over the server-default client.
    ///
    /// Every OpenAI-compatible provider (OpenAI, DeepSeek, Qwen, SiliconFlow,
    /// OpenRouter, custom, …) must apply these overrides — not just the built-in
    /// `openai` provider. Otherwise switching to any other cloud provider would
    /// silently fall back to the default Ollama backend and the model switch
    /// would not take effect. Ollama uses the server-default client (its native
    /// `/api/chat` path).
    pub fn resolve_client(
        &self,
        default: &crate::llm::client::LlmClient,
    ) -> crate::llm::client::LlmClient {
        if self.uses_openai_compatible_api() && !self.api_base_url.is_empty() {
            default.with_overrides(
                (!self.api_key.is_empty()).then(|| self.api_key.clone()),
                Some(self.api_base_url.clone()),
            )
        } else {
            default.clone()
        }
    }
}

/// Whether `provider` uses Ollama's native protocol (`/api/chat`). Unknown
/// provider ids are treated as OpenAI-compatible.
pub(crate) fn provider_uses_ollama_protocol(provider: &str) -> bool {
    crate::models::provider::find_built_in(provider)
        .map(|p| p.protocol == "ollama")
        .unwrap_or(false)
}

/// Resolve the LLM client for a request, layering the active provider's stored
/// API key (decrypted on demand) on top of the routing decision in
/// [`LlmSettings::resolve_client`].
///
/// The provider's key lives encrypted in `llm_providers`; we decrypt it per
/// request rather than copying plaintext into `users.llm_settings`. Without
/// this, switching to an authenticated cloud provider would route to the right
/// base URL but send no `Authorization` header.
pub async fn resolve_client_for_user(
    state: &AppState,
    user_id: Uuid,
    settings: &LlmSettings,
) -> crate::llm::client::LlmClient {
    // Ollama (or any provider missing a base URL) needs no key injection.
    if !settings.uses_openai_compatible_api() || settings.api_base_url.is_empty() {
        return settings.resolve_client(&state.llm);
    }

    // Prefer an explicit key from settings; otherwise decrypt the active
    // provider's stored key. `None` lets `with_overrides` fall back to the
    // server-default key (e.g. from env), keeping single-key setups working.
    let api_key = if settings.api_key.is_empty() {
        active_provider_api_key(state, user_id).await
    } else {
        Some(settings.api_key.clone())
    };

    state
        .llm
        .with_overrides(api_key, Some(settings.api_base_url.clone()))
}

/// Decrypt the active provider's stored API key, if present and decryptable.
async fn active_provider_api_key(state: &AppState, user_id: Uuid) -> Option<String> {
    let provider = load_active_provider_from_db(state, user_id).await?;
    let encrypted = provider.api_key_encrypted?;
    let cipher = state.token_cipher.as_ref()?;
    cipher.decrypt(&encrypted).ok()
}

/// Mask an API key: show first 4 and last 4 characters, replace middle with `***`.
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "***".to_string();
    }
    let prefix = &key[..4];
    let suffix = &key[key.len() - 4..];
    format!("{prefix}***{suffix}")
}

pub async fn llm_settings_for_user(state: &AppState, user_id: Uuid) -> LlmSettings {
    // 1. In-memory (fast path)
    {
        let map = state.llm_settings.read().await;
        if let Some(s) = map.get(&user_id) {
            return s.clone();
        }
    }

    // 2. Redis cache. Older builds could leave this out of sync with the
    // active provider row, so reconcile it once before warming in-memory cache.
    let key = crate::cache::llm_settings_cache_key(user_id);
    if let Some(cached) = crate::cache::get_json::<LlmSettings>(&state.redis, &key).await {
        let cached = reconcile_llm_settings_with_active_provider(state, user_id, cached).await;
        let mut map = state.llm_settings.write().await;
        map.insert(user_id, cached.clone());
        return cached;
    }

    // 3. PostgreSQL — durable source of truth, survives restarts even with no Redis.
    if let Some(stored) = load_llm_settings_from_db(state, user_id).await {
        let stored = reconcile_llm_settings_with_active_provider(state, user_id, stored).await;
        // Warm both caches so subsequent reads skip the DB.
        crate::cache::set_json(&state.redis, &key, &stored, None).await;
        let mut map = state.llm_settings.write().await;
        map.insert(user_id, stored.clone());
        return stored;
    }

    // 4. Fallback: config defaults
    let defaults = state.default_llm_settings();
    reconcile_llm_settings_with_active_provider(state, user_id, defaults).await
}

/// Load persisted LLM settings from `users.llm_settings`. The column defaults to
/// `'{}'`, which is not a complete `LlmSettings`; any value that fails to
/// deserialize is treated as "not configured yet" (→ `None`).
async fn load_llm_settings_from_db(state: &AppState, user_id: Uuid) -> Option<LlmSettings> {
    let raw: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT llm_settings FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
    raw.and_then(|v| serde_json::from_value::<LlmSettings>(v).ok())
}

async fn load_active_provider_from_db(state: &AppState, user_id: Uuid) -> Option<LlmProviderRow> {
    sqlx::query_as::<_, LlmProviderRow>(
        r#"SELECT id, user_id, provider_id, display_name, api_key_encrypted, api_base_url, models, is_active, created_at, updated_at
           FROM llm_providers
           WHERE user_id = $1 AND is_active = true AND deleted_at IS NULL
           ORDER BY updated_at DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
}

async fn reconcile_llm_settings_with_active_provider(
    state: &AppState,
    user_id: Uuid,
    settings: LlmSettings,
) -> LlmSettings {
    let Some(active_provider) = load_active_provider_from_db(state, user_id).await else {
        return settings;
    };
    if active_provider.provider_id == settings.provider {
        return settings;
    }

    let repaired =
        llm_settings_from_provider_row_preserving_generation(&active_provider, &settings);
    if persist_llm_settings_for_user(state, user_id, &repaired)
        .await
        .is_ok()
    {
        repaired
    } else {
        settings
    }
}

pub(crate) async fn persist_llm_settings_for_user(
    state: &AppState,
    user_id: Uuid,
    settings: &LlmSettings,
) -> Result<(), AppError> {
    let value = serde_json::to_value(settings)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize LLM settings: {e}")))?;
    sqlx::query("UPDATE users SET llm_settings = $1, updated_at = NOW() WHERE id = $2")
        .bind(value)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    {
        let mut map = state.llm_settings.write().await;
        map.insert(user_id, settings.clone());
    }
    crate::cache::set_json(
        &state.redis,
        &crate::cache::llm_settings_cache_key(user_id),
        settings,
        None,
    )
    .await;

    Ok(())
}

pub(crate) fn llm_settings_from_provider_row(row: &LlmProviderRow) -> LlmSettings {
    llm_settings_from_provider_row_with_generation(row, 0.7, 4096, 20)
}

fn llm_settings_from_provider_row_preserving_generation(
    row: &LlmProviderRow,
    previous: &LlmSettings,
) -> LlmSettings {
    llm_settings_from_provider_row_with_generation(
        row,
        previous.temperature,
        previous.max_tokens,
        previous.context_messages,
    )
}

fn llm_settings_from_provider_row_with_generation(
    row: &LlmProviderRow,
    temperature: f32,
    max_tokens: u32,
    context_messages: u32,
) -> LlmSettings {
    let model = row
        .models
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|m| m.get("id").or(m.get("name")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    LlmSettings {
        provider: row.provider_id.clone(),
        api_key: String::new(),
        api_base_url: row.api_base_url.clone().unwrap_or_default(),
        model,
        temperature,
        max_tokens,
        context_messages,
    }
}

/// Partial update — all fields optional. Only provided fields are applied.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LlmSettingsPatch {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub context_messages: Option<u32>,
}

// ── Handlers ───────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/settings/llm",
    responses((status = 200, body = LlmSettings), (status = 401))
)]
async fn get_llm_settings(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<LlmSettings>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let settings = llm_settings_for_user(&state, user_id).await;
    Ok(Json(settings.masked()))
}

#[utoipa::path(
    patch,
    path = "/api/v1/settings/llm",
    request_body = LlmSettingsPatch,
    responses((status = 200, body = LlmSettings), (status = 401))
)]
async fn update_llm_settings(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(patch): Json<LlmSettingsPatch>,
) -> Result<Json<LlmSettings>, AppError> {
    let user_id = auth::require_user(auth).await?;

    let mut all_settings = state.llm_settings.write().await;
    let settings = all_settings
        .entry(user_id)
        .or_insert_with(|| state.default_llm_settings());

    if let Some(provider) = patch.provider {
        let p = provider.trim().to_lowercase();
        if p.is_empty() {
            return Err(AppError::Validation("provider must not be empty".into()));
        }
        settings.provider = p;
    }
    if let Some(key) = patch.api_key {
        settings.api_key = key.trim().to_string();
    }
    if let Some(url) = patch.api_base_url {
        settings.api_base_url = url.trim().trim_end_matches('/').to_string();
    }
    if let Some(model) = patch.model {
        if model.trim().is_empty() {
            return Err(AppError::Validation("model must not be empty".into()));
        }
        settings.model = model.trim().to_string();
    }
    if let Some(t) = patch.temperature {
        if !(0.0..=2.0).contains(&t) {
            return Err(AppError::Validation(
                "temperature must be between 0.0 and 2.0".into(),
            ));
        }
        settings.temperature = t;
    }
    if let Some(n) = patch.max_tokens {
        if n == 0 {
            return Err(AppError::Validation("max_tokens must be at least 1".into()));
        }
        settings.max_tokens = n;
    }
    if let Some(n) = patch.context_messages {
        if n == 0 || n > 10_000 {
            return Err(AppError::Validation(
                "context_messages must be between 1 and 10000".into(),
            ));
        }
        settings.context_messages = n;
    }

    let result = settings.clone();
    drop(all_settings);

    persist_llm_settings_for_user(&state, user_id, &result).await?;

    Ok(Json(result))
}

// ── Role-Play Prompt ────────────────────────────────────────────────

/// Load the role-play prompt for a user. Checks the in-memory cache first,
/// then falls back to the `users.role_prompt` column in PostgreSQL.
pub async fn role_prompt_for_user(state: &AppState, user_id: Uuid) -> String {
    // 1. In-memory cache
    {
        let map = state.role_prompts.read().await;
        if let Some(prompt) = map.get(&user_id) {
            return prompt.clone();
        }
    }
    // 2. Database
    let prompt: String =
        sqlx::query_scalar("SELECT role_prompt FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
    // Populate cache
    {
        let mut map = state.role_prompts.write().await;
        map.insert(user_id, prompt.clone());
    }
    prompt
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RolePromptPatch {
    pub role_prompt: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RolePromptResponse {
    pub role_prompt: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/role-prompt",
    responses((status = 200, body = RolePromptResponse), (status = 401))
)]
async fn get_role_prompt(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<RolePromptResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let role_prompt = role_prompt_for_user(&state, user_id).await;
    Ok(Json(RolePromptResponse { role_prompt }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/settings/role-prompt",
    request_body = RolePromptPatch,
    responses((status = 200, body = RolePromptResponse), (status = 401))
)]
async fn update_role_prompt(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(patch): Json<RolePromptPatch>,
) -> Result<Json<RolePromptResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let trimmed = patch.role_prompt.trim().to_string();

    // Persist to DB
    sqlx::query("UPDATE users SET role_prompt = $1, updated_at = NOW() WHERE id = $2")
        .bind(&trimmed)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    // Update cache
    {
        let mut map = state.role_prompts.write().await;
        map.insert(user_id, trimmed.clone());
    }

    Ok(Json(RolePromptResponse {
        role_prompt: trimmed,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_settings_default_provider_is_ollama() {
        let s = LlmSettings::default();
        assert_eq!(s.provider, "ollama");
    }

    #[test]
    fn llm_settings_default_temperature() {
        let s = LlmSettings::default();
        assert!((s.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn llm_settings_default_max_tokens() {
        let s = LlmSettings::default();
        assert_eq!(s.max_tokens, 4096);
    }

    #[test]
    fn llm_settings_default_context_messages() {
        let s = LlmSettings::default();
        assert_eq!(s.context_messages, 20);
    }

    #[test]
    fn llm_settings_json_round_trip() {
        let orig = LlmSettings {
            provider: "openai".into(),
            api_key: "sk-test".into(),
            api_base_url: "https://api.deepseek.com".into(),
            model: "deepseek-chat".into(),
            temperature: 1.2,
            max_tokens: 32000,
            context_messages: 50,
        };
        let json = serde_json::to_value(&orig).unwrap();
        let restored: LlmSettings = serde_json::from_value(json).unwrap();
        assert_eq!(restored.provider, "openai");
        assert_eq!(restored.api_key, "sk-test");
        assert_eq!(restored.api_base_url, "https://api.deepseek.com");
        assert_eq!(restored.model, "deepseek-chat");
        assert!((restored.temperature - 1.2).abs() < f32::EPSILON);
        assert_eq!(restored.max_tokens, 32000);
        assert_eq!(restored.context_messages, 50);
    }

    #[test]
    fn llm_settings_patch_partial() {
        // Only provided fields should change
        let patch = LlmSettingsPatch {
            provider: Some("openai".into()),
            api_key: None,
            api_base_url: None,
            model: None,
            temperature: None,
            max_tokens: None,
            context_messages: None,
        };
        assert_eq!(patch.provider.as_deref(), Some("openai"));
        assert!(patch.api_key.is_none());
    }

    #[test]
    fn provider_repair_preserves_generation_controls() {
        let row = LlmProviderRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            provider_id: "ollama".into(),
            display_name: "Ollama（本地）".into(),
            api_key_encrypted: None,
            api_base_url: Some("http://localhost:11434".into()),
            models: serde_json::json!([
                { "id": "gemma4:12b", "name": "gemma4:12b", "display_name": "gemma4:12b" }
            ]),
            is_active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let previous = LlmSettings {
            provider: "openai".into(),
            api_key: String::new(),
            api_base_url: "https://api.deepseek.com".into(),
            model: "deepseek-v4-flash".into(),
            temperature: 1.1,
            max_tokens: 10_000,
            context_messages: 64,
        };

        let repaired = llm_settings_from_provider_row_preserving_generation(&row, &previous);

        assert_eq!(repaired.provider, "ollama");
        assert_eq!(repaired.api_base_url, "http://localhost:11434");
        assert_eq!(repaired.model, "gemma4:12b");
        assert!((repaired.temperature - 1.1).abs() < f32::EPSILON);
        assert_eq!(repaired.max_tokens, 10_000);
        assert_eq!(repaired.context_messages, 64);
    }

    fn default_ollama_client() -> crate::llm::client::LlmClient {
        crate::llm::client::LlmClient::new(
            reqwest::Client::new(),
            "http://localhost:11434",
            None,
            None,
        )
    }

    fn settings_for(provider: &str, api_base_url: &str) -> LlmSettings {
        LlmSettings {
            provider: provider.into(),
            api_base_url: api_base_url.into(),
            model: "test-model".into(),
            ..Default::default()
        }
    }

    #[test]
    fn only_ollama_uses_native_protocol() {
        assert!(provider_uses_ollama_protocol("ollama"));
        for p in [
            "openai",
            "deepseek",
            "qwen",
            "siliconflow",
            "openrouter",
            "moonshot",
            "zhipu",
            "google",
            "custom",
            "anthropic",
            "some-unknown-provider",
        ] {
            assert!(
                !provider_uses_ollama_protocol(p),
                "{p} should be treated as OpenAI-compatible"
            );
        }
    }

    #[test]
    fn resolve_client_applies_overrides_for_non_openai_cloud_providers() {
        // Regression: switching to any OpenAI-compatible cloud provider other
        // than the literal `openai` must route to its base URL, not fall back to
        // the default Ollama client.
        let default = default_ollama_client();
        for provider in ["deepseek", "qwen", "siliconflow", "openrouter", "custom"] {
            let settings = settings_for(provider, "https://api.example.com");
            assert!(
                settings.resolve_client(&default).is_openai(),
                "provider {provider} must use the OpenAI-compatible path"
            );
        }
    }

    #[test]
    fn resolve_client_keeps_ollama_on_native_path() {
        let default = default_ollama_client();
        let settings = settings_for("ollama", "http://localhost:11434");
        assert!(
            !settings.resolve_client(&default).is_openai(),
            "Ollama must keep using its native /api/chat client"
        );
    }

    #[test]
    fn resolve_client_falls_back_without_base_url() {
        // A non-Ollama provider with no base URL can't be overridden; it stays on
        // the default client rather than producing a broken request.
        let default = default_ollama_client();
        let settings = settings_for("deepseek", "");
        assert!(!settings.resolve_client(&default).is_openai());
    }
}
