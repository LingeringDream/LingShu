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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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
    /// The database id of the persisted assistant message, set only on the
    /// final chunk after the message has been written to the `messages` table.
    /// `None` when the stream ended without a valid assistant response,
    /// when the session_id was not provided, or when persistence failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_message_id: Option<uuid::Uuid>,
}

// ── Tool / Function Calling types ─────────────────────────────────

/// A function tool definition sent to the LLM.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunctionDef,
}

impl ToolDefinition {
    pub fn new(name: &str, description: &str, parameters: serde_json::Value) -> Self {
        Self {
            tool_type: "function".into(),
            function: ToolFunctionDef {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A tool call requested by the LLM.
///
/// OpenAI-compatible providers (OpenAI, DeepSeek, …) require `id` and `type`
/// on every tool call — both in the response they send and in the assistant
/// message echoed back on the next turn. Ollama omits them, so both default:
/// `id` to empty (the caller fills a synthetic one before echoing) and `type`
/// to `"function"`. Without these fields the echoed message fails DeepSeek
/// validation with `messages[N]: missing field id`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type", default = "default_tool_call_type")]
    pub tool_type: String,
    pub function: ToolCallFunction,
}

fn default_tool_call_type() -> String {
    "function".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// The result of a non-streaming chat call that may include tool calls.
#[derive(Debug)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

// ── Ollama-specific types ───────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaMsg>,
}

#[derive(Debug, Deserialize)]
struct OllamaMsg {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
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

#[derive(Debug, Deserialize)]
struct OpenAIEmbedResponse {
    data: Vec<OpenAIEmbedData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbedData {
    embedding: Vec<f32>,
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

    /// Create a temporary [`LlmClient`] that overrides API credentials for a
    /// single user/request, reusing the shared HTTP connection pool.
    pub fn with_overrides(
        &self,
        api_key: Option<String>,
        api_base_url: Option<String>,
    ) -> LlmClient {
        LlmClient {
            http: self.http.clone(),
            ollama_url: self.ollama_url.clone(),
            api_key: api_key.or(self.api_key.clone()),
            api_base_url: api_base_url.or(self.api_base_url.clone()),
        }
    }

    // ── Embeddings ─────────────────────────────────────────────

    /// Generate an embedding vector for `text`.
    ///
    /// Ollama path: POST `/api/embeddings` → `{embedding: [f32]}`.
    /// OpenAI path:  POST `/v1/embeddings` → `{data: [{embedding: [f32]}]}`.
    pub async fn embed(&self, model: &str, text: &str) -> anyhow::Result<Vec<f32>> {
        if self.is_openai() {
            self.embed_openai(model, text).await
        } else {
            self.embed_ollama(model, text).await
        }
    }

    async fn embed_ollama(&self, model: &str, text: &str) -> anyhow::Result<Vec<f32>> {
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

    async fn embed_openai(&self, model: &str, text: &str) -> anyhow::Result<Vec<f32>> {
        let url = self.openai_embeddings_url();
        let body = serde_json::json!({
            "model": model,
            "input": text,
        });
        let mut http_req = self.http.post(&url).json(&body);
        if let Some(ref key) = self.api_key {
            http_req = http_req.header("Authorization", format!("Bearer {key}"));
        }
        let resp: OpenAIEmbedResponse = http_req.send().await?.error_for_status()?.json().await?;
        resp.data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| anyhow::anyhow!("OpenAI embeddings returned empty data array"))
    }

    fn openai_embeddings_url(&self) -> String {
        let base_url = self
            .api_base_url
            .as_deref()
            .unwrap_or("")
            .trim_end_matches('/');
        let base_url = base_url.strip_suffix("/v1").unwrap_or(base_url);
        format!("{base_url}/v1/embeddings")
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
            tools: None,
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
        let resp: OpenAIChatResponse = require_success(http_req.send().await?)
            .await?
            .json()
            .await?;
        Ok(resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .unwrap_or_default()
            .to_string())
    }

    // ── Non-streaming chat with tools ─────────────────────────

    /// Send a non-streaming chat request with tool definitions and
    /// return both the text content and any tool calls the model requested.
    /// Provider-agnostic: works with Ollama and OpenAI-compatible APIs.
    pub async fn chat_with_tools(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<ChatResponse> {
        if self.is_openai() {
            self.chat_with_tools_openai(model, messages, options, tools)
                .await
        } else {
            self.chat_with_tools_ollama(model, messages, options, tools)
                .await
        }
    }

    async fn chat_with_tools_ollama(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<ChatResponse> {
        let url = format!("{}/api/chat", self.ollama_url);
        let req = OllamaChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            options,
            tools: Some(tools),
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

        let msg = resp.message.unwrap_or(OllamaMsg {
            content: String::new(),
            tool_calls: None,
        });
        Ok(ChatResponse {
            content: msg.content,
            tool_calls: msg.tool_calls.unwrap_or_default(),
        })
    }

    async fn chat_with_tools_openai(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        options: Option<ChatOptions>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<ChatResponse> {
        let url = self.openai_chat_completions_url();
        let req = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "temperature": options.as_ref().and_then(|o| o.temperature),
            "max_tokens": options.as_ref().and_then(|o| o.num_predict),
            "tools": tools,
        });
        let mut http_req = self.http.post(&url).json(&req);
        if let Some(ref key) = self.api_key {
            http_req = http_req.header("Authorization", format!("Bearer {key}"));
        }

        #[derive(Deserialize)]
        struct OpenAIToolResponse {
            choices: Vec<OpenAIToolChoice>,
        }
        #[derive(Deserialize)]
        struct OpenAIToolChoice {
            message: OpenAIToolMsg,
        }
        #[derive(Deserialize)]
        struct OpenAIToolMsg {
            #[serde(default)]
            content: Option<String>,
            #[serde(default)]
            tool_calls: Option<Vec<ToolCall>>,
        }

        let resp: OpenAIToolResponse = require_success(http_req.send().await?)
            .await?
            .json()
            .await?;
        let msg = resp.choices.into_iter().next().map(|c| c.message);
        Ok(ChatResponse {
            content: msg
                .as_ref()
                .and_then(|m| m.content.clone())
                .unwrap_or_default(),
            tool_calls: msg.and_then(|m| m.tool_calls).unwrap_or_default(),
        })
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
            tools: None,
        };
        let byte_stream = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .into_stream()
            .map_ok(|resp| resp.bytes_stream())
            .try_flatten()
            .map_err(anyhow::Error::from)
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
        // Send and check the status BEFORE streaming bytes, so a 4xx body (the
        // real reason — unknown model, max_tokens out of range, bad key, …) is
        // surfaced instead of a bare "400 Bad Request".
        let byte_stream = futures::stream::once(async move {
            let resp = require_success(http_req.send().await?).await?;
            Ok::<_, anyhow::Error>(resp.bytes_stream().map_err(anyhow::Error::from))
        })
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

// ── Error helpers ───────────────────────────────────────────────────

/// Turn a non-2xx response into an error that includes the provider's body.
///
/// OpenAI-compatible APIs (including DeepSeek) put the real reason for a 4xx —
/// unknown model, `max_tokens` out of the allowed range, malformed messages,
/// bad API key — in the JSON body. `reqwest::Response::error_for_status`
/// discards that body and leaves only "400 Bad Request", which is undebuggable.
/// This reads and surfaces it (truncated to keep logs/errors readable).
async fn require_success(resp: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    let snippet: String = body.trim().chars().take(600).collect();
    if snippet.is_empty() {
        Err(anyhow::anyhow!("LLM API error: {status}"))
    } else {
        Err(anyhow::anyhow!("LLM API error {status}: {snippet}"))
    }
}

// ── Stream parsing helpers ──────────────────────────────────────────

type ParseFn = fn(&str) -> Option<ChatChunk>;

/// Buffers bytes from a byte stream and yields `ChatChunk` items via a
/// line-based parser. Both Ollama NDJSON and OpenAI SSE use newline-
/// delimited frames, so the buffering logic is shared.
fn parse_byte_stream(
    byte_stream: futures::stream::BoxStream<'static, Result<bytes::Bytes, anyhow::Error>>,
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
                                assistant_message_id: None,
                            })),
                            (stream, bytes::BytesMut::new(), true),
                        ));
                    }
                    return Some((
                        Ok(ChatChunk {
                            content: String::new(),
                            done: true,
                            assistant_message_id: None,
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
            assistant_message_id: None,
        });
    }
    let content = parsed.message?.content;
    if content.is_empty() {
        return None;
    }
    Some(ChatChunk {
        content,
        done: false,
        assistant_message_id: None,
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
            assistant_message_id: None,
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
    Some(ChatChunk {
        content,
        done,
        assistant_message_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ollama_client(ollama_url: String) -> LlmClient {
        LlmClient::new(
            reqwest::Client::new(),
            &ollama_url,
            None, // no api_key → Ollama-only
            None, // no api_base_url → not OpenAI
        )
    }

    fn openai_client(base_url: String) -> LlmClient {
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

        let client = openai_client(format!("{}/v1", server.uri()));

        let response = client
            .chat("test-model", vec![ChatMessage::user("ping")], None)
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

        let client = openai_client(format!("{}/v1", server.uri()));

        let chunks: Vec<ChatChunk> = client
            .chat_stream("test-model", vec![ChatMessage::user("ping")], None)
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

    /// A streaming 4xx must surface the provider's response body (the real
    /// reason), not a bare status. Regression for DeepSeek "400 Bad Request"
    /// where `error_for_status` hid the explanation.
    #[tokio::test]
    async fn openai_stream_surfaces_error_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": {"message": "Invalid max_tokens value, the valid range is [1, 8192]"}
            })))
            .mount(&server)
            .await;

        let client = openai_client(server.uri());
        let results = client
            .chat_stream("deepseek-chat", vec![ChatMessage::user("ping")], None)
            .collect::<Vec<_>>()
            .await;

        let err = results
            .into_iter()
            .find_map(|r| r.err())
            .expect("a 400 response must yield an error");
        let msg = err.to_string();
        assert!(
            msg.contains("400"),
            "error should include the status: {msg}"
        );
        assert!(
            msg.contains("valid range is [1, 8192]"),
            "error should include the provider body: {msg}"
        );
    }

    /// Same guarantee for the non-streaming path.
    #[tokio::test]
    async fn openai_chat_surfaces_error_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": {"message": "Model Not Exist"}
            })))
            .mount(&server)
            .await;

        let client = openai_client(server.uri());
        let err = client
            .chat("nope-model", vec![ChatMessage::user("ping")], None)
            .await
            .expect_err("a 400 must be an error");
        let msg = err.to_string();
        assert!(
            msg.contains("400") && msg.contains("Model Not Exist"),
            "error should include status and provider body: {msg}"
        );
    }

    // ── Tool-call serialization ──────────────────────────────────

    /// OpenAI/DeepSeek tool calls carry `id` + `type`; both must survive a
    /// deserialize→serialize round trip or the echoed assistant message fails
    /// DeepSeek validation with "missing field id".
    #[test]
    fn tool_call_round_trips_openai_id_and_type() {
        let json = serde_json::json!({
            "id": "call_abc123",
            "type": "function",
            "function": {"name": "create_event", "arguments": "{\"title\":\"x\"}"}
        });
        let tc: ToolCall = serde_json::from_value(json).expect("deserialize openai tool_call");
        assert_eq!(tc.id, "call_abc123");
        assert_eq!(tc.tool_type, "function");

        let out = serde_json::to_value(&tc).expect("serialize");
        assert_eq!(out["id"], "call_abc123");
        assert_eq!(out["type"], "function");
        assert_eq!(out["function"]["name"], "create_event");
    }

    /// Ollama omits `id`/`type`; they must default so deserialization succeeds.
    #[test]
    fn tool_call_defaults_for_ollama_shape() {
        let json = serde_json::json!({
            "function": {"name": "create_event", "arguments": {"title": "x"}}
        });
        let tc: ToolCall = serde_json::from_value(json).expect("deserialize ollama tool_call");
        assert_eq!(tc.id, "");
        assert_eq!(tc.tool_type, "function");
    }

    // ── Embed tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn ollama_embed_returns_vector() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .and(body_json(serde_json::json!({
                "model": "nomic-embed-text",
                "prompt": "hello world",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.1, 0.2, 0.3]
            })))
            .mount(&server)
            .await;

        let client = ollama_client(server.uri());
        let embedding = client
            .embed("nomic-embed-text", "hello world")
            .await
            .expect("Ollama embed should succeed");

        assert_eq!(embedding, vec![0.1_f32, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn openai_embed_returns_vector() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(body_json(serde_json::json!({
                "model": "text-embedding-3-small",
                "input": "hello world",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"embedding": [0.4, 0.5, 0.6]}]
            })))
            .mount(&server)
            .await;

        let client = openai_client(server.uri());
        let embedding = client
            .embed("text-embedding-3-small", "hello world")
            .await
            .expect("OpenAI embed should succeed");

        assert_eq!(embedding, vec![0.4_f32, 0.5, 0.6]);
    }

    #[tokio::test]
    async fn openai_embed_accepts_base_url_that_already_includes_v1() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"embedding": [0.7, 0.8]}]
            })))
            .mount(&server)
            .await;

        let client = openai_client(format!("{}/v1", server.uri()));
        let embedding = client
            .embed("text-embedding-3-small", "test")
            .await
            .expect("OpenAI embed with /v1 already in base URL should work");

        assert_eq!(embedding, vec![0.7_f32, 0.8]);
    }
}
