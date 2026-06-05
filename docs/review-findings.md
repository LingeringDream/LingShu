# Documentation & Code Review Findings — 待办清单

> 2026-06-04 · 三轮独立审查 + 两次代码审查（Phase 1 / Phase 2）
> 总计 56 项发现：18 项已修复 ✅，38 项待处理。

---

## 状态标记

| 标记 | 含义 |
|------|------|
| ✅ | 已修复 |
| 🔴 P1 | 应在 Phase 1 前处理 |
| 🟡 P2 | Phase 2-3 期间处理 |
| 🟢 P3 | 持续维护 |

---

## 一、代码质量（P1）

- [x] **账号密码注册不符合本地产品定位** — ✅ 2026-06-05：移除对外注册/登录路由，改为本地默认会话签发 JWT；前端不再展示注册、登录、退出入口。
- [x] **OpenAPI 覆盖仅 ~55%** — ✅ 2026-06-04：新增 settings (GET/PATCH)、memories CRUD (6 handlers)，当前 9 paths / 18 endpoints = ~50% 端点注册。auth/users/chat 仍未纳入 utoipa。
- [x] **无认证中间件** — ✅ 2026-06-04：`AuthUser` 从请求状态读取 `jwt_secret` 验证 JWT；缺失 token 不再回退到「第一个用户」。
- [x] **LLM 路由器死代码** — ✅ 2026-06-04：删除 `llm/router.rs`、`ModelRouter` 字段，模型名直接从 `LlmSettings` 读取。
- [x] **`state.http` 死字段** — ✅ 2026-06-04：移除未被任何路由使用的 `http: reqwest::Client` 字段。
- [ ] **Redis 已初始化但零引用** — `state.rs` 连接 Redis 后，没有任何路由处理器使用它。
- [ ] **lingshu-graph crate 死代码** — 有 `lib.rs` + `queries.rs` 但无图数据库 provision。
- [ ] **lingshu-vector crate 无 Qdrant 客户端** — 仅依赖 `reqwest` + `serde`。
- [ ] **chat/sessions 路由孤立** — `routes/sessions.rs` 未合并到 `main.rs` 路由表。
- [ ] **WebSocket 处理器孤立** — `ws/handler.rs` 存在但未注册路由。
- [ ] **前端 Three.js 依赖无引用** — `three`、`@react-three/fiber`、`@react-three/drei` 已安装但零 import。
- [x] **前端 `index.html` title 过期** — ✅ 2026-06-04：改为「macOS 桌面 AI 个人助理」。
- [ ] **Docker Compose `VITE_API_URL` 死配置** — 前端无任何引用。
- [ ] **无基准测试代码** — 性能 checklist 在 CI 中无一实现。
- [ ] **`config.toml` 被 figment 引用但不存在** — `config.rs` merge `Toml::file("config.toml")`。

### 代码审查发现的额外项（已修复）

- [x] **SSE 流永不关闭** — ✅ 2026-06-04：引入 `SseState` enum（Active/Done），Done 状态下 unfold 返回 None 终止流。
- [x] **未 trim 的模型名** — ✅ 2026-06-04：`settings.model = model.trim().to_string()`。
- [x] **serde_json::Value 每行堆分配** — ✅ 2026-06-04：替换为 typed `OllamaLine`/`OllamaMessage` struct。
- [x] **重复序列化代码** — ✅ 2026-06-04：提取为 `sse_event()` helper。
- [x] **fetch_memory_context 未过滤 user_id** — ✅ 2026-06-04：添加 `WHERE user_id = $1`。

---

## 二、产品设计（P2）

- [ ] **无新用户引导流程** — PRD 未描述：安装方式、LLM API Key 配置引导、首次 Calendar 权限 UX、无 LLM 时的降级行为。
- [ ] **无产品成功指标** — 未定义 DAU/MAU、Calendar 事件创建成功率、记忆召回准确率、TTFV。
- [ ] **无明确非目标列表** — 未声明：不支持移动端、不支持多用户、不支持离线模式。
- [ ] **冲突检测与 write-only Calendar 权限矛盾** — 冲突检测需要读取事件，macOS EventKit write-only 不允许。
- [ ] **Swift sidecar 架构未定义** — XPC service 还是 CLI bridge？通信协议？打包方式？
- [ ] **成本模型盈亏不符** — ¥199/月 vs ¥170-480/月，重度用户亏损。
- [ ] **竞品分析补缺** — Apple Intelligence/Siri、Rewind AI、Notion AI、Raycast AI。
- [ ] **Thought Queue 触发引擎无实现路径** — 跨会话推理无启发式规则可兜底。
- [ ] **人格参数跨模型不可移植** — 同组 trait 值在不同 LLM 上表现差异显著。
- [ ] **附录 A 应拆分为独立技术文档** — ~950 行占 PRD 45%。

---

## 三、开发者体验（P2）

- [x] **ESLint 状态不一致** — ✅ 2026-06-04：创建 `eslint.config.js` 扁平配置（TypeScript + 浏览器全局变量），`npm run lint` 零错误通过。
- [x] **README 配置表缺失变量** — ✅ 2026-06-04：从 7 变量扩展到 14 变量，补齐 `DATABASE_MAX_CONNECTIONS`、`SERVER_HOST`、`LLM_API_KEY`、`LLM_API_BASE_URL`、`ENCRYPTION_KEY`、`LLM_DEFAULT_MODEL`。
- [x] **README 快速入门 OLLAMA_URL 缺失** — ✅ 2026-06-04：步骤 3 的 env 块中补上 `OLLAMA_URL=http://localhost:11434`。
- [ ] **`cargo test --all` 已弃用** — 应全局替换为 `cargo test --workspace`。
- [ ] **CI 使用 Node.js 26，README 要求 22+** — 统一基线版本。
- [ ] **`cargo-watch` 依赖未说明** — README 未提及需 `cargo install cargo-watch`。
- [ ] **测试需要基础设施但未说明** — 集成测试需 PostgreSQL/Redis 运行。
- [ ] **`sqlx-cli` 安装命令 README vs CI 不一致** — README 带 `rustls` feature。
- [ ] **`crates/lingshu-server/tests/` 已删除** — 二进制 crate 改用 `#[cfg(test)]` 单元测试，贡献者指南应反映。

---

## 四、文档细节（P3）

- [ ] **PRD 迁移命名示例与实际不符** — 示例 `20260601`，实际 `20260528`。
- [ ] **`docs/` 下子文档均为 TBD** — 应至少完成 `architecture.md` 和 `api.md`。
- [ ] **CHANGELOG 格式** — 建议按 Keep a Changelog 分类。
- [ ] **PRD 术语不统一** — 混用「云端模型」「外部 LLM API」。
- [ ] **附录 A.2.1 L1 渲染写「完整 VRM 模型」** — 应为 glTF。
- [ ] **附录 A.5.1 WebSocket 批处理代码有逻辑错误** — `tokio::time::sleep` 不应阻塞消息处理。
- [ ] **附录 A.9 checklist「Docker 镜像 < 50MB」** — 当前未做 distroless 优化。

---

## 五、Phase 2 新增（SoulLedger 记忆系统）

### 已交付

| 阶段 | 内容 | 状态 |
|------|------|------|
| 2a | `personality_snapshots` + `thought_queue` 迁移 | ✅ |
| 2b | 聊天自动记忆抽取 (`llm/memory.rs`) | ✅ |
| 2c | 记忆 CRUD API（6 端点） | ✅ |
| 2d | 记忆中心前端 UI | ✅ |
| 2e | 记忆注入聊天 system prompt | ✅ |

### Phase 2 遗留

- [ ] **记忆抽取无重复检测** — 用户重复说同一事实会产生多条相同记忆。
- [ ] **记忆抽取无速率限制** — 每条消息 spawn 一个后台 LLM 调用，高频使用会压垮 Ollama。
- [ ] **记忆检索无向量搜索** — 当前用 `importance + updated_at` 排序，无 Qdrant 语义召回。
- [ ] **人格引擎未接入** — `personality_snapshots` 表已建，但无自动演化或 API。
- [ ] **Thought Queue 未接入** — `thought_queue` 表已建，但无触发逻辑或 API。

---

## 统计

| 优先级 | 数量 | 变化 |
|--------|------|------|
| ✅ 已修复 | 18 | +11（含代码审查发现的 5 项 + Phase 2 发现的 1 项） |
| 🔴 P1 | 10 | -4（LLM router、state.http、title、辅助项已修复） |
| 🟡 P2 | 17 | +4（Phase 2 遗留） |
| 🟢 P3 | 7 | -1 |
| **合计待处理** | **34** | |

---

*本文件随问题修复持续更新。已修复项请打勾 `[x]` 并标注日期。*
