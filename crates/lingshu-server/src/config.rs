use figment::{Figment, providers::{Env, Format, Toml}};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub qdrant: QdrantConfig,
    pub llm: LlmConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QdrantConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub ollama_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_base_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub encryption_key: String,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_max_connections() -> u32 {
    20
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config: AppConfig = Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("APP_").split("__"))
            .merge(Env::raw().map(|key| {
                let mapped = match key.as_str() {
                    "DATABASE_URL" => Some("database.url"),
                    "DATABASE_MAX_CONNECTIONS" => Some("database.max_connections"),
                    "REDIS_URL" => Some("redis.url"),
                    "QDRANT_URL" => Some("qdrant.url"),
                    "OLLAMA_URL" => Some("llm.ollama_url"),
                    "LLM_API_KEY" => Some("llm.api_key"),
                    "LLM_API_BASE_URL" => Some("llm.api_base_url"),
                    "SERVER_HOST" => Some("server.host"),
                    "SERVER_PORT" => Some("server.port"),
                    "JWT_SECRET" => Some("security.jwt_secret"),
                    "ENCRYPTION_KEY" => Some("security.encryption_key"),
                    _ => None,
                };
                mapped.map(|m| figment::Profile::new(m))
            }))
            .extract()?;

        Ok(config)
    }
}
