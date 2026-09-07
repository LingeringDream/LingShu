# 灵枢 (LingShu) — Memory Layer for AI Tools

## Project

灵枢是连接不同 AI 工具的个人、团队与企业记忆层，同时提供桌面宠物式 AI 助手。当前入口为灵枢客户端、MCP（让 Claude / Cursor / Codex 共享同一套记忆）和 HTTP API；Python / TypeScript SDK 属于后续规划。

> **灵枢不是另一个 AI。灵枢是连接所有 AI 的记忆层。**

能力包括：SoulLedger 长期记忆（含遗忘衰减与人格演化）、Memory Gateway（三层异构记忆 + 多 Agent 权限空间）、并发写入与冲突治理、自适应检索、平台适配的日程（macOS EventKit，其他平台 ICS）、权限分级、可选云账户与版本权益（Free / Pro / Team）。当前定位、版本边界与付费边界以 [docs/product-positioning.md](./docs/product-positioning.md) 与 [docs/version-entitlements.md](./docs/version-entitlements.md) 为准；[AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md) 是历史设计稿，不代表当前定位。

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/lingshu-server/src/main.rs` | Backend entry point |
| `crates/lingshu-server/migrations/` | Database migrations (29 total) |
| `crates/lingshu-server/src/llm/` | SoulLedger: memory, forgetting, personality, thoughts, consolidation, dedup, prompts, semantic, ollama, client |
| `crates/lingshu-server/src/routes/` | API routes (70 OpenAPI paths, 107 operations) |
| `crates/lingshu-vector/src/` | Vector search (Qdrant) |
| `crates/lingshu-mcp/src/main.rs` | MCP 服务入口（stdio，日志走 stderr） |
| `crates/lingshu-mcp-core/src/lib.rs` | MCP 协议层与 6 个工具（recall/store/grant/conflicts/resolve/stats） |
| `crates/lingshu-eval/src/` | 离线评测：基线、消融、任务完成度、治理指标 |
| `docs/memory-write-consistency.md` | MCP 与 REST 共用的并发写入协议、争议与裁决 |
| `frontend/src/main.tsx` | Frontend entry point (main window) |
| `frontend/src/pet.tsx` | Pet window entry point |
| `frontend/src/components/` | React components (9 module directories; calendar UI lives in workspace/) |
| `src-tauri/src/main.rs` | Tauri desktop entry (main + pet windows) |
| `src-tauri/src/eventkit.rs` | EventKit bridge (macOS Calendar) |
| `docker/docker-compose.dev.yml` | Dev environment orchestration |
| `AI-PersonalAssistant-PRD.md` | Full product requirements |

## Conventions

- Rust crates: `lingshu-*`
- API paths: `/api/v1/*`
- Database tables: `snake_case`, plural
- React components: `PascalCase`
- Commits: [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`)

## Quick Commands

```bash
# One-command local debug (browser mode, no Tauri build): infra + migrations + backend + frontend
./scripts/dev.sh          # or: npm run dev  → open http://localhost:5173

# Infrastructure only (default profile)
docker compose -f docker/docker-compose.dev.yml up -d

# Full Docker stack
docker compose -f docker/docker-compose.dev.yml --profile full up

# Native backend
cargo run -p lingshu-server

# Native backend with hot reload
cargo watch -x "run -p lingshu-server"

# Frontend
cd frontend && npm ci && npm run dev

# Run migrations
sqlx migrate run --source crates/lingshu-server/migrations

# Test (Rust workspace: 408 passed, 19 DB-gated or network-gated ignored at the current snapshot)
cargo test --workspace
cd frontend && npm run test && npm run lint && npm run type-check && npm run build

# Tauri desktop build (macOS)
cd src-tauri && cargo build

# Health check
curl http://localhost:8080/api/v1/system/health
```

## Constraints

- Current product scope: memory layer with three implemented entry points (desktop client, MCP, HTTP API), SoulLedger, Memory Gateway with conflict governance, adaptive retrieval, permission tiers, and Free/Pro/Team entitlements. Python/TypeScript SDKs remain planned. Do NOT treat the historical PRD's future VRM/Live2D or third-party connectors as shipped capabilities.
- **Do NOT describe cloud memory sync, cross-device MCP, advanced governance, or per-seat billing as available.** They are planned capabilities and are explicitly marked "未开放" in `docs/version-entitlements.md`. Local MCP (stdio, this machine only) is a Free capability and IS available. A valid Team subscription can already create a team workspace; invited Free users can join it.
- Automated fact-state governance does NOT meet its target (Macro-F1 44.74% rules / 22.97% LLM, 23–27% wrong-discard vs <5% goal). Never claim auto-governance works; the shipped behavior is conservative dispute preservation plus human adjudication. See `docs/competition/auto-governance-audit.md`.
- Do not cite the withdrawn v1 benchmark conclusions ("五项全部达标", P@5 83%). The v1 report is archived at `docs/competition/experiment-report-v1-archived.md`; current numbers are v3 (self-built) and LoCoMo (public).
- Apache AGE: PoC failed on standard `postgres:16-bookworm` because the image lacks the AGE extension. Do not add AGE to MVP infrastructure; if graph capabilities become necessary, run a fresh AGE custom-image vs Neo4j comparison first.
- Tauri 2: set up with `src-tauri/` directory. Desktop shell provides main + pet windows plus EventKit bridge (macOS only). Browser mode gracefully degrades desktop-only features.
- Infrastructure services (PostgreSQL, Redis, Qdrant) run in Docker. Backend and frontend can run natively or in Docker.
- API endpoints must be registered in utoipa OpenAPI docs. The committed `openapi.json` is checked by `openapi_spec_matches_committed_file` in the test suite and CI; no generated frontend client is maintained yet.
- LLM settings are persisted to PostgreSQL (migration 0021) so they survive restarts. Permission tiers are also persisted to PostgreSQL (migration 0022, `users.permissions` JSONB) and survive restarts.
- Chat supports tool calling (native tool-use loop), role prompts (customizable system prompts), and Markdown rendering (react-markdown + remark-gfm).
