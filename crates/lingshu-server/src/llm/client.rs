use bytes::Buf;
use futures::{FutureExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

// ── Shared types ────────────────────────────────────────────────────

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
pub struct ChatOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<u32>,
}

/// Unified streaming chunk — provider-agnostic.
#[derive(Debug, Clone, Serialize)]
pub struct ChatChunk {
    pub content: String,
    pub done: bool,
}

// ── Ollama-specific types ───────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

// ── OpenAI-compatible types ─────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    #[serde(default)]
    content: String,
}

// ── Constructor ─────────────────────────────────────────────────────

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

    fn is_openai(&self) -> bool {
        self.api_base_url.is_some()
    }

    // ── Embeddings ─────────────────────────────────────────────

    /// Generate an embedding vector for `text` via Ollama's `/api/embeddings`.
    /// Returns the embedding as `Vec<f32>`. This method does NOT support
    /// OpenAI-compatible providers — local Ollama only.
    pub async fn embed(&self, model: &str, text: &str) -> anyhow::Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.ollama_url);
        let body = serde_json::json!({
            "model": model,
            "prompt": text,
        });
        let resp: OllamaEmbedResponse = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.embedding)
    }

    // ── Non-streaming chat ──────────────────────────────────────

    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
    ) -> anyhow::Result<String> {
        if self.is_openai() {
            self.chat_openai(model, messages, options).await
        } else {
            self.chat_ollama(model, messages, options).await
        }
    }

    async fn chat_ollama(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
    ) -> anyhow::Result<String> {
        let url = format!("{}/api/chat", self.ollama_url);
        let req = OllamaChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            options,
        };
        let resp: OllamaChatResponse = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.message.map(|m| m.content).unwrap_or_default())
    }

    async fn chat_openai(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
    ) -> anyhow::Result<String> {
        let url = self.openai_chat_completions_url();
        let req = OpenAIChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            temperature: options.as_ref().and_then(|o| o.temperature),
            max_tokens: options.as_ref().and_then(|o| o.num_predict),
        };
        let mut http_req = self.http.post(&url).json(&req);
        if let Some(ref key) = self.api_key {
            http_req = http_req.header("Authorization", format!("Bearer {key}"));
        }
        let resp: OpenAIChatResponse = http_req.send().await?.error_for_status()?.json().await?;
        Ok(resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .unwrap_or_default()
            .to_string())
    }

    // ── Streaming chat ──────────────────────────────────────────

    /// Returns a stream of `ChatChunk` items. The caller receives
    /// provider-agnostic chunks — Ollama NDJSON and OpenAI SSE parsing
    /// are handled internally.
    pub fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
    ) -> futures::stream::BoxStream<'static, Result<ChatChunk, anyhow::Error>> {
        if self.is_openai() {
            self.chat_stream_openai(model, messages, options)
        } else {
            self.chat_stream_ollama(model, messages, options)
        }
    }

    fn chat_stream_ollama(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
    ) -> futures::stream::BoxStream<'static, Result<ChatChunk, anyhow::Error>> {
        let url = format!("{}/api/chat", self.ollama_url);
        let req = OllamaChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
            options,
        };
        let byte_stream = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .into_stream()
            .map_ok(|resp| resp.bytes_stream())
            .try_flatten()
            .boxed();
        parse_byte_stream(byte_stream, parse_ollama_line)
    }

    fn chat_stream_openai(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
    ) -> futures::stream::BoxStream<'static, Result<ChatChunk, anyhow::Error>> {
        let url = self.openai_chat_completions_url();
        let req = OpenAIChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
            temperature: options.as_ref().and_then(|o| o.temperature),
            max_tokens: options.as_ref().and_then(|o| o.num_predict),
        };
        let mut http_req = self.http.post(&url).json(&req);
        if let Some(ref key) = self.api_key {
            http_req = http_req.header("Authorization", format!("Bearer {key}"));
        }
        let byte_stream = http_req
            .send()
            .into_stream()
            .map(|result| result.and_then(|resp| resp.error_for_status()))
            .map_ok(|resp| resp.bytes_stream())
            .try_flatten()
            .boxed();
        parse_byte_stream(byte_stream, parse_openai_line)
    }

    fn openai_chat_completions_url(&self) -> String {
        let base_url = self
            .api_base_url
            .as_deref()
            .unwrap_or("")
            .trim_end_matches('/');
        let base_url = base_url.strip_suffix("/v1").unwrap_or(base_url);
        format!("{base_url}/v1/chat/completions")
    }
}

// ── Stream parsing helpers ──────────────────────────────────────────

type ParseFn = fn(&str) -> Option<ChatChunk>;

/// Buffers bytes from a byte stream and yields `ChatChunk` items via a
/// line-based parser. Both Ollama NDJSON and OpenAI SSE use newline-
/// delimited frames, so the buffering logic is shared.
fn parse_byte_stream(
    byte_stream: futures::stream::BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    parse_line: ParseFn,
) -> futures::stream::BoxStream<'static, Result<ChatChunk, anyhow::Error>> {
    let state = (byte_stream, bytes::BytesMut::new(), false);
    futures::stream::unfold(state, move |(mut stream, mut buf, done)| async move {
        if done {
            return None;
        }
        loop {
            // Emit if we have a complete line in the buffer
            if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line_bytes = buf.split_to(pos);
                buf.advance(1); // skip the newline
                let line = String::from_utf8_lossy(&line_bytes);
                if let Some(chunk) = (parse_line)(&line) {
                    let is_done = chunk.done;
                    return Some((Ok(chunk), (stream, buf, is_done)));
                }
                continue;
            }
            // Need more bytes
            match stream.next().await {
                Some(Ok(bytes)) => {
                    buf.extend_from_slice(&bytes);
                    continue;
                }
                Some(Err(e)) => {
                    return Some((
                        Err(anyhow::anyhow!("Stream error: {e}")),
                        (stream, buf, true),
                    ));
                }
                None => {
                    // Flush remaining buffer, then emit terminal chunk
                    if !buf.is_empty() {
                        let line = String::from_utf8_lossy(&buf);
                        let maybe = (parse_line)(&line);
                        return Some((
                            Ok(maybe.unwrap_or(ChatChunk {
                                content: String::new(),
                                done: true,
                            })),
                            (stream, bytes::BytesMut::new(), true),
                        ));
                    }
                    return Some((
                        Ok(ChatChunk {
                            content: String::new(),
                            done: true,
                        }),
                        (stream, buf, true),
                    ));
                }
            }
        }
    })
    .boxed()
}

/// Parse one NDJSON line from Ollama's `/api/chat` streaming response.
fn parse_ollama_line(line: &str) -> Option<ChatChunk> {
    #[derive(Deserialize)]
    struct OllamaLine {
        #[serde(default)]
        message: Option<OllamaMsg>,
        #[serde(default)]
        done: bool,
    }
    #[derive(Deserialize)]
    struct OllamaMsg {
        #[serde(default)]
        content: String,
    }

    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let parsed: OllamaLine = serde_json::from_str(line).ok()?;
    if parsed.done {
        // "done":true frames often carry a final empty message
        let content = parsed
            .message
            .map(|m| m.content)
            .filter(|c| !c.is_empty())
            .unwrap_or_default();
        return Some(ChatChunk {
            content,
            done: true,
        });
    }
    let content = parsed.message?.content;
    if content.is_empty() {
        return None;
    }
    Some(ChatChunk {
        content,
        done: false,
    })
}

/// Parse one SSE line from OpenAI's `/v1/chat/completions` streaming response.
fn parse_openai_line(line: &str) -> Option<ChatChunk> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return None;
    }
    // SSE termination signal
    if line == "data: [DONE]" {
        return Some(ChatChunk {
            content: String::new(),
            done: true,
        });
    }
    let json_str = line.strip_prefix("data: ")?;
    let chunk: OpenAIStreamChunk = serde_json::from_str(json_str).ok()?;
    let choice = chunk.choices.first()?;
    let content = choice.delta.content.clone();
    let done = choice.finish_reason.is_some();
    if content.is_empty() && !done {
        return None;
    }
    Some(ChatChunk { content, done })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for_base_url(base_url: String) -> LlmClient {
        LlmClient::new(
            reqwest::Client::new(),
            "http://ollama.invalid",
            Some("test-key".to_string()),
            Some(base_url),
        )
    }

    #[tokio::test]
    async fn openai_chat_accepts_base_url_that_already_includes_v1() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_json(serde_json::json!({
                "model": "test-model",
                "messages": [{"role": "user", "content": "ping"}],
                "stream": false,
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "pong"}}]
            })))
            .mount(&server)
            .await;

        let client = client_for_base_url(format!("{}/v1", server.uri()));

        let response = client
            .chat(
                "test-model",
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: "ping".to_string(),
                }],
                None,
            )
            .await
            .expect("chat request should use the configured /v1 endpoint once");

        assert_eq!(response, "pong");
    }

    #[tokio::test]
    async fn openai_stream_accepts_base_url_that_already_includes_v1() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"pong\"}}]}\n\n\
                         data: [DONE]\n\n",
                    ),
            )
            .mount(&server)
            .await;

        let client = client_for_base_url(format!("{}/v1", server.uri()));

        let chunks: Vec<ChatChunk> = client
            .chat_stream(
                "test-model",
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: "ping".to_string(),
                }],
                None,
            )
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("stream request should use the configured /v1 endpoint once");

        assert_eq!(
            chunks.first().map(|chunk| chunk.content.as_str()),
            Some("pong")
        );
        assert!(chunks.last().is_some_and(|chunk| chunk.done));
    }
}
