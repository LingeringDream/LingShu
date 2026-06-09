# LingShu MVP 完成度对照 · MVP Status vs PRD

> 对照 `CLAUDE.md` 的 MVP 范围与 `AI-PersonalAssistant-PRD.md`，记录截至当前的实现状态、
> 已知局限与剩余可选项。日期：2026-06-09。
> 图例：✅ 已实现并测试 · ◐ 已实现但有验证缺口/局限 · ⏳ 未做（可选/非 MVP）

## 一、MVP 四大件（CLAUDE.md 范围）

| 能力 | 状态 | 说明 |
|------|------|------|
| macOS 桌面壳 + 桌面宠物 | ✅ | Tauri 2，main 控制面板窗口 + pet 透明置顶可拖拽浮窗（原生 `startDragging`）；浏览器模式优雅降级 |
| Apple Calendar | ✅ | 自然语言解析 → 落 PG → L1 逐次确认流 → EventKit 写入系统日历 + 回写 `external_event_id`；macOS 14+ `NSCalendarsFullAccessUsageDescription` 已配置 |
| SoulLedger | ✅ | 见第二节，七项机制全部落地 |
| 权限分级 L0–L4 | ◐ | 数据模型 + API + 日历处 L1 强制 + 审计日志 + LLM 设置已持久化至 PostgreSQL；**权限分级运行时设置仍仅存内存**（重启回默认） |

## 二、SoulLedger 机制（对照设计决策文档）

| 机制 | 状态 | 实现要点 |
|------|------|---------|
| 埋点 telemetry | ✅ | append-only `signal_events`，服务端自动采集 + `/api/v1/signals` 客户端入口，白名单校验；已接入 memory、personality、chat 等核心路径 |
| 记忆留存 | ✅ | 决策树：显式"记住"→高权；dedup 命中→抬权；否则默认 0.5 交给衰减 |
| 遗忘衰减 + provenance 护栏 | ✅ | 指数半衰减纯逻辑 + 后台冷却扫描软删除；被活跃人格快照 / derived 记忆引用的源豁免遗忘；同步从 Qdrant 移除 |
| 人格 | ✅ | 用户滑块（主）+ 显式反馈样本（赞/踩、风格 chip → few-shot 注入）+ 自动演化（降级、24h 冷却 + 保守 clamp） |
| Thought Queue | ✅ | 状态机 pending→shown→accepted/dismissed/snoozed + expired；生命周期守卫 + 防打扰抑制（近 14 天 dismissed）+ 活跃上限 + 每日维护扫描 |
| LLM-as-judge 离线整合 | ✅ | 24h 冷却的离线语义合并，生成 `tier='derived'` 记忆、软降级源、保留来源链 |
| 向量检索 | ✅ | Ollama 嵌入 + Qdrant，带 SQL 回退；保存即建索引，遗忘即删点 |

## 三、Chat 与前端

| 项 | 状态 | 说明 |
|----|------|------|
| 工具调用 Tool Calling | ✅ | Native tool-use 循环，支持 Ollama / OpenAI 兼容 / DeepSeek；带 id + type 以适配 DeepSeek |
| 角色提示词 Role Prompts | ✅ | 用户自定义系统角色提示词（migration 0020），集成进对话工作区，前端含 RolePromptSettings 组件 |
| Markdown 渲染 | ✅ | MessageBubble 使用 react-markdown + remark-gfm 渲染 |
| LLM 设置持久化 | ✅ | migration 0021：模型、max_tokens、context_messages 等存 PostgreSQL，重启保留；前端 ChatSettings 面板可编辑 |
| LLM 错误处理 | ✅ | provider error body 直传前端，不再裸"400 Bad Request" |
| CORS | ✅ | Tauri webview origin 白名单已配置，桌面壳可正常启动 |

## 四、安全与质量

| 项 | 状态 | 说明 |
|----|------|------|
| 网络绑定 | ✅ | 默认 `127.0.0.1`（本地单用户、无密码 owner 令牌，不可暴露公网） |
| 集成令牌加密 | ✅ | AES-256-GCM 静态加密（`TokenCipher` 启动派生一次缓存），响应不回传任何 token |
| SQL 注入 | ✅ | 全部参数化、按 `user_id` 作用域 |
| 路由参数 | ✅ | 已修 axum 0.7 `:param` 语法（此前 `{id}` 致 by-id 路由全 404）+ router 级回归测试 |
| 测试 | ✅ | **223** 单元 + DB 集成 + 路由回归（0 失败 / 15 ignored）；clippy `--all-targets --all-features -D warnings` 零警告 |
| CI | ✅ | Rust lint/test、前端 type-check/build、Docker（GHCR 小写）、sqlx-cli 锁 0.8.x、actions v5 |

## 五、待办 / 验证缺口 / 可选增强

1. **WebSocket 回声桩替换**（可选，非 MVP）：桌面宠物状态 / 主动建议实时推送，目前 `ws/handler.rs` 仍是回声。
2. **权限分级设置持久化**（可选增强）：权限运行时设置目前仍存内存，重启回默认（LLM 设置已于 2026-06-09 完成持久化）。
3. **Thought 跨会话实体计数触发器**（设计文档 §3 follow-up，可选）：让主动建议更精准，纯后端。
4. **前端 ESLint 接入**：当前 `npm run lint` 未配置 ESLint 9 flat config。
5. **OpenAPI 契约门禁**：严格 committed spec + frontend generated client + diff gate 仍待 Phase 1。

## 六、结论

CLAUDE.md 定义的 MVP 范围（桌面壳 + Apple Calendar + SoulLedger + 权限分级）在代码层面**已全部落地**，
后端经 223 项单元/集成/回归测试通过。Apple Calendar EventKit 写入 + external_event_id 回写已完整实现。
Chat 侧工具调用、角色提示词、Markdown 渲染与 LLM 设置持久化均已完工。
剩余均为可选增强与非 MVP 项。
