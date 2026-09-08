//! 灵枢记忆 MCP 核心逻辑（与传输层无关）。
//!
//! 同时被以下两者复用，避免逻辑分叉：
//! - `lingshu-mcp`（stdio 本地二进制）
//! - `lingshu-server` 的 `routes/mcp.rs`（HTTP+SSE 端点）
//!
//! 协议层（initialize / tools/list / tools/call）与 6 个工具实现都在这里；
//! 对外只暴露 [`McpCore::handle_frame`]，传入一个 JSON-RPC 帧，返回要回写的帧
//! （通知类无 id 时返回 `None`）。

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};

const DEFAULT_AGENT: &str = "lingshu";
const PROTOCOL_VERSION: &str = "2024-11-05";

/// 一个 MCP 会话的协议处理器。持有后端基址、HTTP 客户端与鉴权令牌。
pub struct McpCore {
    pub base_url: String,
    client: reqwest::Client,
    token: String,
}

impl McpCore {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            token: token.into(),
        }
    }

    /// 本地单用户部署：调 local-session 自取 owner 令牌（无需密码，设计如此）。
    pub async fn from_local_session(base_url: &str) -> Result<Self> {
        let url = format!("{base_url}/api/v1/auth/local-session");
        let resp = reqwest::Client::new()
            .post(&url)
            .send()
            .await
            .with_context(|| format!("local-session 请求 {url} 失败"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("local-session 失败：{}", resp.status()));
        }
        let v: Value = resp.json().await.context("解析 local-session 响应失败")?;
        let token = v
            .get("token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("local-session 响应缺少 token"))?;
        Ok(Self::new(base_url, token))
    }

    /// 处理一个 JSON-RPC 帧，返回需要回写的帧（通知类返回 `None`）。
    pub async fn handle_frame(&self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        match method {
            "initialize" => Some(self.initialize(id)),
            "ping" => Some(json!({"jsonrpc":"2.0","id":id,"result":{}})),
            "tools/list" => Some(self.tools_list(id)),
            "tools/call" => {
                let result = self.tools_call(msg).await;
                Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
            }
            _ => id.map(|id| {
                json!({
                    "jsonrpc":"2.0","id":id,
                    "error":{"code":-32601,"message":"Method not found"}
                })
            }),
        }
    }

    fn initialize(&self, id: Option<Value>) -> Value {
        json!({
            "jsonrpc":"2.0","id":id,
            "result":{
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities":{"tools":{}},
                "serverInfo":{"name":"lingshu-memory","version":env!("CARGO_PKG_VERSION")}
            }
        })
    }

    fn tools_list(&self, id: Option<Value>) -> Value {
        let tools = vec![
            json!({
                "name":"lingshu_recall",
                "description":"按语义查询检索灵枢长期记忆（自适应重排，自动过滤过期/冲突记忆）。适合「你还记得…吗」「之前关于 X 的讨论」这类问题。",
                "inputSchema":{
                    "type":"object",
                    "properties":{
                        "query":{"type":"string","description":"自然语言检索词"},
                        "as_of":{"type":"string","format":"date-time","description":"可选有效时间点（RFC3339）；按当前权限读取，旧主张单独标记，不等于历史真相"},
                        "context_scope":{"type":"string","description":"业务作用域，例如 project:lingshu"},
                        "agent_id":{"type":"string","description":"调用方 Agent 标识，默认 lingshu","default":"lingshu"},
                        "mode":{"type":"string","enum":["adaptive","static"],"description":"adaptive=自适应重排（含冲突/过期过滤）；static=传统 RAG 基线","default":"adaptive"},
                        "top_k":{"type":"integer","description":"返回条数","default":5},
                        "layer":{"type":"string","enum":["episode","fact","experience"],"description":"可选：仅检索某一层记忆"},
                        "scene_tags":{"type":"array","items":{"type":"string"},"description":"可选：按场景标签过滤"}
                    },
                    "required":["query"]
                }
            }),
            json!({
                "name":"lingshu_store",
                "description":"写入一条记忆到灵枢。fact 层需提供 fact_key 形成结构化事实（会自动触发冲突治理）。",
                "inputSchema":{
                    "type":"object",
                    "properties":{
                        "content":{"type":"string","description":"记忆内容"},
                        "context_scope":{"type":"string","description":"业务作用域，例如 project:lingshu","default":"default"},
                        "request_id":{"type":"string","description":"幂等键；同一次写入重试必须保持相同键和内容"},
                        "agent_id":{"type":"string","description":"写入方 Agent 标识","default":"lingshu"},
                        "memory_layer":{"type":"string","enum":["episode","fact","experience"],"description":"记忆层","default":"fact"},
                        "memory_scope":{"type":"string","enum":["private","shared","global"],"description":"可见范围：私有/定向共享/全局公开","default":"private"},
                        "memory_type":{"type":"string","description":"记忆类型标签","default":"fact"},
                        "fact_key":{"type":"string","description":"当 layer=fact 时必填，结构化事实的键"},
                        "fact_value":{"type":"string","description":"当 layer=fact 时的取值"},
                        "importance":{"type":"number","description":"重要性 0-1","default":0.7},
                        "scene_tags":{"type":"array","items":{"type":"string"},"description":"场景标签"},
                        "source_type":{"type":"string","description":"来源类型","default":"manual"},
                        "source_trust":{"type":"number","description":"来源可信度 0-1","default":0.8}
                    },
                    "required":["content"]
                }
            }),
            json!({
                "name":"lingshu_grant",
                "description":"把某条记忆授权给另一个 Agent（按记忆粒度做跨 Agent 共享）。",
                "inputSchema":{
                    "type":"object",
                    "properties":{
                        "memory_id":{"type":"string","description":"记忆 UUID"},
                        "owner_agent_id":{"type":"string","description":"记忆所有者 Agent 标识"},
                        "grantee_agent_id":{"type":"string","description":"被授权 Agent 标识"},
                        "can_read":{"type":"boolean","description":"是否可读","default":true},
                        "can_write":{"type":"boolean","description":"是否可写","default":false},
                        "can_share":{"type":"boolean","description":"是否可再授权","default":false}
                    },
                    "required":["memory_id","owner_agent_id","grantee_agent_id"]
                }
            }),
            json!({
                "name":"lingshu_conflicts",
                "description":"列出待人工裁决的记忆冲突（同 fact_key 的新旧事实相互矛盾）。",
                "inputSchema":{"type":"object","properties":{}}
            }),
            json!({
                "name":"lingshu_resolve_conflict",
                "description":"裁决一条记忆冲突：选定胜出方，并可把裁决沉淀为 experience 经验记忆。",
                "inputSchema":{
                    "type":"object",
                    "properties":{
                        "id":{"type":"string","description":"冲突事件 UUID"},
                        "winner":{"type":"string","enum":["incumbent","challenger"],"description":"胜出方：既有事实 / 挑战事实"},
                        "reviewer_agent_id":{"type":"string","description":"裁决方 Agent 标识","default":"lingshu"},
                        "note":{"type":"string","description":"裁决依据说明"},
                        "promote_to_experience":{"type":"boolean","description":"是否沉淀为经验记忆","default":true}
                    },
                    "required":["id","winner"]
                }
            }),
            json!({
                "name":"lingshu_stats",
                "description":"返回记忆网关的全局统计（各层/各空间数量、未决冲突、检索次数等）。",
                "inputSchema":{"type":"object","properties":{}}
            }),
        ];
        json!({
            "jsonrpc":"2.0","id":id,
            "result":{"tools":tools}
        })
    }

    async fn tools_call(&self, msg: &Value) -> Value {
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = match params.get("arguments").cloned().unwrap_or(json!({})) {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        let outcome = match name {
            "lingshu_recall" => self.tool_recall(&args).await,
            "lingshu_store" => self.tool_store(&args).await,
            "lingshu_grant" => self.tool_grant(&args).await,
            "lingshu_conflicts" => self.tool_conflicts().await,
            "lingshu_resolve_conflict" => self.tool_resolve(&args).await,
            "lingshu_stats" => self.tool_stats().await,
            other => Err(anyhow!("未知工具：{other}")),
        };
        match outcome {
            Ok(text) => json!({
                "content":[{"type":"text","text":text}],
                "isError": false
            }),
            Err(e) => json!({
                "content":[{"type":"text","text":format!("错误：{e}")}],
                "isError": true
            }),
        }
    }

    // ── 后端调用 ───────────────────────────────────────────────
    async fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        let url = format!("{}/{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("请求 {url} 失败"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("后端 {path} 返回 {status}：{text}"));
        }
        serde_json::from_str(&text).with_context(|| format!("解析后端返回失败：{path}"))
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        let url = format!("{}/{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .with_context(|| format!("请求 {url} 失败"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("后端 {path} 返回 {status}：{text}"));
        }
        serde_json::from_str(&text).with_context(|| format!("解析后端返回失败：{path}"))
    }

    // ── 工具实现 ──────────────────────────────────────────────
    async fn tool_recall(&self, args: &Map<String, Value>) -> Result<String> {
        let query = arg_str(args, "query").ok_or_else(|| anyhow!("缺少参数 query"))?;
        let agent_id = arg_str(args, "agent_id")
            .unwrap_or(DEFAULT_AGENT)
            .to_string();
        let mode = arg_str(args, "mode").unwrap_or("adaptive").to_string();
        let top_k = arg_u64(args, "top_k").unwrap_or(5) as u32;
        let mut body = json!({"query":query,"agent_id":agent_id,"mode":mode,"top_k":top_k});
        if let Some(v) = arg_str(args, "as_of") {
            body["as_of"] = json!(v);
        }
        if let Some(v) = arg_str(args, "context_scope") {
            body["context_scope"] = json!(v);
        }
        if let Some(v) = arg_str(args, "layer") {
            body["layer"] = json!(v);
        }
        if let Some(v) = args.get("scene_tags").and_then(|v| v.as_array()) {
            body["scene_tags"] = json!(v);
        }
        let r = self.post_json("api/v1/memory-gateway/search", body).await?;
        let results = r
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let disputed = r
            .get("disputed_memories")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let historical = r
            .get("historical_memories")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if results.is_empty() && disputed.is_empty() && historical.is_empty() {
            return Ok(format!("（未检索到与「{query}」相关的记忆，mode={mode}）"));
        }
        let count = results.len();
        let mut out = format!("检索「{query}」（mode={mode}，命中 {count} 条）：\n");
        if let Some(as_of) = r.get("as_of").and_then(Value::as_str) {
            out.push_str(&format!(
                "有效时间点：{as_of}（使用当前权限；不代表当时已知信息的完整快照）\n"
            ));
        }
        for memory in &historical {
            out.push_str(&format!(
                "[historical / superseded，非已确认事实] {}\n",
                memory.get("content").and_then(Value::as_str).unwrap_or("")
            ));
        }
        for item in &results {
            let rank = item.get("rank").and_then(|v| v.as_u64()).unwrap_or(0);
            let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let mem = item.get("memory").unwrap_or(item);
            let content = mem.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let layer = mem
                .get("memory_layer")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let scope = mem
                .get("memory_scope")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            out.push_str(&format!(
                "#{rank} [{score:.3}] ({layer}/{scope}, status={}) {content}\n",
                mem.get("conflict_status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ));
        }
        if !disputed.is_empty() {
            out.push_str("存在未裁决主张，不能作为确定事实使用：\n");
            for memory in disputed {
                out.push_str(&format!(
                    "[disputed] {}\n",
                    memory.get("content").and_then(Value::as_str).unwrap_or("")
                ));
            }
        }
        Ok(out)
    }

    async fn tool_store(&self, args: &Map<String, Value>) -> Result<String> {
        let content = arg_str(args, "content").ok_or_else(|| anyhow!("缺少参数 content"))?;
        let agent_id = arg_str(args, "agent_id")
            .unwrap_or(DEFAULT_AGENT)
            .to_string();
        let layer = arg_str(args, "memory_layer").unwrap_or("fact").to_string();
        let scope = arg_str(args, "memory_scope")
            .unwrap_or("private")
            .to_string();
        let mut body = json!({
            "agent_id": agent_id,
            "memory_layer": layer,
            "memory_scope": scope,
            "content": content
        });
        for key in [
            "context_scope",
            "request_id",
            "memory_type",
            "importance",
            "fact_key",
            "fact_value",
            "source_type",
            "source_trust",
            "occurred_at",
            "valid_from",
            "valid_to",
        ] {
            if let Some(v) = args.get(key) {
                body[key] = v.clone();
            }
        }
        if let Some(v) = args.get("scene_tags").and_then(|v| v.as_array()) {
            body["scene_tags"] = json!(v);
        }
        if let Some(v) = args.get("source_memory_ids").and_then(|v| v.as_array()) {
            body["source_memory_ids"] = json!(v);
        }
        let r = self
            .post_json("api/v1/memory-gateway/memories", body)
            .await?;
        let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let rlayer = r
            .get("memory_layer")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let rscope = r
            .get("memory_scope")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let status = r
            .get("conflict_status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(format!("已记录记忆 {id}（layer={rlayer}，scope={rscope}，status={status}）；disputed 表示尚未成为确定事实"))
    }

    async fn tool_grant(&self, args: &Map<String, Value>) -> Result<String> {
        let memory_id = arg_str(args, "memory_id").ok_or_else(|| anyhow!("缺少参数 memory_id"))?;
        let owner =
            arg_str(args, "owner_agent_id").ok_or_else(|| anyhow!("缺少参数 owner_agent_id"))?;
        let grantee = arg_str(args, "grantee_agent_id")
            .ok_or_else(|| anyhow!("缺少参数 grantee_agent_id"))?;
        let body = json!({
            "owner_agent_id": owner,
            "grantee_agent_id": grantee,
            "can_read": arg_bool(args, "can_read", true),
            "can_write": arg_bool(args, "can_write", false),
            "can_share": arg_bool(args, "can_share", false),
        });
        let path = format!("api/v1/memory-gateway/memories/{memory_id}/grants");
        let r = self.post_json(&path, body).await?;
        let can_r = r.get("can_read").and_then(|v| v.as_bool()).unwrap_or(false);
        Ok(format!(
            "已把记忆 {memory_id} 授权给 Agent「{grantee}」（can_read={can_r}）"
        ))
    }

    async fn tool_conflicts(&self) -> Result<String> {
        let r = self.get_json("api/v1/memory-gateway/conflicts").await?;
        let list = r.as_array().cloned().unwrap_or_default();
        if list.is_empty() {
            return Ok("（没有待裁决的冲突）".into());
        }
        let total = list.len();
        let mut out = format!("待裁决冲突共 {total} 条：\n");
        for c in &list {
            let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let key = c.get("fact_key").and_then(|v| v.as_str()).unwrap_or("?");
            let inc = c
                .get("incumbent_value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cha = c
                .get("challenger_value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let status = c
                .get("review_status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            out.push_str(&format!(
                "- {id} fact_key={key} 现={inc} 挑战={cha} review={status}\n"
            ));
        }
        Ok(out)
    }

    async fn tool_resolve(&self, args: &Map<String, Value>) -> Result<String> {
        let id = arg_str(args, "id").ok_or_else(|| anyhow!("缺少参数 id"))?;
        let winner = arg_str(args, "winner")
            .ok_or_else(|| anyhow!("缺少参数 winner（incumbent/challenger）"))?;
        let reviewer = arg_str(args, "reviewer_agent_id")
            .unwrap_or(DEFAULT_AGENT)
            .to_string();
        let mut body = json!({
            "winner": winner,
            "reviewer_agent_id": reviewer,
            "promote_to_experience": arg_bool(args, "promote_to_experience", true)
        });
        if let Some(v) = args.get("note") {
            body["note"] = v.clone();
        }
        let path = format!("api/v1/memory-gateway/conflicts/{id}/resolve");
        let r = self.post_json(&path, body).await?;
        let w = r
            .get("winner_memory_id")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let l = r
            .get("loser_memory_id")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        Ok(format!("已裁决冲突 {id}：胜出 {w}，淘汰 {l}"))
    }

    async fn tool_stats(&self) -> Result<String> {
        let r = self.get_json("api/v1/memory-gateway/stats").await?;
        Ok(serde_json::to_string_pretty(&r).unwrap_or_else(|_| "{}".into()))
    }
}

// ── 参数辅助 ──────────────────────────────────────────────────
fn arg_str<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}
fn arg_u64(args: &Map<String, Value>, key: &str) -> Option<u64> {
    args.get(key).and_then(|v| v.as_u64())
}
fn arg_bool(args: &Map<String, Value>, key: &str, def: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(def)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_version_is_supported() {
        let core = McpCore::new("http://127.0.0.1:8080", "dummy");
        let init = core.initialize(Some(json!(1)));
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert!(init["result"]["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn unknown_method_with_id_is_method_not_found() {
        let core = McpCore::new("http://127.0.0.1:8080", "dummy");
        let resp = core
            .handle_frame(&json!({"jsonrpc":"2.0","id":7,"method":"bogus"}))
            .await
            .unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn notification_without_id_returns_none() {
        let core = McpCore::new("http://127.0.0.1:8080", "dummy");
        let resp = core
            .handle_frame(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        assert!(resp.is_none());
    }
    #[tokio::test]
    async fn store_and_recall_preserve_conflict_contract() {
        use wiremock::matchers::{body_partial_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/v1/memory-gateway/memories"))
            .and(body_partial_json(json!({"context_scope":"project:test","request_id":"retry-1"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"m1","memory_layer":"fact","memory_scope":"global","conflict_status":"disputed"})))
            .expect(1).mount(&server).await;
        Mock::given(method("POST")).and(path("/api/v1/memory-gateway/search"))
            .and(body_partial_json(json!({"context_scope":"project:test","as_of":"2025-01-01T00:00:00Z"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results":[{"rank":1,"score":0.9,"content":"known fact","memory_layer":"fact","memory_scope":"global"}],
                "disputed_memories":[{"content":"uncertain claim"}],
                "as_of":"2025-01-01T00:00:00Z",
                "historical_memories":[{"content":"old claim"}]
            }))).expect(1).mount(&server).await;
        let core = McpCore::new(server.uri(), "test-token");
        let stored=core.tool_store(json!({"content":"db","fact_key":"db","fact_value":"pg","context_scope":"project:test","request_id":"retry-1"}).as_object().unwrap()).await.unwrap();
        assert!(stored.contains("disputed"));
        let recalled = core
            .tool_recall(
                json!({"query":"db","context_scope":"project:test","as_of":"2025-01-01T00:00:00Z"})
                    .as_object()
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(recalled.contains("known fact"));
        assert!(recalled.contains("[disputed] uncertain claim"));
        assert!(recalled.contains("[historical / superseded，非已确认事实] old claim"));
        assert!(recalled.contains("2025-01-01T00:00:00Z"));
    }
}
