# LingShu MVP 完成度对照 · MVP Status vs PRD

> 对照 `CLAUDE.md` 的原始 MVP 范围与 `AI-PersonalAssistant-PRD.md`，记录当前实现状态、
> 已知局限与剩余可选项。当前快照：2026-09-07，版本 1.6.0。原 PRD 是历史设计稿，
> 不代表当前全部平台和云账户能力。
> 图例：✅ 已实现并测试 · ◐ 已实现但有验证缺口/局限 · ⏳ 未做（可选/非 MVP）
>
> **注意：产品定位已更新为「连接所有 AI 的记忆层」，当前客户端、MCP 与 HTTP API 同为核心入口，SDK 属于后续规划。**
> 当前定位、版本边界与已开放能力以 [产品定位与版本体系](./product-positioning.md)、
> [版本权益实施说明](./version-entitlements.md) 和根目录 README 为准。

## 一、MVP 四大件（CLAUDE.md 范围）

| 能力 | 状态 | 说明 |
|------|------|------|
| 跨平台桌面壳 + 桌面宠物 | ✅ | Tauri 2，main 控制面板窗口 + pet 透明置顶可拖拽浮窗；macOS 使用 EventKit，Windows/Linux/Web 对系统日历降级为 ICS，浏览器模式优雅降级 |
| Apple Calendar / ICS | ✅ | 自然语言解析 → 落 PG → L1 逐次确认流；macOS 写入/删除 EventKit，其他平台导出 ICS；macOS 14+ `NSCalendarsFullAccessUsageDescription` 已配置 |
| 云账户、设备会话与 Pro 同步 | ✅ | 可选云账户、浏览器 PKCE 登录、设备撤销、订阅概览、端到端加密记忆同步、冲突历史和下载中心；忘记密码和账户删除仍未开放 |
| SoulLedger | ✅ | 见第三节，七项机制全部落地 |
| 权限分级 L0–L4 | ✅ | 数据模型 + API + 日历处 L1 强制 + 审计日志；**权限设置已持久化至 PostgreSQL**（migration 0022，`users.permissions` JSONB，重启保留） |

## 二、记忆层扩展（1.6.0）

| 能力 | 状态 | 说明 |
|------|------|------|
| MCP 记忆服务 | ✅ | `lingshu-mcp` / `lingshu-mcp-core` 提供 6 个 stdio 工具；桌面包携带二进制，设置页支持为多个客户端写入配置 |
| 并发写入与争议裁决 | ✅ | 事实级事务锁、`context_scope` 隔离、不同主张保留为 `disputed`，人工裁决保留审计记录 |
| 离线评测 | ✅ | `lingshu-eval` 提供自建 v3、LoCoMo 基线、消融和治理诊断；公开集策略尚未接入在线检索 |
| 版本权益 | ✅ | Free / Pro / Team 由云端统一计算；有效 Team 可创建团队工作区，受邀 Free 用户可加入 |
| 自包含桌面包 | ✅ | macOS 与 Windows 包内置 PostgreSQL；Linux 暂用于 CI 与纯后端 |

## 三、SoulLedger 机制（对照设计决策文档）

| 机制 | 状态 | 实现要点 |
|------|------|---------|
| 埋点 telemetry | ✅ | append-only `signal_events`，服务端自动采集 + `/api/v1/signals` 客户端入口，白名单校验；已接入 memory、personality、chat 等核心路径 |
| 记忆留存 | ✅ | 决策树：显式"记住"→高权；dedup 命中→抬权；否则默认 0.5 交给衰减 |
| 遗忘衰减 + provenance 护栏 | ✅ | 指数半衰减纯逻辑 + 后台冷却扫描软删除；被活跃人格快照 / derived 记忆引用的源豁免遗忘；同步从 Qdrant 移除 |
| 人格 | ✅ | 用户滑块（主）+ 显式反馈样本（赞/踩、风格 chip → few-shot 注入）+ 自动演化（降级、24h 冷却 + 保守 clamp） |
| Thought Queue | ✅ | 状态机 pending→shown→accepted/dismissed/snoozed + expired；生命周期守卫 + 防打扰抑制（近 14 天 dismissed）+ 活跃上限 + 每日维护扫描 |
| LLM-as-judge 离线整合 | ✅ | 24h 冷却的离线语义合并，生成 `tier='derived'` 记忆、软降级源、保留来源链 |
| 向量检索 | ✅ | Ollama 嵌入 + Qdrant，带 SQL 回退；保存即建索引，遗忘即删点 |

## 四、Chat 与前端

| 项 | 状态 | 说明 |
|----|------|------|
| 工具调用 Tool Calling | ✅ | Native tool-use 循环，支持 Ollama / OpenAI 兼容 / DeepSeek；带 id + type 以适配 DeepSeek |
| 角色提示词 Role Prompts | ✅ | 用户自定义系统角色提示词（migration 0020），集成进对话工作区，前端含 RolePromptSettings 组件 |
| Markdown 渲染 | ✅ | MessageBubble 使用 react-markdown + remark-gfm 渲染 |
| LLM 设置持久化 | ✅ | migration 0021：模型、max_tokens、context_messages 等存 PostgreSQL，重启保留；前端 ChatSettings 面板可编辑 |
| LLM 错误处理 | ✅ | provider error body 直传前端，不再裸"400 Bad Request" |
| CORS | ✅ | Tauri webview origin 白名单已配置，桌面壳可正常启动 |

## 五、安全与质量

| 项 | 状态 | 说明 |
|----|------|------|
| 网络绑定 | ✅ | 默认 `127.0.0.1`（本地单用户、无密码 owner 令牌，不可暴露公网） |
| 集成令牌加密 | ✅ | AES-256-GCM 静态加密（`TokenCipher` 启动派生一次缓存），响应不回传任何 token |
| SQL 注入 | ✅ | 全部参数化、按 `user_id` 作用域 |
| 路由参数 | ✅ | 已修 axum 0.7 `:param` 语法（此前 `{id}` 致 by-id 路由全 404）+ router 级回归测试 |
| 测试 | ✅ | Rust workspace **408 passed、19 ignored**；前端 **29 个测试文件、132 项通过**；云 Worker **55 项通过**。前端 lint/type-check/build、Worker build 和 OpenAPI 契约均已通过 |
| CI | ✅ | 已配置 Rust lint/test、前端 type-check/build、Docker（GHCR 小写）、sqlx-cli 锁 0.8.x、actions v5 |

## 六、待办 / 验证缺口 / 可选增强

> **全部已完成**（2026-06-10）。以下为已关闭项：

1. ~~**WebSocket 回声桩替换**~~ ✅ 已替换为 `tokio::sync::broadcast` 实时推送。`PetNotification` 通过 WebSocket 向桌面宠物推送日历提醒与 thought 建议。
2. ~~**Thought 跨会话实体计数触发器**~~ ✅ `gather_entity_counts` 汇总日历/项目/任务/记忆/对话数量，注入 `thought_queue_prompt`。
3. ~~**前端 ESLint 接入**~~ ✅ ESLint 9 flat config（`eslint.config.mjs`），`npm run lint` 零错误。
4. ~~**OpenAPI 契约门禁**~~ ✅ 提交 `openapi.json` + `openapi_spec_matches_committed_file` 测试。CI 对比代码与已提交 spec。

> 已完成（历史）：LLM 设置持久化（migration 0021）、权限分级设置持久化（migration 0022）、WebSocket 实时推送、Thought 跨会话触发器、ESLint、OpenAPI 契约。

## 七、结论

CLAUDE.md 定义的 MVP 范围（桌面壳 + 日历 + SoulLedger + 权限分级）在代码层面**已全部落地**，
并已扩展出 Memory Evolution Lab、MCP 记忆服务、并发争议裁决、团队工作区、macOS / Windows 自包含安装包和云账户入口。Rust workspace 当前为 **408 passed、19 ignored**，
前端为 **29 个测试文件、132 项通过**，云 Worker 为 **55 项通过**。
Apple Calendar EventKit 写入/删除 + external_event_id 回写已完整实现。
Chat 侧工具调用、WebSocket 实时推送、角色提示词、Markdown 渲染、
LLM 设置与权限分级的 PostgreSQL 持久化，以及 OpenAPI 契约门禁均已完工。
CI 门禁覆盖 test + fmt + clippy + type-check + lint + build。

**当前工作区版本为 1.6.0（2026-09-07）。**
