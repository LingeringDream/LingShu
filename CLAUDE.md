# 灵枢 (LingShu) — Cross-platform Desktop AI Assistant

## Project

Cross-platform desktop pet AI assistant: floating desktop avatar, platform-adapted calendar scheduling (EventKit on macOS, ICS export elsewhere), tiered permissions, SoulLedger memory with forgetting and personality evolution, editable Memory & Personality Center, tool-calling chat, role prompts, persistent LLM settings, and optional cloud-account/device sessions. See [README.md](./README.md) for current status and [AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md) for the historical product design.

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/lingshu-server/src/main.rs` | Backend entry point |
| `crates/lingshu-server/migrations/` | Database migrations (27 total) |
| `crates/lingshu-server/src/llm/` | SoulLedger: memory, forgetting, personality, thoughts, consolidation, dedup, prompts, semantic, ollama, client |
| `crates/lingshu-server/src/routes/` | API routes (58 OpenAPI paths, 90 operations) |
| `crates/lingshu-vector/src/` | Vector search (Qdrant) |
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

# Test (383 passed, 17 DB-gated or network-gated ignored at the current snapshot)
cargo test --workspace
cd frontend && npm run test && npm run lint && npm run type-check && npm run build

# Tauri desktop build (macOS)
cd src-tauri && cargo build

# Health check
curl http://localhost:8080/api/v1/system/health
```

## Constraints

- Current product scope: cross-platform desktop shell, platform-adapted calendar, SoulLedger, Memory Evolution Lab, permission tiers, and optional cloud-account/device sessions. Do NOT treat the historical PRD's future VRM/Live2D, third-party connectors, or cloud memory sync as shipped capabilities.
- Apache AGE: PoC failed on standard `postgres:16-bookworm` because the image lacks the AGE extension. Do not add AGE to MVP infrastructure; if graph capabilities become necessary, run a fresh AGE custom-image vs Neo4j comparison first.
- Tauri 2: set up with `src-tauri/` directory. Desktop shell provides main + pet windows plus EventKit bridge (macOS only). Browser mode gracefully degrades desktop-only features.
- Infrastructure services (PostgreSQL, Redis, Qdrant) run in Docker. Backend and frontend can run natively or in Docker.
- API endpoints must be registered in utoipa OpenAPI docs. The committed `openapi.json` is checked by `openapi_spec_matches_committed_file` in the test suite and CI; no generated frontend client is maintained yet.
- LLM settings are persisted to PostgreSQL (migration 0021) so they survive restarts. Permission tiers are also persisted to PostgreSQL (migration 0022, `users.permissions` JSONB) and survive restarts.
- Chat supports tool calling (native tool-use loop), role prompts (customizable system prompts), and Markdown rendering (react-markdown + remark-gfm).
