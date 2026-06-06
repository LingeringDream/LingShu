use fred::interfaces::ClientLike;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::llm::client::LlmClient;
use crate::routes::permissions::PermissionSettings;
use crate::routes::settings::LlmSettings;
use lingshu_vector::search::QdrantClient;

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
    /// Key for integration token-at-rest encryption (AES-256-GCM), derived from
    /// `ENCRYPTION_KEY`. `None` when unconfigured — integration writes that would
    /// need to encrypt a token must then be rejected rather than stored in the clear.
    pub encryption_key: Option<String>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> anyhow::Result<Self> {
        // PostgreSQL (required)
        let db = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&config.database.url)
            .await?;

        // Shared HTTP client — used by both LlmClient and QdrantClient
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(20)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        // Redis (optional — skip if URL is empty or connection fails)
        let redis = if config.redis.url.is_empty() {
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
        };

        // Qdrant (optional — skip if URL is empty or connection fails)
        let vector = if config.qdrant.url.is_empty() {
            None
        } else {
            match try_connect_qdrant(&config.qdrant.url, &http).await {
                Ok(client) => {
                    tracing::info!("Qdrant connected");
                    Some(client)
                }
                Err(e) => {
                    tracing::warn!("Qdrant unavailable (non-fatal): {}", e);
                    None
                }
            }
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
            encryption_key: config.security.encryption_key.clone(),
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

async fn try_connect_qdrant(url: &str, http: &reqwest::Client) -> anyhow::Result<QdrantClient> {
    let client = QdrantClient::with_client(url, http.clone());
    // Try to create the memories collection (idempotent — ignore "already exists")
    match client.create_collection("memories", 768).await {
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
