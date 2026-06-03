use sqlx::postgres::PgPoolOptions;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub redis: fred::clients::Client,
    pub http: reqwest::Client,
    pub config: AppConfig,
    pub start_time: std::time::Instant,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> anyhow::Result<Self> {
        // Database pool
        let db = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&config.database.url)
            .await?;

        // Redis client
        let redis_config = fred::types::RedisConfig::from_url(&config.redis.url)?;
        let redis = fred::clients::Client::new(redis_config, None, None, None);
        redis.connect();
        redis.wait_for_connect().await?;

        // HTTP client for LLM and external APIs
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(20)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            db,
            redis,
            http,
            config: config.clone(),
            start_time: std::time::Instant::now(),
        })
    }
}
