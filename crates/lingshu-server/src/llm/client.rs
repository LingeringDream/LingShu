use futures::{FutureExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct LlmClient {
    http: reqwest::Client,
    ollama_url: String,
    api_key: Option<String>,
    api_base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub message: Option<ChatMessage>,
}

impl LlmClient {
    pub fn new(
        http: reqwest::Client,
        ollama_url: &str,
        api_key: Option<String>,
        api_base_url: Option<String>,
    ) -> Self {
        Self {
            http,
            ollama_url: ollama_url.to_string(),
            api_key,
            api_base_url,
        }
    }

    pub async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        let url = format!("{}/api/chat", self.ollama_url);
        let req = ChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
        };

        let resp: ChatResponse = self.http.post(&url).json(&req).send().await?.json().await?;

        Ok(resp.message.map(|m| m.content).unwrap_or_default())
    }

    pub fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> futures::stream::BoxStream<'static, Result<bytes::Bytes, reqwest::Error>> {
        let url = format!("{}/api/chat", self.ollama_url);
        let req = ChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
        };

        self.http
            .post(&url)
            .json(&req)
            .send()
            .into_stream()
            .map_ok(|resp| resp.bytes_stream())
            .try_flatten()
            .boxed()
    }
}
