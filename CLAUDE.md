# 灵枢 (LingShu) — macOS Desktop AI Assistant

## Project

macOS desktop pet AI assistant. MVP: floating desktop avatar, Apple Calendar scheduling, tiered permissions, SoulLedger memory with forgetting and personality evolution, editable Memory & Personality Center. See [README.md](./README.md) for full overview and [AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md) for detailed spec.

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/lingshu-server/src/main.rs` | Backend entry point |
| `crates/lingshu-server/migrations/` | Database migrations |
| `crates/lingshu-vector/src/` | Vector search (Qdrant) |
| `frontend/src/main.tsx` | Frontend entry point |
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

# Test
cargo test --workspace
cd frontend && npm run type-check && npm run build

# Health check
curl http://localhost:8080/api/v1/system/health
```

## Constraints

- MVP priorities: macOS desktop shell, Apple Calendar, SoulLedger, permission tiers. Do NOT build VRM/Live2D, PM workbench, third-party connectors, or L4 autonomous screen control.
- Apache AGE: PoC failed on standard `postgres:16-bookworm` because the image lacks the AGE extension. Do not add AGE to MVP infrastructure; if graph capabilities become necessary, run a fresh AGE custom-image vs Neo4j comparison first.
- Tauri 2: planned for Phase 1, NOT yet set up. No `src-tauri/` directory exists.
- Infrastructure services (PostgreSQL, Redis, Qdrant) run in Docker. Backend and frontend can run natively or in Docker.
- API endpoints must be registered in utoipa OpenAPI docs. Strict OpenAPI diffing and generated frontend clients are planned for Phase 1, not currently enforced by CI.
