use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub qdrant: QdrantConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub cors: CorsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}
impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
            max_connections: default_max_connections(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RedisConfig {
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QdrantConfig {
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_embed_model")]
    pub embed_model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_base_url: Option<String>,
    #[serde(default = "default_embed_dim")]
    pub embed_dim: u64,
}
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            ollama_url: default_ollama_url(),
            default_model: default_model(),
            embed_model: default_embed_model(),
            api_key: None,
            api_base_url: None,
            embed_dim: default_embed_dim(),
        }
    }
}

fn default_embed_dim() -> u64 {
    768
}

fn default_model() -> String {
    // No default committed to the repo — set LLM_DEFAULT_MODEL in your local .env
    // or add [llm] default_model = "..." to a local config.toml (gitignored).
    String::new()
}

fn default_embed_model() -> String {
    "nomic-embed-text".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    /// Key for integration token-at-rest encryption (AES-256-GCM, see `crate::crypto`).
    #[serde(default)]
    pub encryption_key: Option<String>,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            encryption_key: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    #[serde(default = "default_cors_origins")]
    pub allowed_origins: Vec<String>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: default_cors_origins(),
        }
    }
}

fn default_cors_origins() -> Vec<String> {
    vec![
        // Vite dev server / `tauri dev` (loads the frontend over http)
        "http://localhost:5173".to_string(),
        "http://localhost:8080".to_string(),
        "http://127.0.0.1:5173".to_string(),
        "http://127.0.0.1:8080".to_string(),
        // Bundled Tauri 2 app webview origins. The frontend calls the backend
        // cross-origin (apiBaseUrl() → http://127.0.0.1:8080), so the packaged
        // app's custom-scheme origin must be allowed or the local session POST
        // is blocked by CORS ("本地控制台启动失败").
        //   macOS / Linux / iOS: tauri://localhost
        //   Windows / Android:   http://tauri.localhost
        "tauri://localhost".to_string(),
        "http://tauri.localhost".to_string(),
    ]
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_max_connections() -> u32 {
    20
}
fn default_database_url() -> String {
    "postgres://lingshu:lingshu@localhost:5432/lingshu".to_string()
}
fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_jwt_secret() -> String {
    "lingshu-local-dev".to_string()
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config: AppConfig = Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("APP_").split("__"))
            .merge(Env::raw().filter_map(|key| match key.as_str() {
                "DATABASE_URL" => Some("database.url".into()),
                "DATABASE_MAX_CONNECTIONS" => Some("database.max_connections".into()),
                "REDIS_URL" => Some("redis.url".into()),
                "QDRANT_URL" => Some("qdrant.url".into()),
                "OLLAMA_URL" => Some("llm.ollama_url".into()),
                "LLM_DEFAULT_MODEL" => Some("llm.default_model".into()),
                "LLM_EMBED_MODEL" => Some("llm.embed_model".into()),
                "LLM_EMBED_DIM" => Some("llm.embed_dim".into()),
                "LLM_API_KEY" => Some("llm.api_key".into()),
                "LLM_API_BASE_URL" => Some("llm.api_base_url".into()),
                "SERVER_HOST" => Some("server.host".into()),
                "SERVER_PORT" => Some("server.port".into()),
                "JWT_SECRET" => Some("security.jwt_secret".into()),
                "ENCRYPTION_KEY" => Some("security.encryption_key".into()),
                "CORS_ALLOWED_ORIGINS" => Some("cors.allowed_origins".into()),
                _ => None,
            }))
            .extract()?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::{providers::Toml, Figment};

    /// Config deserialises correctly from a structured TOML source and applies
    /// defaults for omitted optional fields.
    ///
    /// The figment is built entirely in-memory (no `std::env::set_var` /
    /// `remove_var` calls) so this test is safe to run in parallel with any
    /// other test and does not interfere with CI-provided env vars like
    /// `DATABASE_URL` or `REDIS_URL`.
    #[test]
    fn config_loads_from_environment_and_falls_back_to_defaults() {
        // Phase 1: full config — all optional values overridden.
        let config: AppConfig = Figment::new()
            .merge(Toml::string(
                r#"
                [server]
                host = "127.0.0.1"
                port = 9090

                [database]
                url = "postgres://specified:specified@localhost/specified"
                max_connections = 10

                [redis]
                url = "redis://localhost:6379"

                [qdrant]
                url = "http://localhost:6333"

                [llm]
                ollama_url = "http://localhost:11434"
                embed_model = "test-embed-model"

                [security]
                jwt_secret = "test-jwt-secret"
                encryption_key = "test-encryption-key"
                "#,
            ))
            .extract()
            .expect("Full config should load");

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.llm.embed_model, "test-embed-model");
        assert!(config.llm.api_key.is_none());
        assert_eq!(
            config.security.encryption_key.as_deref(),
            Some("test-encryption-key")
        );

        // Phase 2: minimum config — only required fields present, defaults kick in.
        let config: AppConfig = Figment::new()
            .merge(Toml::string(
                r#"
                [server]
                host = "0.0.0.0"
                port = 8080

                [database]
                url = "postgres://test:test@localhost:5432/test"

                [redis]
                url = "redis://localhost:6379"

                [qdrant]
                url = "http://localhost:6333"

                [llm]
                ollama_url = "http://localhost:11434"

                [security]
                jwt_secret = "test-jwt-secret"
                "#,
            ))
            .extract()
            .expect("Minimal config should load");

        assert_eq!(config.database.max_connections, 20); // default
        assert!(config.security.encryption_key.is_none()); // omitted key → None
    }

    #[test]
    fn security_config_allows_missing_encryption_key() {
        let config: SecurityConfig = serde_json::from_value(serde_json::json!({
            "jwt_secret": "test-jwt-secret"
        }))
        .expect("encryption_key should be optional");

        assert_eq!(config.jwt_secret, "test-jwt-secret");
        assert!(config.encryption_key.is_none());
    }

    #[test]
    fn llm_config_defaults_embed_model() {
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "ollama_url": "http://localhost:11434"
        }))
        .expect("embed_model should have a default");

        assert_eq!(config.default_model, "");
        assert_eq!(config.embed_model, "nomic-embed-text");
        assert_eq!(config.embed_dim, 768);
    }
}
