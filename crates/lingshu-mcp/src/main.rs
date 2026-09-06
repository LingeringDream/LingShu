//! 灵枢记忆 MCP 服务（stdio 传输）
//!
//! 把灵枢「记忆网关」(`/api/v1/memory-gateway/*`) 暴露为标准 MCP 工具，
//! 任何支持 Model Context Protocol 的 AI 工具（Claude Desktop / Cursor /
//! Cline / VS Code / Zed 等）都能把灵枢当作共享长期记忆层来读写。
//!
//! - 仅用 stdio 与 MCP 客户端通信；**所有日志写 stderr**，stdout 只走 JSON-RPC。
//! - 鉴权：优先读 `LINGSHU_TOKEN`；否则启动时调本地 `local-session` 自取 owner 令牌。
//! - 协议逻辑在 `lingshu-mcp-core`，本文件只负责 stdio 帧的读写。

use lingshu_mcp_core::McpCore;
use std::io::{BufRead, BufReader, Write};
use tracing::info;

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let base_url =
        std::env::var("LINGSHU_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let core = match std::env::var("LINGSHU_TOKEN") {
        Ok(t) if !t.is_empty() => McpCore::new(&base_url, t),
        _ => McpCore::from_local_session(&base_url).await?,
    };
    info!("lingshu-mcp 就绪，后端={}", base_url);

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin.lock());
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("非法 JSON-RPC 帧：{}", e);
                continue;
            }
        };
        if let Some(response) = core.handle_frame(&msg).await {
            let out = serde_json::to_string(&response)?;
            writeln!(stdout, "{out}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
