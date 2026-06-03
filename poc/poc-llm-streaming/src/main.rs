use axum::{routing::post, Json, Router};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
    model: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    content: String,
    time_to_first_token_ms: u64,
    total_time_ms: u64,
    chunks_received: usize,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let ollama_url =
        std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let app = Router::new()
        .route("/chat", post(chat))
        .route("/health", post(|| async { "ok" }))
        .with_state(ollama_url);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("PoC server listening on :3000");
    axum::serve(listener, app).await.unwrap();
}

async fn chat(
    axum::extract::State(ollama_url): axum::extract::State<String>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let model = req.model.unwrap_or_else(|| "qwen2.5:1.5b".to_string());
    let start = Instant::now();

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": req.message}],
        "stream": true
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/chat", ollama_url))
        .json(&body)
        .send()
        .await
        .unwrap();

    let mut stream = resp.bytes_stream();
    let mut content = String::new();
    let mut chunks = 0;
    let mut ttft = 0u64;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                chunks += 1;
                if ttft == 0 {
                    ttft = start.elapsed().as_millis() as u64;
                }
                // Ollama sends NDJSON
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    for line in text.lines() {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                            if let Some(token) = json["response"].as_str() {
                                content.push_str(token);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Stream error: {}", e);
                break;
            }
        }
    }

    let total = start.elapsed().as_millis() as u64;

    Json(ChatResponse {
        content,
        time_to_first_token_ms: ttft,
        total_time_ms: total,
        chunks_received: chunks,
    })
}
