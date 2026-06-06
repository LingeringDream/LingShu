use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub qdrant: QdrantConfig,
    pub llm: LlmConfig,
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

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
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
    pub ollama_url: String,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_base_url: Option<String>,
}

fn default_model() -> String {
    // No default committed to the repo — set LLM_DEFAULT_MODEL in your local .env
    // or add [llm] default_model = "..." to a local config.toml (gitignored).
    String::new()
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    /// Reserved for integration token-at-rest encryption (AES-GCM).
    ///
    /// NOT YET WIRED: the integration write path and an encrypt/decrypt helper
    /// do not exist yet, so setting this key currently protects nothing. It is
    /// intentionally optional to avoid implying a security guarantee that the
    /// code does not provide. Make it required again only once the encryption
    /// helper is implemented and used on the `integrations` write path.
    #[serde(default)]
    pub encryption_key: Option<String>,
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
        "http://localhost:5173".to_string(),
        "http://localhost:8080".to_string(),
        "http://127.0.0.1:5173".to_string(),
        "http://127.0.0.1:8080".to_string(),
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

    /// Config loads from env vars and applies defaults for omitted optional fields.
    /// Uses a single test to avoid env-var interference across parallel tests.
    #[test]
    fn config_loads_from_environment_and_falls_back_to_defaults() {
        // Phase 1: full config — all optional values overridden
        let all_vars: &[(&str, &str)] = &[
            ("SERVER_HOST", "127.0.0.1"),
            ("SERVER_PORT", "9090"),
            (
                "DATABASE_URL",
                "postgres://specified:specified@localhost/specified",
            ),
            ("DATABASE_MAX_CONNECTIONS", "10"),
            ("REDIS_URL", "redis://localhost:6379"),
            ("QDRANT_URL", "http://localhost:6333"),
            ("OLLAMA_URL", "http://localhost:11434"),
            ("JWT_SECRET", "test-jwt-secret"),
            ("ENCRYPTION_KEY", "test-encryption-key"),
        ];
        for (k, v) in all_vars {
            std::env::set_var(k, v);
        }
        let config = AppConfig::load().expect("Full config should load");
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.database.max_connections, 10);
        assert!(config.llm.api_key.is_none());
        assert_eq!(
            config.security.encryption_key.as_deref(),
            Some("test-encryption-key")
        );
        for (k, _) in all_vars {
            std::env::remove_var(k);
        }

        // Phase 2: minimum config — only required vars, defaults kick in
        let min_vars: &[(&str, &str)] = &[
            ("SERVER_HOST", "0.0.0.0"),
            ("SERVER_PORT", "8080"),
            ("DATABASE_URL", "postgres://test:test@localhost:5432/test"),
            ("REDIS_URL", "redis://localhost:6379"),
            ("QDRANT_URL", "http://localhost:6333"),
            ("OLLAMA_URL", "http://localhost:11434"),
            ("JWT_SECRET", "test-jwt-secret"),
            // ENCRYPTION_KEY intentionally omitted — it is now optional.
        ];
        for (k, v) in min_vars {
            std::env::set_var(k, v);
        }
        let config = AppConfig::load().expect("Minimal config should load");
        assert_eq!(config.database.max_connections, 20); // default
                                                         // `config.toml` is gitignored and may provide any local encryption_key
                                                         // value. A separate serde test below covers the missing-field default.
        for (k, _) in min_vars {
            std::env::remove_var(k);
        }
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
}
