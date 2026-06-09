use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;
use uuid::Uuid;

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

pub async fn llm_settings_for_user(state: &AppState, user_id: Uuid) -> LlmSettings {
    // 1. In-memory (fast path)
    {
        let map = state.llm_settings.read().await;
        if let Some(s) = map.get(&user_id) {
            return s.clone();
        }
    }
    // 2. Redis cache
    let key = crate::cache::llm_settings_cache_key(user_id);
    if let Some(cached) = crate::cache::get_json::<LlmSettings>(&state.redis, &key).await {
        let mut map = state.llm_settings.write().await;
        map.insert(user_id, cached.clone());
        return cached;
    }
    // 3. PostgreSQL — durable source of truth, survives restarts even with no Redis.
    if let Some(stored) = load_llm_settings_from_db(state, user_id).await {
        // Warm both caches so subsequent reads skip the DB.
        crate::cache::set_json(&state.redis, &key, &stored, None).await;
        let mut map = state.llm_settings.write().await;
        map.insert(user_id, stored.clone());
        return stored;
    }
    // 4. Fallback: config defaults
    state.default_llm_settings()
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
    Ok(Json(llm_settings_for_user(&state, user_id).await))
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
        if p != "ollama" && p != "openai" {
            return Err(AppError::Validation(
                "provider must be 'ollama' or 'openai'".into(),
            ));
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

    // Durable persistence — survives restarts even when Redis is unavailable.
    // This is the whole point: without it, settings live only in memory/Redis
    // and every backend restart forces the user to re-enter their model config.
    let value = serde_json::to_value(&result)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize LLM settings: {e}")))?;
    sqlx::query("UPDATE users SET llm_settings = $1, updated_at = NOW() WHERE id = $2")
        .bind(value)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    // Write-through to Redis (no TTL — config persists until explicitly changed)
    crate::cache::set_json(
        &state.redis,
        &crate::cache::llm_settings_cache_key(user_id),
        &result,
        None,
    )
    .await;

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
