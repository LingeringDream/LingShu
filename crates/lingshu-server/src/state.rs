use fred::interfaces::ClientLike;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::crypto::TokenCipher;
use crate::llm::client::LlmClient;
use crate::routes::permissions::PermissionSettings;
use crate::routes::settings::LlmSettings;
use lingshu_vector::search::QdrantClient;

/// Real-time notification sent to connected pet-window clients via WebSocket.
/// Serialised as JSON over the wire.
#[derive(Debug, Clone, Serialize)]
pub struct PetNotification {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_url: Option<String>,
    /// Arbitrary extra payload — used by `mood` events to carry personality
    /// traits so the pet window can adjust its animation parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl PetNotification {
    pub fn new(kind: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            title: title.into(),
            body: body.into(),
            action_url: None,
            data: None,
        }
    }

    /// Send a bare mood change (no extra data).
    pub fn mood(m: &str) -> Self {
        Self {
            kind: "mood".into(),
            title: m.into(),
            body: String::new(),
            action_url: None,
            data: None,
        }
    }

    /// Send a mood change with an attached JSON payload (e.g. personality traits).
    pub fn mood_with_data(m: &str, data: serde_json::Value) -> Self {
        Self {
            kind: "mood".into(),
            title: m.into(),
            body: String::new(),
            action_url: None,
            data: Some(data),
        }
    }
}

/// Type alias so callers can pattern-match on Redis availability.
pub type OptionalRedis = Option<fred::clients::RedisClient>;

/// Type alias so callers can pattern-match on Qdrant availability.
pub type OptionalVector = Option<QdrantClient>;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
    pub start_time: std::time::Instant,
    pub llm: LlmClient,
    /// Redis client — `None` when the URL is empty or the connection failed.
    pub redis: OptionalRedis,
    /// Qdrant vector client — `None` when the URL is empty or the connection failed.
    pub vector: OptionalVector,
    /// Runtime settings changeable via API (model, temperature, etc.).
    /// Backed by memory (HashMap) with optional Redis cache layer.
    pub llm_settings: Arc<RwLock<HashMap<Uuid, LlmSettings>>>,
    /// L0-L4 permission tiers. In-memory, defaults to L0 only.
    pub permissions: Arc<RwLock<HashMap<Uuid, PermissionSettings>>>,
    /// Per-user role-play / custom persona prompt. Cached in memory,
    /// backed by the `users.role_prompt` column in PostgreSQL.
    pub role_prompts: Arc<RwLock<HashMap<Uuid, String>>>,
    /// Pre-initialised AES-256-GCM cipher for integration token encryption.
    /// The expensive 100k-round KDF runs once at startup. `None` when
    /// `ENCRYPTION_KEY` is unconfigured — integration writes that would need to
    /// encrypt a token must then be rejected rather than stored in the clear.
    /// Wrapped in `Arc` because `TokenCipher` wraps `Aes256Gcm` which is not `Clone`.
    pub token_cipher: Option<Arc<TokenCipher>>,
    /// Broadcast channel for pet-window notifications. The handler subscribes
    /// connected WebSocket clients and forwards events (calendar reminders,
    /// thought suggestions, etc.) to the floating desktop pet.
    pub pet_notifications: tokio::sync::broadcast::Sender<PetNotification>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> anyhow::Result<Self> {
        // Shared HTTP client — used by both LlmClient and QdrantClient. Built
        // first because the Qdrant connect below borrows it.
        //
        // NOTE: deliberately NOT using `.timeout()`. reqwest's total timeout
        // caps the WHOLE request including the streaming response body, so a
        // long LLM reply — a cloud model with a high `max_tokens` can stream for
        // several minutes — gets killed mid-sentence at the cap (the "回复到一半
        // 就断了" bug). Instead we bound:
        //   - connect_timeout: time to establish the TCP/TLS connection.
        //   - read_timeout: idle gap between received bytes; it RESETS on every
        //     chunk, so an actively-streaming reply is never cut, while a truly
        //     stalled connection still aborts. 180 s also comfortably covers
        //     local model cold-load latency before the first token (9.6 GB
        //     models can take 30+ s on first request).
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(20)
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(180))
            .build()?;

        // Connect PostgreSQL (required) + Redis + Qdrant (both optional)
        // concurrently. These are independent network round-trips, so doing them
        // together shaves cold-start latency before the server binds :8080 — and
        // the desktop app keeps its window hidden until :8080 is up, so a faster
        // startup makes the window appear sooner.
        let (db, redis, vector) = tokio::join!(
            PgPoolOptions::new()
                .max_connections(config.database.max_connections)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(&config.database.url),
            async {
                // Redis (optional — skip if URL is empty or connection fails)
                if config.redis.url.is_empty() {
                    None
                } else {
                    match try_connect_redis(&config.redis.url).await {
                        Ok(client) => {
                            tracing::info!("Redis connected");
                            Some(client)
                        }
                        Err(e) => {
                            tracing::warn!("Redis unavailable (non-fatal): {}", e);
                            None
                        }
                    }
                }
            },
            async {
                // Qdrant (optional — skip if URL is empty or connection fails)
                if config.qdrant.url.is_empty() {
                    None
                } else {
                    match try_connect_qdrant(&config.qdrant.url, config.llm.embed_dim, &http).await
                    {
                        Ok(client) => {
                            tracing::info!("Qdrant connected");
                            Some(client)
                        }
                        Err(e) => {
                            tracing::warn!("Qdrant unavailable (non-fatal): {}", e);
                            None
                        }
                    }
                }
            },
        );
        // PostgreSQL is required — abort if it is unreachable.
        let db = db?;

        // Pre-derive the encryption cipher once at startup (100k-round KDF)
        let token_cipher: Option<Arc<TokenCipher>> = match config.security.encryption_key.as_deref()
        {
            Some(key) if !key.is_empty() => match TokenCipher::from_key_str(key) {
                Ok(cipher) => {
                    tracing::info!("TokenCipher initialised");
                    Some(Arc::new(cipher))
                }
                Err(e) => {
                    tracing::warn!("TokenCipher initialisation failed (non-fatal): {e}");
                    None
                }
            },
            _ => None,
        };

        Ok(Self {
            db,
            config: config.clone(),
            start_time: std::time::Instant::now(),
            llm: LlmClient::new(
                http.clone(),
                &config.llm.ollama_url,
                config.llm.api_key.clone(),
                config.llm.api_base_url.clone(),
            ),
            redis,
            vector,
            llm_settings: Arc::new(RwLock::new(HashMap::new())),
            permissions: Arc::new(RwLock::new(HashMap::new())),
            role_prompts: Arc::new(RwLock::new(HashMap::new())),
            token_cipher,
            pet_notifications: tokio::sync::broadcast::channel(64).0,
        })
    }

    pub fn default_llm_settings(&self) -> LlmSettings {
        LlmSettings {
            model: self.config.llm.default_model.clone(),
            ..Default::default()
        }
    }
}

async fn try_connect_redis(url: &str) -> anyhow::Result<fred::clients::RedisClient> {
    let redis_config = fred::types::RedisConfig::from_url(url)?;
    let client = fred::clients::RedisClient::new(redis_config, None, None, None);
    client.connect();
    client.wait_for_connect().await?;
    Ok(client)
}

async fn try_connect_qdrant(
    url: &str,
    embed_dim: u64,
    http: &reqwest::Client,
) -> anyhow::Result<QdrantClient> {
    let client = QdrantClient::with_client(url, http.clone());
    // Try to create the memories collection (idempotent — ignore "already exists")
    match client.create_collection("memories", embed_dim).await {
        Ok(()) => {}
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already exists") || msg.contains("409") {
                tracing::debug!("Memories collection already exists in Qdrant");
            } else {
                // Other creation errors are non-fatal — collection may already exist
                tracing::warn!("Qdrant collection creation warning: {e}");
            }
        }
    }
    Ok(client)
}
