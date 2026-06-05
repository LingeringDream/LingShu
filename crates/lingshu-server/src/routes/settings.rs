use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::state::AppState;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/settings/llm",
        get(get_llm_settings).patch(update_llm_settings),
    )
}

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSettings {
    /// Ollama model name (e.g. "gemma4:e4b", "qwen2.5:7b")
    pub model: String,
    /// Generation temperature (0.0–2.0)
    pub temperature: f32,
    /// Max output tokens
    pub max_tokens: u32,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            model: String::new(),
            temperature: 0.7,
            max_tokens: 2048,
        }
    }
}

pub async fn llm_settings_for_user(state: &AppState, user_id: Uuid) -> LlmSettings {
    state
        .llm_settings
        .read()
        .await
        .get(&user_id)
        .cloned()
        .unwrap_or_else(|| state.default_llm_settings())
}

/// Partial update — all fields optional. Only provided fields are applied.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LlmSettingsPatch {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
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
        if n == 0 || n > 32768 {
            return Err(AppError::Validation(
                "max_tokens must be between 1 and 32768".into(),
            ));
        }
        settings.max_tokens = n;
    }

    Ok(Json(settings.clone()))
}
