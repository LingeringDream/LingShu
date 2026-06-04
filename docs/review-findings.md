# Documentation Review Findings — 待办清单

> 2026-06-04 · 三轮独立审查汇总（技术准确性 / 产品结构 / 开发者体验）
> 总计 49 项发现：7 项 HIGH 已修复 ✅，42 项待处理。

---

## 状态标记

| 标记 | 含义 |
|------|------|
| ✅ | 已修复（2026-06-04） |
| 🔴 P1 | 应在 Phase 1 前处理 |
| 🟡 P2 | Phase 2-3 期间处理 |
| 🟢 P3 | 持续维护 |

---

## 一、代码质量（P1）

- [ ] **Redis 已初始化但零引用** — `state.rs` 连接 Redis 后，没有任何路由处理器使用它。要么为 session/ratelimit 接入，要么从 AppState 中移除以免误导。
- [ ] **OpenAPI 覆盖仅 ~55%** — `routes/mod.rs` 注册了 10 个 path，但 auth、users、chat、project detail、task detail、conversation detail 均未纳入 utoipa。Phase 1 契约门禁前补齐。
- [ ] **无认证中间件** — 所有端点通过 `SELECT ... LIMIT 1` 获取「第一个用户」作为当前用户。JWT secret 存在于配置但未被任何中间件消费。
- [ ] **lingshu-graph crate 死代码** — 有 `lib.rs` + `queries.rs` 但无图数据库 provision。AGE PoC 已失败。要么删除 crate，要么在 README/PRD 中明确标注仅作远期占位。
- [ ] **lingshu-vector crate 无 Qdrant 客户端** — 仅依赖 `reqwest` + `serde`，不含任何 Qdrant 集合管理或向量搜索逻辑。
- [ ] **chat/sessions 路由文件孤立** — `routes/sessions.rs` 存在但未合并到 `main.rs` 路由表，且直接 query conversations 表，API 语义不清晰。
- [ ] **WebSocket 处理器孤立** — `ws/handler.rs` 存在，`main.rs` 只 `mod ws;` 但未注册路由。
- [ ] **前端 Three.js 依赖无引用** — `three`、`@react-three/fiber`、`@react-three/drei` 已安装但零 import。AvatarPlaceholder 是纯 CSS div。
- [ ] **前端 `index.html` title 过期** — 仍显示「AI 项目经理助理」，产品已 pivot 到桌面个人助理。
- [ ] **Docker Compose `VITE_API_URL` 死配置** — 前端无任何 `.ts/.tsx` 引用此变量（Vite 开发代理直连）。
- [ ] **无基准测试代码** — PRD 附录 A 列出的性能 checklist 在 CI 中无一实现。指标端点返回硬编码占位字符串。
- [ ] **LLM 路由器简化版** — 实际 `llm/router.rs` 仅按消息长度三分类，无 intent 枚举、无 `Intent::RiskAnalysis` 等分发逻辑。
- [ ] **`config.toml` 被 figment 引用但不存在** — `config.rs:67` merge `Toml::file("config.toml")`，文件缺失时 Figment 静默跳过，但建议提供最小示例。
- [ ] **argon2 盐值用时间戳生成** — `auth.rs` 的 `generate_salt()` 用 `SystemTime::subsec_nanos()` 而非 CSPRNG。Phase 1 改为 `rand::thread_rng()` 或等价安全随机源。

---

## 二、产品设计（P2）

- [ ] **无新用户引导流程** — PRD 未描述：安装方式（DMG/App Store/Homebrew）、LLM API Key 配置引导、首次 Calendar 权限请求 UX、无 LLM 时的降级行为。
- [ ] **无产品成功指标** — 未定义 DAU/MAU、Calendar 事件创建成功率、记忆召回准确率、TTFV（首次价值实现时间）。
- [ ] **无明确非目标列表** — 未声明：不支持移动端、不支持多用户、不支持离线模式、不提供第三方 API。
- [ ] **冲突检测与 write-only Calendar 权限矛盾** — 冲突检测需要读取已有事件，但 macOS EventKit 的「write-only」权限不允许读取。需改为 full-access 权限或放弃冲突检测。
- [ ] **Swift sidecar 架构未定义** — 是 XPC service 还是 CLI bridge？通信协议？打包方式？Phase 3 前需设计文档。
- [ ] **成本模型盈亏不符** — ¥199/月定价 vs ¥170-480/月成本，重度用户直接亏损。需补充缓存+小模型路由后的成本压降分析。
- [ ] **竞品分析补缺** — 应加入：Apple Intelligence/Siri、Rewind AI、Notion AI、Raycast AI、Character.AI。
- [ ] **Thought Queue 触发引擎无实现路径** — 触发条件「用户多次提到某任务但未排期」需要 LLM 跨会话推理，无启发式规则可兜底。
- [ ] **人格参数跨模型不可移植** — 同组 trait 值在 GPT-4/Claude/Qwen 上的表现差异显著，模型切换后人格漂移不可预测。
- [ ] **附录 A 应拆分为独立技术文档** — ~950 行代码/性能优化细节占 PRD 45%，建议移入 `docs/architecture-performance.md`。

---

## 三、开发者体验（P2）

- [ ] **`cargo test --all` 已弃用** — 应全局替换为 `cargo test --workspace`（README、CONTRIBUTING、CLAUDE.md、CI yml）。
- [ ] **CI 使用 Node.js 26，README 要求 22+** — 统一基线版本。
- [ ] **ESLint 状态不一致** — CONTRIBUTING.md 说 ESLint 是「计划中」，CI 用 `continue-on-error: true` 跑 lint，但 `eslint.config.*` 不存在导致报错。要么补最小配置，要么从 CI 移除。
- [ ] **`cargo-watch` 依赖未在前提条件中说明** — CLAUDE.md 推荐 `cargo watch`，README 未提及需 `cargo install cargo-watch`。
- [ ] **README 配置表缺失 5 个变量** — `DATABASE_MAX_CONNECTIONS`、`SERVER_HOST`、`LLM_API_KEY`、`LLM_API_BASE_URL`、`ENCRYPTION_KEY`。
- [ ] **测试需要基础设施但未说明** — `cargo test --workspace` 如涉及集成测试需 PostgreSQL/Redis 容器运行。应在测试章节加注。
- [ ] **`sqlx-cli` 安装命令 README vs CI 不一致** — README 带 `rustls` feature，CI 不带。
- [ ] **`crates/lingshu-server/tests/` 目录已删除** — 二进制 crate 不支持 `tests/` 集成测试，改用 `#[cfg(test)]` 单元测试。贡献者指南应反映这点。

---

## 四、文档细节（P3）

- [ ] **PRD 迁移命名示例与实际文件日期不符** — 示例用 `20260601`，实际用 `20260528`。
- [ ] **`docs/` 下所有子文档均为 TBD 占位** — 至少应先完成 `architecture.md`（可从 PRD 第 4 节提取）和 `api.md`（可指向 Swagger UI）。
- [ ] **CHANGELOG 格式** — 建议按 [Keep a Changelog](https://keepachangelog.com/) 分类（Added/Changed/Removed）。
- [ ] **PRD 术语不统一** — 全文混用「云端模型」「外部 LLM API」「云端 API」。
- [ ] **附录 A.2.1 L1 渲染写「完整 VRM 模型」** — 应为「完整 glTF 模型」，VRM 推迟到 v2.0。
- [ ] **附录 A.5.1 WebSocket 批处理代码有逻辑错误** — `tokio::time::sleep` 在 async loop 中不应阻塞消息处理。
- [ ] **附录 A.9 checklist 中「Docker 镜像 < 50MB」** — 当前 Dockerfile 未做 distroless 优化，实际镜像远大于此。
- [ ] **Qdrant PoC filtered P95 29.82ms 超出 20ms 目标** — Phase 2 需补充过滤检索优化方案或调整 filtered 场景的时效目标。

---

## 统计

| 优先级 | 数量 | 说明 |
|--------|------|------|
| ✅ 已修复 | 7 | 本次 PRD/README/auth/test 修复 |
| 🔴 P1 | 14 | 代码质量，Phase 1 前应解决 |
| 🟡 P2 | 13 | 产品设计 + DX，Phase 2-3 |
| 🟢 P3 | 8 | 细节打磨，持续维护 |
| **合计** | **42** | |

---

*本文件随问题修复持续更新。已修复项请打勾 `[x]` 并标注日期。*
