use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::provider::{built_in_providers, find_built_in};

pub use crate::models::provider::{
    BuiltInProvider, ConnectivityResult, CreateProviderRequest, ProviderModel, ProviderResponse,
    UpdateProviderRequest,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/settings/providers",
            get(list_providers).post(create_provider),
        )
        .route(
            "/api/v1/settings/providers/{id}",
            get(get_provider)
                .patch(update_provider)
                .delete(delete_provider),
        )
        .route(
            "/api/v1/settings/providers/{id}/activate",
            post(activate_provider),
        )
        .route(
            "/api/v1/settings/providers/{id}/test",
            post(test_connectivity),
        )
}

// ── Handlers ───────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/settings/providers",
    responses((status = 200, description = "Built-in + saved providers"), (status = 401))
)]
async fn list_providers(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let built_in = built_in_providers();
    let saved = load_user_providers(&state, user_id).await;
    Ok(Json(serde_json::json!({
        "built_in": built_in,
        "saved": saved,
    })))
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/providers/{id}",
    responses((status = 200, body = ProviderResponse), (status = 401), (status = 404))
)]
async fn get_provider(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProviderResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let row = load_provider_by_id(&state, user_id, id).await?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/providers",
    request_body = CreateProviderRequest,
    responses((status = 201, body = ProviderResponse), (status = 401), (status = 422))
)]
async fn create_provider(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(req): Json<CreateProviderRequest>,
) -> Result<(axum::http::StatusCode, Json<ProviderResponse>), AppError> {
    let user_id = auth::require_user(auth).await?;

    // Validate provider_id
    let built_in = find_built_in(&req.provider_id);
    if built_in.is_none() && req.provider_id != "custom" {
        return Err(AppError::Validation(format!(
            "Unknown provider_id '{}'. Use a built-in ID or 'custom'.",
            req.provider_id
        )));
    }

    if req.display_name.trim().is_empty() {
        return Err(AppError::Validation(
            "display_name must not be empty".into(),
        ));
    }

    // For custom providers, api_base_url is required
    let base_url = if req.provider_id == "custom" {
        req.api_base_url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .ok_or_else(|| {
                AppError::Validation("api_base_url is required for custom providers".into())
            })?
            .trim()
            .trim_end_matches('/')
            .to_string()
    } else if let Some(ref url) = req.api_base_url {
        // Built-in with optional URL override
        url.trim().trim_end_matches('/').to_string()
    } else {
        built_in.as_ref().unwrap().base_url.clone()
    };

    // Resolve models
    let models = req.models.unwrap_or_else(|| {
        built_in
            .as_ref()
            .map(|b| b.default_models.clone())
            .unwrap_or_default()
    });

    // Encrypt API key
    let api_key_encrypted = match req.api_key.filter(|k| !k.trim().is_empty()) {
        Some(key) => Some(encrypt_api_key(&state, &key)?),
        None => None,
    };

    // Deactivate other providers if this one should be active
    if req.activate {
        deactivate_all_providers(&state, user_id).await?;
    }

    let id = Uuid::new_v4();
    let display_name = req.display_name.trim().to_string();
    let models_json = serde_json::to_value(&models)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize models: {e}")))?;

    sqlx::query(
        r#"INSERT INTO llm_providers (id, user_id, provider_id, display_name, api_key_encrypted, api_base_url, models, is_active)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(&req.provider_id)
    .bind(&display_name)
    .bind(&api_key_encrypted)
    .bind(&base_url)
    .bind(&models_json)
    .bind(req.activate)
    .execute(&state.db)
    .await?;

    let row = load_provider_by_id(&state, user_id, id).await?;
    Ok((axum::http::StatusCode::CREATED, Json(row.into())))
}

#[utoipa::path(
    patch,
    path = "/api/v1/settings/providers/{id}",
    request_body = UpdateProviderRequest,
    responses((status = 200, body = ProviderResponse), (status = 401), (status = 404))
)]
async fn update_provider(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let existing = load_provider_by_id(&state, user_id, id).await?;

    let display_name = req
        .display_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(existing.display_name.clone());

    let api_key_encrypted = match req.api_key.filter(|k| !k.trim().is_empty()) {
        Some(key) => Some(encrypt_api_key(&state, &key)?),
        None => existing.api_key_encrypted.clone(),
    };

    let api_base_url = req
        .api_base_url
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .or(existing.api_base_url.clone());

    let models_json = match req.models {
        Some(m) => serde_json::to_value(&m)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize models: {e}")))?,
        None => serde_json::to_value(&existing.models).unwrap_or_default(),
    };

    sqlx::query(
        r#"UPDATE llm_providers
           SET display_name = $1, api_key_encrypted = $2, api_base_url = $3,
               models = $4, updated_at = NOW()
           WHERE id = $5 AND user_id = $6 AND deleted_at IS NULL"#,
    )
    .bind(&display_name)
    .bind(&api_key_encrypted)
    .bind(&api_base_url)
    .bind(&models_json)
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    let row = load_provider_by_id(&state, user_id, id).await?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/providers/{id}",
    responses((status = 200), (status = 401), (status = 404))
)]
async fn delete_provider(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let result = sqlx::query(
        "UPDATE llm_providers SET deleted_at = NOW() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Provider not found".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/providers/{id}/activate",
    responses((status = 200, body = ProviderResponse), (status = 401), (status = 404))
)]
async fn activate_provider(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProviderResponse>, AppError> {
    let user_id = auth::require_user(auth).await?;

    // Verify the provider exists
    let _existing = load_provider_by_id(&state, user_id, id).await?;

    // Deactivate all providers for this user
    deactivate_all_providers(&state, user_id).await?;

    // Activate the target
    sqlx::query(
        "UPDATE llm_providers SET is_active = true, updated_at = NOW() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    // Also update users.llm_settings to reflect the active provider
    sync_llm_settings_from_active_provider(&state, user_id, id).await?;

    let row = load_provider_by_id(&state, user_id, id).await?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/providers/{id}/test",
    responses((status = 200, body = ConnectivityResult), (status = 401), (status = 404))
)]
async fn test_connectivity(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConnectivityResult>, AppError> {
    let user_id = auth::require_user(auth).await?;
    let row = load_provider_by_id(&state, user_id, id).await?;

    let built_in = find_built_in(&row.provider_id);
    let protocol = built_in
        .as_ref()
        .map(|b| b.protocol.as_str())
        .unwrap_or("openai_compatible");

    let api_key = match &row.api_key_encrypted {
        Some(enc) => Some(decrypt_api_key(&state, enc)?),
        None => None,
    };

    let base_url = row
        .api_base_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();

    let model = row
        .models
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|m| m.get("id").or(m.get("name")))
        .and_then(|v| v.as_str())
        .unwrap_or("test");

    let start = std::time::Instant::now();
    let result = match protocol {
        "ollama" => test_ollama(&state, &base_url).await,
        "anthropic" => test_anthropic(&state, &base_url, api_key.as_deref(), model).await,
        _ => test_openai_compatible(&state, &base_url, api_key.as_deref(), model).await,
    };
    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(msg) => Ok(Json(ConnectivityResult {
            ok: true,
            message: msg,
            latency_ms,
        })),
        Err(e) => Ok(Json(ConnectivityResult {
            ok: false,
            message: format!("{e}"),
            latency_ms,
        })),
    }
}

// ── Helpers ────────────────────────────────────────────────────────

async fn load_user_providers(state: &AppState, user_id: Uuid) -> Vec<ProviderResponse> {
    let rows: Vec<crate::models::provider::LlmProviderRow> = sqlx::query_as(
        "SELECT id, user_id, provider_id, display_name, api_key_encrypted, api_base_url, models, is_active, created_at, updated_at
         FROM llm_providers WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    rows.into_iter().map(Into::into).collect()
}

async fn load_provider_by_id(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<crate::models::provider::LlmProviderRow, AppError> {
    sqlx::query_as(
        "SELECT id, user_id, provider_id, display_name, api_key_encrypted, api_base_url, models, is_active, created_at, updated_at
         FROM llm_providers WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Provider not found".into()))
}

async fn deactivate_all_providers(state: &AppState, user_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE llm_providers SET is_active = false, updated_at = NOW() WHERE user_id = $1 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Sync users.llm_settings from the newly activated provider.
async fn sync_llm_settings_from_active_provider(
    state: &AppState,
    user_id: Uuid,
    provider_id: Uuid,
) -> Result<(), AppError> {
    let row = load_provider_by_id(state, user_id, provider_id).await?;

    let model = row
        .models
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|m| m.get("id").or(m.get("name")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let llm_settings = serde_json::json!({
        "provider": row.provider_id,
        "api_base_url": row.api_base_url,
        "model": model,
        "temperature": 0.7,
        "max_tokens": 4096,
        "context_messages": 20,
    });

    sqlx::query("UPDATE users SET llm_settings = $1, updated_at = NOW() WHERE id = $2")
        .bind(&llm_settings)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    // Invalidate in-memory cache
    {
        let mut map = state.llm_settings.write().await;
        map.remove(&user_id);
    }

    Ok(())
}

fn encrypt_api_key(state: &AppState, key: &str) -> Result<Vec<u8>, AppError> {
    let cipher = state.token_cipher.as_ref().ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "ENCRYPTION_KEY not configured — cannot encrypt API key"
        ))
    })?;
    cipher
        .encrypt(key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("encrypt API key: {e}")))
}

fn decrypt_api_key(state: &AppState, encrypted: &[u8]) -> Result<String, AppError> {
    let cipher = state.token_cipher.as_ref().ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "ENCRYPTION_KEY not configured — cannot decrypt API key"
        ))
    })?;
    cipher
        .decrypt(encrypted)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("decrypt API key: {e}")))
}

// ── Connectivity test helpers ──────────────────────────────────────

async fn test_ollama(state: &AppState, base_url: &str) -> Result<String, AppError> {
    let url = format!("{base_url}/api/tags");
    let resp = state
        .llm
        .test_ollama_models(&url)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Ollama connection failed: {e}")))?;
    Ok(format!("Connected. {resp} model(s) available."))
}

async fn test_anthropic(
    state: &AppState,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> Result<String, AppError> {
    let key =
        api_key.ok_or_else(|| AppError::Validation("API key is required for Anthropic".into()))?;
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 10,
        "messages": [{"role": "user", "content": "hi"}]
    });

    let resp = state
        .llm
        .http_client()
        .post(&url)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Request failed: {e}")))?;

    let status = resp.status();
    if status.is_success() {
        Ok(format!("Connected to Anthropic ({model})."))
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(AppError::Internal(anyhow::anyhow!(
            "Anthropic returned {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        )))
    }
}

async fn test_openai_compatible(
    state: &AppState,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> Result<String, AppError> {
    let base = base_url.trim_end_matches('/');
    let base = base.strip_suffix("/v1").unwrap_or(base);
    let url = format!("{base}/v1/chat/completions");

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 5,
        "stream": false,
    });

    let mut req = state.llm.http_client().post(&url).json(&body);
    if let Some(key) = api_key {
        req = req.header("Authorization", format!("Bearer {key}"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Request failed: {e}")))?;

    let status = resp.status();
    if status.is_success() {
        Ok(format!("Connected ({model})."))
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(AppError::Internal(anyhow::anyhow!(
            "API returned {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        )))
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_providers_are_listed() {
        let built_in = built_in_providers();
        assert!(built_in.len() >= 15);
        let ids: Vec<_> = built_in.iter().map(|p| p.provider_id.as_str()).collect();
        assert!(ids.contains(&"ollama"));
        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"anthropic"));
        assert!(ids.contains(&"google"));
        assert!(ids.contains(&"deepseek"));
        assert!(ids.contains(&"openrouter"));
        assert!(ids.contains(&"custom"));
    }

    #[test]
    fn find_built_in_for_custom_returns_none_for_models() {
        let custom = find_built_in("custom").unwrap();
        assert!(custom.default_models.is_empty());
    }

    #[test]
    fn all_built_in_providers_have_base_url() {
        for p in built_in_providers() {
            if p.provider_id != "custom" {
                assert!(
                    !p.base_url.is_empty(),
                    "{} has empty base_url",
                    p.provider_id
                );
            }
        }
    }
}
