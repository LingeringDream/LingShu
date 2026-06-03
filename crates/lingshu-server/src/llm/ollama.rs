use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct OllamaRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct OllamaResponse {
    pub model: String,
    pub response: String,
    pub done: bool,
}

/// Check if Ollama is available
pub async fn health_check(client: &reqwest::Client, base_url: &str) -> bool {
    client
        .get(format!("{}/api/tags", base_url))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// List available models
pub async fn list_models(client: &reqwest::Client, base_url: &str) -> anyhow::Result<Vec<String>> {
    let resp: serde_json::Value = client
        .get(format!("{}/api/tags", base_url))
        .send()
        .await?
        .json()
        .await?;

    let models = resp["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}
