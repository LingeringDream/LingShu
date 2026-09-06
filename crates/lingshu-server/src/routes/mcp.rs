//! 灵枢记忆 MCP —— HTTP+SSE 传输端点 + 一键导入 AI 工具配置。
//!
//! 两个能力：
//! 1. **HTTP+SSE MCP 端点**（`GET /api/v1/mcp/sse` + `POST /api/v1/mcp/messages`）
//!    经典 SSE 传输。AI 工具通过 HTTP 连接灵枢，复用与 stdio 版完全相同的
//!    协议逻辑（`lingshu-mcp-core`）。令牌取自 SSE 连接时的 Authorization 头。
//! 2. **一键导入**（`POST /api/v1/mcp/import`）
//!    把灵枢 MCP 配置写入本机指定 AI 工具的配置文件（Claude Desktop /
//!    Cursor / Cline / VS Code），读取-合并-写回；路径不存在或不可写时
//!    返回 snippet 供前端复制到剪贴板。
//!
//! 注：这些路由**不**进入 OpenAPI `paths(...)`，因此不影响已提交的
//! `openapi.json` 契约门禁。

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use axum::body::Bytes;
use axum::extract::{Json, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing;
use axum::Router;
use futures::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::auth;
use crate::state::AppState;

// ── SSE 会话注册表 ─────────────────────────────────────────────
// 单进程本地服务，用进程级全局表即可。键为 sessionId，值为 SSE 发送端 + 令牌。
struct Session {
    tx: mpsc::UnboundedSender<String>,
    token: String,
}
type SessionMap = Mutex<HashMap<String, Session>>;
static SESSIONS: OnceLock<SessionMap> = OnceLock::new();

fn sessions() -> &'static SessionMap {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/mcp/sse", routing::get(mcp_sse))
        .route("/api/v1/mcp/messages", routing::post(mcp_messages))
        .route("/api/v1/mcp/import", routing::post(mcp_import))
}

// ── SSE 传输 ───────────────────────────────────────────────────
async fn mcp_sse(
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let token = extract_bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let session_id = Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    sessions()
        .lock()
        .unwrap()
        .insert(session_id.clone(), Session { tx, token });

    let endpoint = format!("/api/v1/mcp/messages?sessionId={session_id}");
    let init = Event::default().event("endpoint").data(endpoint);

    let sse_stream = stream::once(async move { Ok::<_, Infallible>(init) }).chain(stream::unfold(
        rx,
        |mut rx| async move {
            rx.recv()
                .await
                .map(|msg| (Ok::<_, Infallible>(Event::default().data(msg)), rx))
        },
    ));

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

async fn mcp_messages(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    body: Bytes,
) -> StatusCode {
    let session_id = match params.get("sessionId") {
        Some(id) => id.clone(),
        None => return StatusCode::BAD_REQUEST,
    };
    let frame: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    // 令牌在 SSE 建连时已校验并随会话保存，这里直接用。
    let token = {
        let guard = sessions().lock().unwrap();
        match guard.get(&session_id) {
            Some(s) => s.token.clone(),
            None => return StatusCode::NOT_FOUND,
        }
    };

    let base_url = format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    );
    let core = lingshu_mcp_core::McpCore::new(&base_url, token);

    if let Some(response) = core.handle_frame(&frame).await {
        if let Ok(text) = serde_json::to_string(&response) {
            let mut guard = sessions().lock().unwrap();
            match guard.get(&session_id) {
                Some(s) if s.tx.send(text).is_ok() => {}
                // 发送失败说明 SSE 已断开，清理会话。
                _ => {
                    guard.remove(&session_id);
                }
            }
        }
    }
    StatusCode::ACCEPTED
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ").map(|s| s.trim().to_string())
}

// ── 一键导入 AI 工具配置 ───────────────────────────────────────
#[derive(Debug, Deserialize)]
struct ImportRequest {
    /// 预设工具 id（claude / cursor / cline / vscode / windsurf / ...）或任意自定义名称
    tool: String,
    /// stdio（默认，最通用）| http（SSE 传输）
    #[serde(default)]
    transport: Option<String>,
    /// http 传输时的后端地址，默认取本机服务地址
    #[serde(default)]
    base_url: Option<String>,
    /// stdio 传输时的可执行文件路径，默认取与后端同目录的 lingshu-mcp
    #[serde(default)]
    binary_path: Option<String>,
    /// 自定义配置文件路径。提供时优先于内置工具映射
    #[serde(default)]
    config_path: Option<String>,
    /// 自定义配置在 JSON 中的父键，例如 "mcp.servers" / "cline.mcpServers"。
    /// 空或不传表示顶层 "mcpServers"
    #[serde(default)]
    parent_key: Option<String>,
}

async fn mcp_import(
    _auth: auth::AuthUser,
    State(state): State<AppState>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    let transport = req.transport.as_deref().unwrap_or("stdio");
    let base_url = req.base_url.unwrap_or_else(|| {
        format!(
            "http://{}:{}",
            state.config.server.host, state.config.server.port
        )
    });

    let (path, parent_key) = resolve_tool_config(
        &req.tool,
        req.config_path.as_deref(),
        req.parent_key.as_deref(),
    )
    .ok_or(StatusCode::BAD_REQUEST)?;

    let entry = if transport == "http" {
        json!({
            "url": format!("{}/api/v1/mcp/sse", base_url),
            "transport": "sse"
        })
    } else {
        let bin = req
            .binary_path
            .map(PathBuf::from)
            .unwrap_or_else(default_mcp_binary);
        json!({
            "command": bin.to_string_lossy(),
            "env": { "LINGSHU_BASE_URL": base_url }
        })
    };

    // 读取（或新建）配置，合并 lingshu-memory 条目，保留其他键。
    let mut obj: Value = if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
            .unwrap_or(json!({}))
    } else {
        json!({})
    };
    if !obj.is_object() {
        obj = json!({});
    }
    let servers_key = parent_key.unwrap_or_else(|| "mcpServers".to_string());
    let obj_map = obj.as_object_mut().expect("obj is an object");
    let servers = obj_map
        .entry(servers_key.clone())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers
        .as_object_mut()
        .expect("servers is an object")
        .insert("lingshu-memory".to_string(), entry.clone());

    let written = if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).is_ok()
            && std::fs::write(&path, serde_json::to_string_pretty(&obj).unwrap()).is_ok()
    } else {
        false
    };

    let snippet = serde_json::to_string_pretty(&json!({
        servers_key: { "lingshu-memory": entry }
    }))
    .unwrap();

    Ok(Json(json!({
        "written": written,
        "path": path.to_string_lossy(),
        "transport": transport,
        "tool": req.tool,
        "snippet": snippet,
    })))
}

/// 解析某 AI 工具的配置文件路径，以及 mcpServers 映射所在的 JSON 键。
/// 返回 `None` 表示无法解析；`Some((path, None))` 表示顶层 `mcpServers`，
/// `Some((path, Some(key)))` 表示嵌套键（如 Cline 的 `cline.mcpServers`）。
fn resolve_tool_config(
    tool: &str,
    custom_path: Option<&str>,
    custom_key: Option<&str>,
) -> Option<(PathBuf, Option<String>)> {
    // 自定义配置优先于任何内置映射
    if let Some(path) = custom_path {
        return Some((PathBuf::from(path), custom_key.map(|k| k.to_string())));
    }

    let top = |p: PathBuf| (p, None);
    let nested = |p: PathBuf, k: String| (p, Some(k));

    // 通用编辑器/IDE 配置目录：VS Code 及其衍生版复用同一片 Code/User 路径
    let vscode_user_dir = || -> Option<PathBuf> {
        let base = if cfg!(windows) {
            win_appdata()?
        } else if cfg!(target_os = "macos") {
            unix_home()?.join("Library").join("Application Support")
        } else {
            unix_home()?.join(".config")
        };
        Some(base.join("Code").join("User"))
    };

    match tool {
        "claude" => {
            let base = if cfg!(windows) {
                win_appdata()?
            } else if cfg!(target_os = "macos") {
                unix_home()?.join("Library").join("Application Support")
            } else {
                unix_home()?.join(".config")
            };
            Some(top(base.join("Claude").join("claude_desktop_config.json")))
        }
        "cursor" => {
            let base = if cfg!(windows) {
                win_home()?
            } else {
                unix_home()?
            };
            Some(top(base.join(".cursor").join("mcp.json")))
        }
        "cline" => {
            let dir = vscode_user_dir()?;
            Some(nested(
                dir.join("globalStorage")
                    .join("saoudrizwan.claude-dev")
                    .join("settings.json"),
                "cline.mcpServers".to_string(),
            ))
        }
        "vscode" => {
            let dir = vscode_user_dir()?;
            Some(nested(dir.join("settings.json"), "mcp.servers".to_string()))
        }
        "windsurf" => {
            let base = if cfg!(windows) {
                win_home()?
            } else {
                unix_home()?
            };
            Some(top(base
                .join(".codeium")
                .join("windsurf")
                .join("mcp_config.json")))
        }
        "trae" => {
            let base = if cfg!(windows) {
                win_home()?
            } else {
                unix_home()?
            };
            Some(top(base.join(".trae").join("mcp.json")))
        }
        "continue" => {
            let base = if cfg!(windows) {
                win_home()?
            } else {
                unix_home()?
            };
            Some(top(base.join(".continue").join("config.json")))
        }
        "roo" | "roo-code" => {
            let dir = vscode_user_dir()?;
            Some(nested(
                dir.join("globalStorage")
                    .join("roovio.roo-code")
                    .join("settings.json"),
                "roo.mcpServers".to_string(),
            ))
        }
        "chatgpt" => {
            let base = if cfg!(windows) {
                win_appdata()?
            } else if cfg!(target_os = "macos") {
                unix_home()?.join("Library").join("Application Support")
            } else {
                unix_home()?.join(".config")
            };
            Some(top(base.join("ChatGPT").join("mcp.json")))
        }
        "workbuddy" => {
            // 灵枢（WorkBuddy）自身的 MCP 配置，可用于把记忆网关快速接入自身或其它
            // 客户端实例。用户级路径：~/.workbuddy/mcp.json
            let base = if cfg!(windows) {
                win_home()?
            } else {
                unix_home()?
            };
            Some(top(base.join(".workbuddy").join("mcp.json")))
        }
        _ => None,
    }
}

/// Windows 的 AppData/Roaming：优先 `%APPDATA%`，缺失时回退到
/// `%USERPROFILE%\AppData\Roaming`（某些精简/交叉环境里 APPDATA 可能未导出）。
fn win_appdata() -> Option<PathBuf> {
    env_path("APPDATA")
        .or_else(|| env_path("USERPROFILE").map(|p| p.join("AppData").join("Roaming")))
}
fn win_home() -> Option<PathBuf> {
    env_path("USERPROFILE")
}
fn unix_home() -> Option<PathBuf> {
    env_path("HOME")
}

/// stdio 传输默认命令：与后端可执行文件同目录下的 lingshu-mcp[.exe]。
fn default_mcp_binary() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let name = if cfg!(windows) {
                "lingshu-mcp.exe"
            } else {
                "lingshu-mcp"
            };
            return dir.join(name);
        }
    }
    PathBuf::from(if cfg!(windows) {
        "lingshu-mcp.exe"
    } else {
        "lingshu-mcp"
    })
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_tool_is_rejected() {
        assert!(resolve_tool_config("not-a-tool", None, None).is_none());
    }

    #[test]
    fn known_tools_resolve_a_path() {
        for tool in [
            "claude",
            "cursor",
            "cline",
            "vscode",
            "windsurf",
            "trae",
            "continue",
            "roo",
            "chatgpt",
            "workbuddy",
        ] {
            assert!(
                resolve_tool_config(tool, None, None).is_some(),
                "tool {tool} should resolve"
            );
        }
    }

    #[test]
    fn cline_and_vscode_use_nested_keys() {
        assert_eq!(
            resolve_tool_config("cline", None, None).unwrap().1,
            Some("cline.mcpServers".to_string())
        );
        assert_eq!(
            resolve_tool_config("vscode", None, None).unwrap().1,
            Some("mcp.servers".to_string())
        );
    }

    #[test]
    fn custom_path_and_key_take_precedence() {
        let (path, key) = resolve_tool_config(
            "ignored",
            Some("/tmp/custom.json"),
            Some("custom.mcpServers"),
        )
        .unwrap();
        assert_eq!(path, PathBuf::from("/tmp/custom.json"));
        assert_eq!(key, Some("custom.mcpServers".to_string()));
    }

    #[test]
    fn custom_path_defaults_to_top_level_mcp_servers() {
        let (_, key) = resolve_tool_config("ignored", Some("/tmp/custom.json"), None).unwrap();
        assert_eq!(key, None);
    }

    #[test]
    fn bearer_extraction_handles_prefix_and_whitespace() {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer   abc123".parse().unwrap(),
        );
        assert_eq!(extract_bearer(&h), Some("abc123".to_string()));
    }
}
