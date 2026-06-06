# LingShu · 灵枢

[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

macOS desktop companion AI personal assistant — a floating desktop pet with memory, personality, and calendar integration.

macOS 桌面宠物式 AI 个人助理，具备长期记忆、可控人格、主动建议和 Apple Calendar 日程管理。

---

## Features

| Category | Capability | Status |
|----------|-----------|--------|
| Desktop Pet | Transparent overlay, draggable, resizable, snap-to-edge | Planned (Phase 1) |
| SoulLedger | Layered memory, forgetting, personality evolution, editable memory center | Planned (Phase 2) |
| Apple Calendar | Natural language scheduling, conflict detection, EventKit integration | Planned (Phase 3) |
| Permission Tiers | L0–L4 graduated system access with audit logging | Planned (Phase 3) |
| Thought Queue | Proactive suggestions with explainable confidence | Planned (Phase 4) |

## Project Status

**Phase 0 / MVP prework** — infrastructure and API scaffolding is in place; feature implementation begins in Phase 1.

- ✅ Rust Axum backend skeleton (API, WebSocket, DB migrations)
- ✅ React + TypeScript + Vite frontend skeleton
- ✅ PostgreSQL + Redis + Qdrant development environment
- ✅ Chat, conversations, projects, tasks, and memories API stubs
- ⏳ Tauri 2 desktop shell — Phase 1
- ⏳ SoulLedger memory engine — Phase 2
- ⏳ Apple Calendar integration — Phase 3

Full product spec: [AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md)

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust 1.88, Axum 0.7, Tokio, sqlx (PostgreSQL), reqwest |
| Frontend | React 18, TypeScript, Three.js, Zustand, Vite 6 |
| Data | PostgreSQL 16, Redis 7, Qdrant |
| Desktop (planned) | Tauri 2 + macOS APIs (EventKit, Shortcuts, Accessibility) |
| CI/CD | GitHub Actions, Docker Compose |

## Prerequisites

- [Rust](https://rustup.rs) 1.88+ — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ and npm
- [Docker](https://docker.com) Desktop (for infrastructure services)
- macOS recommended for development; Linux works for CI/backend

## Quick Start

### 1. Clone

```bash
git clone <repo-url>
cd PA
```

### 2. Start infrastructure

```bash
docker compose -f docker/docker-compose.dev.yml up -d
```

This starts PostgreSQL (5432), Redis (6379), and Qdrant (6333).

### 3. Configure environment

```bash
cp config.example.toml config.toml
cp .env.example .env
```

- `config.toml` is the base configuration file loaded by the backend at startup. It is **gitignored** — do not commit real keys or personal model names.
- `.env` provides environment variable overrides after it is loaded by the backend. Both files ship with safe defaults; edit them for your local setup.

For local dev, update `.env` hostnames from Docker service names to `localhost`:

```env
DATABASE_URL=postgres://lingshu:lingshu@localhost:5432/lingshu
REDIS_URL=redis://localhost:6379
QDRANT_URL=http://localhost:6333
OLLAMA_URL=http://localhost:11434
SERVER_HOST=127.0.0.1
```

### 4. Run database migrations

```bash
cargo install sqlx-cli --no-default-features --features postgres,rustls
sqlx migrate run --source crates/lingshu-server/migrations
```

### 5. Start backend

```bash
cargo run -p lingshu-server
```

| Endpoint | URL |
|----------|-----|
| Health check | http://localhost:8080/api/v1/system/health |
| Swagger UI | http://localhost:8080/swagger-ui |
| OpenAPI spec | http://localhost:8080/api-docs/openapi.json |

### 6. Start frontend (separate terminal)

```bash
cd frontend
npm ci
npm run dev
```

Frontend: http://localhost:5173 (API calls proxied to backend)

### Alternative: Full Docker mode

```bash
docker compose -f docker/docker-compose.dev.yml --profile full up
```

Builds and runs backend + frontend inside Docker containers alongside infrastructure.

## Configuration

LingShu loads configuration in layers. `config.toml` is loaded first, then environment variables override it. The backend loads `.env` at startup, so values from `.env` participate in the same environment override layer as shell variables.

1. **`config.toml`** — local base config (gitignored, created from [`config.example.toml`](./config.example.toml))
2. **Environment variables** — values from [`.env`](./.env.example) or inline shell overrides, e.g. `LLM_DEFAULT_MODEL=<your-local-model> cargo run -p lingshu-server`

Key environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://lingshu:lingshu@localhost:5432/lingshu` | PostgreSQL connection |
| `DATABASE_MAX_CONNECTIONS` | `20` | DB connection pool size |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection |
| `QDRANT_URL` | `http://localhost:6333` | Qdrant connection |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama LLM endpoint |
| `LLM_DEFAULT_MODEL` | — | Ollama model for chat. Set this to a model installed on your machine. |
| `LLM_API_KEY` | — | Cloud LLM API key (Phase 1+ — not yet used; all calls go to Ollama) |
| `LLM_API_BASE_URL` | `https://api.openai.com/v1` | Cloud LLM base URL (Phase 1+ — not yet used) |
| `SERVER_HOST` | `127.0.0.1` | Backend bind address for the local management interface |
| `SERVER_PORT` | `8080` | Backend listen port |
| `JWT_SECRET` | — | JWT signing secret (change in production) |
| `ENCRYPTION_KEY` | — | 64-char hex key for encrypted fields |
| `RUST_LOG` | `info,lingshu_server=debug` | Logging verbosity |

## Project Structure

```
crates/
  lingshu-server/      Rust Axum backend (API routes, DB, WebSocket, LLM client)
  lingshu-vector/      Vector search capabilities (Qdrant client)
frontend/              React + TypeScript + Vite SPA
docker/                Docker Compose dev environment and configs
poc/                   Technology proofs of concept
  poc-age-graph/       Apache AGE graph query validation
  poc-llm-streaming/   Rust → Ollama streaming validation
  poc-qdrant-search/   Qdrant HNSW search validation
```

## Testing

```bash
# Backend unit + integration tests
cargo test --workspace

# Frontend type check
cd frontend && npm run type-check

# Frontend production build
cd frontend && npm run build
```

CI currently runs Rust lint/test, frontend type-check/build, and an OpenAPI placeholder job. Frontend ESLint and strict OpenAPI contract checks are planned but not yet active gates.

## API Documentation

OpenAPI 3.0 spec is generated by [utoipa](https://github.com/juhaku/utoipa). Swagger UI is available at `/swagger-ui` when the backend is running. Phase 0 registers only part of the implemented routes; strict backend↔frontend contract validation is planned for Phase 1.

## Documentation

- [Product Requirements Document](./AI-PersonalAssistant-PRD.md) — full product spec (Chinese)
- [Contributing Guide](./CONTRIBUTING.md) — how to contribute
- [Changelog](./CHANGELOG.md) — version history
- [Developer Docs](./docs/) — planned technical documentation index

## License

MIT License. See [LICENSE](./LICENSE).
