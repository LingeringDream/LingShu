# LingShu · 灵枢

[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

[中文](./README.md)

A macOS desktop-companion AI assistant — a floating desktop pet with long-term memory (SoulLedger), an adjustable personality, proactive suggestions (Thought Queue), tiered system permissions, and Apple Calendar scheduling.

---

## Features

| Category | Capability | Status |
|----------|-----------|--------|
| Desktop Pet | Transparent overlay, draggable, snappable floating window; Tauri `startDragging` native drag | ✅ Tauri 2 shell ready |
| SoulLedger | Layered memory, dedup, forgetting decay (with provenance guardrails), personality evolution, offline LLM semantic consolidation, vector retrieval | ✅ Backend complete |
| Apple Calendar | Natural-language scheduling, L1 confirmation flow, event CRUD, EventKit system calendar write-back, external_event_id sync | ✅ Backend + bridge complete |
| Permission Tiers | L0–L4 graduated system access + audit logging | ✅ Backend complete |
| Thought Queue | State machine + anti-nag suppression + lifecycle guards + daily maintenance + explainable confidence | ✅ Backend complete |
| Integration Token Encryption | AES-256-GCM at-rest encryption | ✅ Implemented |
| Signal Telemetry | Append-only event log, wired into memory, personality, and chat paths | ✅ Implemented |
| Chat Tool Calling | Native tool-use loop, supports Ollama / OpenAI-compatible / DeepSeek models | ✅ Implemented |
| Role Prompts | User-customizable system role prompts, integrated into chat workspace | ✅ Implemented |
| Markdown Rendering | MessageBubble supports formatted Markdown display | ✅ Implemented |
| LLM Settings Persistence | LLM config (model, max_tokens, context_messages, etc.) stored in PostgreSQL, survives restarts | ✅ Implemented |

## Project Status

**Core backend mechanisms are complete; finishing desktop/native integration.** SoulLedger, personality, thought queue, permission tiers, vector retrieval, signal telemetry, tool calling, and calendar EventKit write-back are all implemented. Backend passes **223 tests (0 failures / 15 ignored)**.

- ✅ Rust + Axum backend (API, WebSocket, DB migrations 001–0021)
- ✅ React + TypeScript + Vite frontend (chat + memory/personality/calendar/thought centers + settings + role prompts + workspace)
- ✅ PostgreSQL + Redis + Qdrant development environment
- ✅ SoulLedger: memory extraction/dedup, forgetting decay + provenance guardrails, personality evolution, offline LLM semantic consolidation
- ✅ Semantic memory retrieval (Ollama embeddings + Qdrant, with SQL fallback)
- ✅ Thought Queue state machine + anti-nag + lifecycle guards + daily maintenance
- ✅ Permission tiers L0–L4 + audit log
- ✅ Integration token AES-256-GCM at-rest encryption
- ✅ Signal telemetry event layer, wired into core business paths
- ✅ Tauri 2 desktop shell (main + pet windows) + EventKit bridge (macOS 14+ `NSCalendarsFullAccessUsageDescription`)
- ✅ Apple Calendar: NL parse + CRUD + L1 confirmation + EventKit write-back + external_event_id sync
- ✅ Chat: tool-use loop, role prompts, Markdown rendering, streaming SSE responses
- ✅ LLM settings persisted to PostgreSQL (model, max_tokens, context_messages, etc. — survives restarts)

Full product spec: [AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md) (Chinese)

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust 1.88, Axum 0.7, Tokio, sqlx (PostgreSQL), fred (Redis), reqwest |
| Frontend | React 18, TypeScript, Zustand, Vite 6 |
| Data | PostgreSQL 16, Redis 7, Qdrant |
| LLM | Ollama (local, default) or any OpenAI-compatible endpoint (DeepSeek, etc.) |
| Desktop | Tauri 2 + macOS EventKit |
| CI/CD | GitHub Actions, Docker Compose |

## Prerequisites

- [Rust](https://rustup.rs) 1.88 — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ with npm
- [Docker](https://docker.com) Desktop (infrastructure services)
- [Ollama](https://ollama.com) (local LLM): pull a chat model and the `nomic-embed-text` embedding model
- macOS recommended for desktop/EventKit; Linux works for CI and backend-only

## Quick Start

### 1. Start Infrastructure

```bash
docker compose -f docker/docker-compose.dev.yml up -d   # PostgreSQL (5432) / Redis (6379) / Qdrant (6333)
```

### 2. Configure

```bash
cp config.example.toml config.toml   # gitignored; don't commit real keys or model names
cp .env.example .env
```

For local dev, change hostnames to `localhost`/`127.0.0.1`:

```env
DATABASE_URL=postgres://lingshu:lingshu@localhost:5432/lingshu
REDIS_URL=redis://localhost:6379
QDRANT_URL=http://localhost:6333
OLLAMA_URL=http://localhost:11434
SERVER_HOST=127.0.0.1
LLM_DEFAULT_MODEL=<your-installed-chat-model>
```

### 3. Install CLI and Run Migrations

```bash
# sqlx-cli pinned to 0.8.x for rustc 1.88 compatibility (0.9+ requires 1.94+)
cargo install sqlx-cli --version '^0.8' --locked --no-default-features --features postgres,rustls
sqlx migrate run --source crates/lingshu-server/migrations
```

### 4. Pull Models (Ollama)

```bash
ollama pull <your-chat-model>    # matches LLM_DEFAULT_MODEL
ollama pull nomic-embed-text     # embeddings for semantic memory search
```

### 5. Start Backend

```bash
cargo run -p lingshu-server
```

| Endpoint | URL |
|----------|-----|
| Health check | http://127.0.0.1:8080/api/v1/system/health |
| Swagger UI | http://127.0.0.1:8080/swagger-ui |
| OpenAPI spec | http://127.0.0.1:8080/api-docs/openapi.json |

### 6. Start Frontend (new terminal)

```bash
cd frontend && npm ci && npm run dev      # http://localhost:5173, /api proxied to backend
```

> Default permission is L0. Calendar features require enabling **L1** in the Permissions panel, otherwise `/api/v1/calendar/*` returns 403 (expected).

### Alternative: Full Docker

```bash
docker compose -f docker/docker-compose.dev.yml --profile full up
```

## Configuration

Layered loading: `config.toml` first, then environment variable overrides (`.env` auto-loaded at backend startup). Key variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://lingshu:lingshu@localhost:5432/lingshu` | PostgreSQL connection |
| `DATABASE_MAX_CONNECTIONS` | `20` | Connection pool size |
| `REDIS_URL` | `redis://localhost:6379` | Redis (optional; skip if empty) |
| `QDRANT_URL` | `http://localhost:6333` | Qdrant (optional; skip if empty, disables vector search) |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama endpoint |
| `LLM_DEFAULT_MODEL` | — | Chat model (must be installed locally) |
| `LLM_EMBED_MODEL` | `nomic-embed-text` | Embedding model |
| `LLM_EMBED_DIM` | `768` | Embedding dimension; must match your model (Qdrant collection size) |
| `LLM_API_KEY` | — | When set, chat/embeddings use OpenAI-compatible endpoint instead of Ollama |
| `LLM_API_BASE_URL` | — | OpenAI-compatible base URL; setting it enables cloud backend |
| `SERVER_HOST` | `127.0.0.1` | Backend bind address (single-user local; do NOT expose to network) |
| `SERVER_PORT` | `8080` | Listen port |
| `JWT_SECRET` | — | JWT signing secret (change in production) |
| `ENCRYPTION_KEY` | — | Integration token at-rest encryption key; required before creating integrations |
| `RUST_LOG` | `info,lingshu_server=debug` | Log level |

> Security: Backend binds `127.0.0.1` by default. The local session endpoint (`POST /api/v1/auth/local-session`) issues owner tokens without password — **never** bind to `0.0.0.0` or expose to the network.

## Project Structure

```
crates/
  lingshu-server/      Rust Axum backend (API routes, DB, LLM, WebSocket, crypto, telemetry)
  lingshu-vector/      Qdrant vector search client
frontend/              React + TypeScript + Vite SPA (includes pet window entry)
  src/components/
    avatar/            Desktop pet (PetWindow, AvatarPlaceholder, AvatarControlPanel)
    calendar/          Calendar panel
    chat/              Chat window (ChatWindow, ChatInput, MessageBubble, ChatSettings, RolePromptSettings)
    layout/            App layout (AppLayout)
    memory/            Memory center (MemoryCenter)
    personality/       Personality center (PersonalityCenter)
    projects/          Project manager (ProjectManager)
    settings/          Permission settings (PermissionSettings)
    thoughts/          Thought queue (ThoughtQueue)
    workspace/         Workspace page (WorkspacePage)
  src/stores/          Zustand state management (chatStore, memoryStore, projectStore)
  src/lib/             API client, EventKit bridge, Tauri utilities
src-tauri/             Tauri 2 desktop shell (standalone Cargo workspace): main + pet windows, EventKit bridge
docker/                Docker Compose dev environment and configs
docs/                  Design decisions, MVP status, and other documents
poc/                   Technology proofs of concept (AGE graph, LLM streaming, Qdrant)
```

SoulLedger key modules (`crates/lingshu-server/src/llm/`): `memory.rs` (extraction/dedup), `forgetting.rs` (decay + provenance guardrails), `personality.rs` (personality evolution), `consolidation.rs` (offline semantic merge), `thoughts.rs` (proactive suggestions), `dedup.rs`, `prompts.rs`, `semantic.rs`, `client.rs` (Ollama/OpenAI + embeddings + tool calling).

Backend routes (`crates/lingshu-server/src/routes/`): `chat.rs` (chat + tool calling), `calendar.rs`, `memories.rs`, `personality.rs`, `thoughts.rs`, `permissions.rs`, `settings.rs` (LLM settings + role prompts), `integrations.rs`, `signals.rs`, `audit.rs`, `conversations.rs`, `projects.rs`, `tasks.rs`, `users.rs`, `auth.rs`, `system.rs`, `sessions.rs`, `project_members.rs`, `task_dependencies.rs`.

## Tauri Desktop App

Two windows: **main** (full control panel, 1200×800) and **pet** (frameless + transparent + always-on-top desktop pet, 200×260). Backend runs as an independent process; frontend calls `127.0.0.1:8080`. Without Tauri (browser mode), desktop-only features like EventKit gracefully degrade.

```
┌─ src-tauri/ (Tauri 2, standalone Cargo workspace) ─────────────────┐
│  main.rs          ── Desktop entry, launches main + pet windows     │
│  eventkit.rs      ── EventKit bridge (macOS only)                   │
└──────────────────────────────────────────────────────────────────┘
         │ Tauri invoke (desktop only)       │ HTTP (127.0.0.1:8080)
         ▼                                   ▼
┌─ frontend/ (Vite + React) ──┐   ┌─ lingshu-server (Axum) ──────────┐
│  main.tsx → App.tsx          │   │  API routes, PostgreSQL, LLM     │
│  pet.tsx  → PetWindow        │   │  Runs as independent process     │
│  lib/tauri.ts (graceful deg) │   └──────────────────────────────────┘
└──────────────────────────────┘
```

```bash
# Browser mode (no Tauri required)
cargo run -p lingshu-server &
cd frontend && npm run dev

# Tauri desktop mode (macOS + Xcode CLI tools required)
cargo run -p lingshu-server &
cd frontend && ./node_modules/.bin/tauri dev
./node_modules/.bin/tauri build     # → src-tauri/target/release/bundle/macos/LingShu.app
```

**Apple Calendar write-back**: After confirming an event (L1), the desktop app writes it to the system calendar via EventKit Tauri commands and syncs back the `external_event_id`. macOS 14+ is configured with `NSCalendarsFullAccessUsageDescription`; grant calendar permissions in System Settings.

## Testing

```bash
# Backend tests (223 passed / 0 failed / 15 ignored)
cargo test --workspace

# Frontend type-check + build
cd frontend && npm run type-check && npm run build

# Tauri build (macOS)
cd src-tauri && cargo build

# Full CI check (matching GitHub Actions)
SQLX_OFFLINE=true cargo fmt --all -- --check
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
DATABASE_URL=... REDIS_URL=... cargo test --all
```

CI (GitHub Actions): Rust fmt/clippy/test, frontend type-check/build, Docker image build (GHCR).

## API Documentation

OpenAPI 3.0 generated by [utoipa](https://github.com/juhaku/utoipa), covering all **34 registered routes** (system, auth, users, LLM settings, chat, sessions, memories, personality, thoughts, calendar, permissions, integrations, signals, audit, project management, etc.). Swagger UI available at `/swagger-ui` while the backend is running.

## Known Limitations

- WebSocket is still an echo stub; real-time push for proactive suggestions is pending.
- Permission tier runtime settings are currently in-memory, resetting on restart (LLM settings were migrated to PostgreSQL on 2026-06-09).
- Thought Queue "cross-session entity count" trigger is a future enhancement.

## Documentation

- [Product Requirements Document](./AI-PersonalAssistant-PRD.md) — full product spec (Chinese)
- [SoulLedger Design Decisions](./docs/soulledger-design-decisions.md)
- [MVP Status](./docs/mvp-status.md)
- [Contributing Guide](./CONTRIBUTING.md) · [Changelog](./CHANGELOG.md)

## License

MIT — see [LICENSE](./LICENSE).
