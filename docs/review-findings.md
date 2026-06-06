# Documentation & Code Review Findings — 待办清单

> 2026-06-06 · 三轮独立审查 + 两次代码审查（Phase 1 / Phase 2）
> 总计 56 项发现：31 项已修复 ✅，25 项待处理。

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
- [x] **OpenAPI 覆盖仅 ~55%** — ✅ 2026-06-04→2026-06-06：初期仅 ~50% 端点注册（9 paths），现已覆盖全部 18 个路由模块共 60 个 handler（system, auth, users, settings, projects, tasks, conversations, chat, sessions, memories, calendar, permissions, project_members, task_dependencies, personality, thoughts, integrations, audit）。所有 HTTP handler 均已在 `routes/mod.rs` 的 `#[derive(OpenApi)]` 中注册。
- [x] **无认证中间件** — ✅ 2026-06-04：`AuthUser` 从请求状态读取 `jwt_secret` 验证 JWT；缺失 token 不再回退到「第一个用户」。
- [x] **LLM 路由器死代码** — ✅ 2026-06-04：删除 `llm/router.rs`、`ModelRouter` 字段，模型名直接从 `LlmSettings` 读取。
- [x] **`state.http` 死字段** — ✅ 2026-06-04：移除未被任何路由使用的 `http: reqwest::Client` 字段。
- [x] **Redis 已初始化但零引用** — ✅ 2026-06-06：Redis 缓存层已全面接入路由处理器。`settings.rs`：LLM 配置 read-through + write-through 缓存；`sessions.rs`：会话列表缓存（TTL 30s）+ 创建/删除时主动失效；`conversations.rs`：会话变更时调用 `invalidate_session_cache` 失效缓存。`state.rs` 在启动时自动连接 Redis，不可用时优雅降级。详见 `cache.rs`。
- [x] **lingshu-graph crate 死代码** — ✅ 2026-06-06：从 workspace members、`lingshu-server` 依赖、Dockerfile 缓存占位和 `Cargo.lock` 中移除，并删除 `crates/lingshu-graph/` 目录。AGE PoC 仍保留在 `poc/` 下作为历史验证材料。
- [x] **lingshu-vector crate 无 Qdrant 客户端** — ✅ 2026-06-06：`crates/lingshu-vector/src/search.rs` 已实现 `QdrantClient`（`new` / `create_collection` / `upsert_point` / `search`），基于 HTTP API 的轻量客户端。**注意：记忆检索仍未接入向量搜索（见 Phase 2 遗留）。**
- [x] **chat/sessions 路由孤立** — ✅ 2026-06-06：`main.rs` 已 `.merge(routes::sessions::router())`，`routes/sessions.rs` 提供 `list_sessions` / `get_session` / `delete_session` 三个端点。
- [ ] **WebSocket 处理器孤立** — `ws/handler.rs` 存在（实现 echo 逻辑），`main.rs` 有 `mod ws;` 声明，但**未注册任何 WebSocket 路由**（无 `.route()` 或 `.merge()` 挂载 handler）。
- [ ] **前端 Three.js 依赖无引用** — `three`、`@react-three/fiber`、`@react-three/drei` 在 `package.json` 中已安装，但前端源码中 **零 import**。
- [x] **前端 `index.html` title 过期** — ✅ 2026-06-04：改为「macOS 桌面 AI 个人助理」。
- [x] **Docker Compose `VITE_API_URL` 死配置** — ✅ 2026-06-06：`docker-compose.dev.yml` 设置 `VITE_API_URL: http://backend:8080`，`frontend/vite.config.ts` 通过 `loadEnv()` 读取并用于 Vite dev proxy 的 `/api` target，同时派生 `/ws` proxy target。
- [ ] **无基准测试代码** — 性能 checklist 在 CI 中无一实现。
- [x] **`config.toml` 被 figment 引用但不存在** — ✅ 2026-06-06：新增 `config.example.toml`（覆盖 database/redis/qdrant/server/llm/security/cors 全部 7 个 section），README Quick Start 步骤 3 引导 `cp config.example.toml config.toml`，配置章节说明 `config.toml` 先加载、随后由 `.env`/shell 环境变量覆盖。`config.toml` 仍被 `.gitignore` 忽略，新克隆可通过示例文件获得完整默认配置。

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
- [x] **Thought Queue 触发引擎无实现路径** — ✅ 2026-06-06：`llm/thoughts.rs` 实现完整生成管线（LLM 生成 → 去重 → 插入 `thought_queue`）；`POST /api/v1/thoughts/generate` 提供手动触发；`chat.rs` 在每条消息结束后自动调用 `should_generate_thoughts()` + `generate_and_save_thoughts()` 后台生成。**注意：仍无定时 worker 定期为沉默用户生成思考。**
- [ ] **人格参数跨模型不可移植** — 同组 trait 值在不同 LLM 上表现差异显著，尚无模型校准方案或映射表。
- [ ] **附录 A 应拆分为独立技术文档** — ~950 行占 PRD 45%。

---

## 三、开发者体验（P2）

- [x] **ESLint 状态不一致** — ✅ 2026-06-04：创建 `eslint.config.js` 扁平配置（TypeScript + 浏览器全局变量），`npm run lint` 零错误通过。
- [x] **README 配置表缺失变量** — ✅ 2026-06-04：从 7 变量扩展到 14 变量，补齐 `DATABASE_MAX_CONNECTIONS`、`SERVER_HOST`、`LLM_API_KEY`、`LLM_API_BASE_URL`、`ENCRYPTION_KEY`、`LLM_DEFAULT_MODEL`。
- [x] **README 快速入门 OLLAMA_URL 缺失** — ✅ 2026-06-04：步骤 3 的 env 块中补上 `OLLAMA_URL=http://localhost:11434`。
- [x] **旧 workspace 测试命令已弃用** — ✅ 2026-06-06：`CLAUDE.md`、`README.md`、`CONTRIBUTING.md` 全部统一为 `cargo test --workspace`。
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

- [x] **记忆抽取无重复检测** — ✅ 2026-06-06：`llm/dedup.rs` 实现 Jaccard 相似度去重（`DEDUP_SIMILARITY_THRESHOLD = 0.82`），`memory.rs` 的 `save_memory()` 在插入前调用 `find_duplicate_memory()`，命中则提升 importance 而非重复插入。Thought Queue 生成同样使用该去重逻辑。
- [x] **记忆抽取无速率限制** — ✅ 2026-06-06：`memory.rs` 实现 `EXTRACTION_COOLDOWN_SECS = 60`（per-user cooldown），`should_extract_memory()` 在冷却期内跳过抽取，附 `extraction_cooldown_is_scoped_per_user` 单元测试。
- [ ] **记忆检索无向量搜索** — `search_memories` 仍使用 PostgreSQL `ILIKE` 关键词匹配，按 `importance` 排序。Qdrant 客户端已在 `lingshu-vector` 中就绪，但记忆的 embedding 生成和向量索引尚未集成。
- [x] **人格引擎未接入** — ✅ 2026-06-06：`llm/personality.rs` 实现 `evolve_and_save_personality()` 自动演化引擎；`routes/personality.rs` 提供 5 个端点（list/create/get-active/activate/evolve）；`chat.rs` 每条消息结束后调用 `should_evolve_personality()` + `evolve_and_save_personality()` 自动触发人格演化；激活的人格快照注入聊天 system prompt。
- [x] **Thought Queue 未接入** — ✅ 2026-06-06：`llm/thoughts.rs` 实现 `generate_and_save_thoughts()` 及去重逻辑；`routes/thoughts.rs` 提供 4 个端点（list/get/update/generate）；`chat.rs` 消息结束后调用 `should_generate_thoughts()` + `generate_and_save_thoughts()` 自动后台生成。

---

## 统计

统计口径：所有 `- [x]` 条目和「Phase 2 已交付」表格中的 ✅ 计入已修复；所有 `- [ ]` 条目按所在章节归类为 P1/P2/P3（Phase 2 遗留计入 P2）。本文件共 56 项发现。

| 优先级 | 数量 | 变化 |
|--------|------|------|
| ✅ 已修复 | 31 | +13（P1+6: Redis/Qdrant/sessions/VITE_API_URL/config.toml/lingshu-graph、P2 产品+1: Thought Queue 引擎、DevEx+1: cargo test --workspace、Phase 2 遗留+4: 去重/限流/人格/Thought Queue 接入、OpenAPI 描述修正） |
| 🔴 P1 | 3 | -7（WebSocket、Three.js、无基准） |
| 🟡 P2 | 15 | -2（产品设计 9 + 开发者体验 5 + Phase 2 遗留 1） |
| 🟢 P3 | 7 | 0（无变化） |
| **合计待处理** | **25** | -9 |

---

*本文件随问题修复持续更新。已修复项请打勾 `[x]` 并标注日期。*
