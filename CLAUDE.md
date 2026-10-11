# 灵枢 (LingShu) — Memory Layer for AI Tools

## Project

灵枢是连接常用 AI 工具的个人共享记忆层。当前个人客户端主线为记忆管理、MCP 接入与基础聊天，可选云账户与加密同步；桌宠、日历、项目任务、人格、思考队列与团队不再进入个人主流程。

以 [docs/product-scope.md](./docs/product-scope.md) 和 [docs/product-positioning.md](./docs/product-positioning.md) 定义产品范围，以 [docs/version-entitlements.md](./docs/version-entitlements.md) 定义付费权益。旧模块、数据库与兼容 API 暂保留，云端团队服务继续兼容；历史个人助理 PRD 不代表当前范围。

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/lingshu-cli/src/` | CLI lifecycle, per-user config, MCP tool setup |
| `scripts/install.sh`, `scripts/install.ps1` | Prebuilt CLI installation / version switching |
| `docs/cli-installation.md` | CLI usage, data ownership, release limitations |
| `crates/lingshu-server/src/main.rs` | Backend entry point |
| `crates/lingshu-server/migrations/` | Database migrations (34 total) |
| `crates/lingshu-server/src/llm/` | SoulLedger: memory, forgetting, personality, thoughts, consolidation, dedup, prompts, semantic, ollama, client |
| `crates/lingshu-server/src/routes/` | API routes (70 OpenAPI paths, 107 operations) |
| `crates/lingshu-vector/src/` | Vector search (Qdrant) |
| `crates/lingshu-mcp/src/main.rs` | MCP 服务入口（stdio，日志走 stderr） |
| `crates/lingshu-mcp-core/src/lib.rs` | MCP 协议层与 6 个工具（recall/store/grant/conflicts/resolve/stats） |
| `crates/lingshu-eval/src/` | 离线评测：基线、消融、任务完成度、治理指标 |
| `docs/memory-write-consistency.md` | MCP 与 REST 共用的并发写入协议、争议与裁决 |
| `frontend/src/main.tsx` | Frontend entry point (memory console; team compatibility route is dev-only) |
| `frontend/src/pet.tsx` | Pet window entry point |
| `frontend/src/components/` | React components (9 module directories; calendar UI lives in workspace/) |
| `src-tauri/src/main.rs` | Tauri desktop entry (main window; legacy pet commands retained) |
| `src-tauri/src/eventkit.rs` | EventKit bridge (macOS Calendar) |
| `docker/docker-compose.dev.yml` | Dev environment orchestration |
| `AI-PersonalAssistant-PRD.md` | Historical personal-assistant requirements |

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

# Test (database/network-gated regressions may be ignored in the regular suite)
cargo test --workspace
cd frontend && npm run test && npm run lint && npm run type-check && npm run build

# Tauri desktop build (macOS)
cd src-tauri && cargo build

# Health check
curl http://localhost:8080/api/v1/system/health
```

## Constraints

- Current product scope: personal shared memory via MCP, memory management and basic chat. Keep legacy data and API compatibility; do not expand pet/calendar/tasks/personality/thought/team features as part of the current client. Development/competition tools remain separate. See docs/product-scope.md for acceptance criteria.
- Pro encrypted private-memory sync is available; cross-device remote MCP and advanced commercial governance are not. Local MCP remains Free. Cloud sync uses client-side AES-256-GCM; never imply that the cloud can decrypt memories or recover a recovery key. Existing Team subscriptions and web workspaces remain compatible; team MCP and per-seat billing are unavailable. Effective Team subscriptions can create workspaces; invited Free users can join.
- Automated fact-state governance does NOT meet its target (Macro-F1 44.74% rules / 22.97% LLM, 23–27% wrong-discard vs <5% goal). Never claim auto-governance works; the shipped behavior is conservative dispute preservation plus human adjudication. See `docs/competition/auto-governance-audit.md`.
- Do not cite the withdrawn v1 benchmark conclusions ("五项全部达标", P@5 83%). The v1 report is archived at `docs/competition/experiment-report-v1-archived.md`; current numbers are v3 (self-built) and LoCoMo (public).
- Apache AGE: PoC failed on standard `postgres:16-bookworm` because the image lacks the AGE extension. Do not add AGE to MVP infrastructure; if graph capabilities become necessary, run a fresh AGE custom-image vs Neo4j comparison first.
- Tauri 2: set up with `src-tauri/` directory. Desktop shell starts only the main memory window; legacy pet code and EventKit bridge remain for compatibility (EventKit is macOS only). Browser mode gracefully degrades desktop-only features.
- Infrastructure services (PostgreSQL, Redis, Qdrant) run in Docker. Backend and frontend can run natively or in Docker.
- API endpoints must be registered in utoipa OpenAPI docs. The committed `openapi.json` is checked by `openapi_spec_matches_committed_file` in the test suite and CI; no generated frontend client is maintained yet.
- LLM settings are persisted to PostgreSQL (migration 0021) so they survive restarts. Permission tiers are also persisted to PostgreSQL (migration 0022, `users.permissions` JSONB) and survive restarts.
- Chat supports tool calling (native tool-use loop), role prompts (customizable system prompts), and Markdown rendering (react-markdown + remark-gfm).
