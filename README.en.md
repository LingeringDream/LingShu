# LingShu · 灵枢

[![Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2FLingeringDream%2FLingShu%2Fmain%2FCargo.toml&query=%24.workspace.package.version&label=version&prefix=v&color=blue)](./CHANGELOG.md)
[![License](https://img.shields.io/badge/license-Commercial-red)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

[中文](./README.md)

LingShu is a personal shared-memory layer for AI tools. MCP lets Claude, Cursor, Codex and other tools carry your preferences, project decisions and experience across conversations. The client focuses on memory management, MCP setup and basic chat, with optional encrypted cloud sync.

Pets, calendars, task management, personality, thought queues and teams leave the personal client's main flow. Existing data and compatibility APIs remain. See the [current scope](docs/product-scope.md) and [positioning](docs/product-positioning.md).

---

## Command-line Installation

CLI bundles include the memory service, PostgreSQL, MCP and a web console. Use `lingshu start`, `lingshu connect cursor` and `lingshu open`; Docker is unnecessary and updates/uninstall retain memories by default.

The new CLI assets still need a release. Build from source for offline installation today; see the [CLI guide](docs/cli-installation.md) for supported platforms, commands and limitations.

## Current Version and Editions

The source version is **1.6.3**. Installer versions and checksums follow the [download manifest](lingshu-cloud/public/downloads.json); platform releases may differ from the source version.

- **Desktop client:** memory management, MCP setup and basic chat, with optional cloud account and encrypted sync. Windows x64 and macOS Apple Silicon packages include PostgreSQL, the backend, and the MCP sidecar; Docker and an external database are unnecessary. Configure Ollama or your own OpenAI-compatible model service separately.
- **Development edition:** Docker or source-based browser app with Memory Console, Agent Governance, and Benchmark & Integration. In the current working tree, desktop builds use `npm run build:client` and omit Memory Console; development builds use `npm run build`.
- **Cloud account / team web app:** login, device management, subscriptions, downloads, and shared team memory; separate from the desktop local session.

These editions are independent of subscription tiers:

| Tier | Available now | Not yet enabled |
|------|---------------|-----------------|
| Free | Local chat, memory, MCP, backup; accept team invitations | — |
| Pro | Free local capabilities and encrypted private-memory sync | Remote cross-device MCP and advanced governance |
| Team | Free local capabilities plus workspace creation and role-based team access | Planned Pro capabilities and per-seat billing |

Invited Free users receive team-member access, but creating another workspace requires their own valid Team subscription. Expiry does not remove local memory or local MCP, and existing teams are not automatically deleted. Team sharing does not imply personal cross-device sync. See [entitlements](docs/version-entitlements.md).

## Memory Engine and Experimental Evidence

The memory engine implements episode/fact/experience memory, Agent access controls, conflict tracking, and adaptive retrieval. Concurrent writes use fact-level transaction locks; conflicting claims retain provenance and await resolution. See the [write protocol](docs/memory-write-consistency.md) and [research index](docs/research/README.md).

Offline results reported on 2026-09-06:

| Evaluation | Static baseline | Comparison | Result |
|------------|----------------:|------------|-------:|
| Synthetic v3 test Top-1, 100 queries | 62.00% | LingShu Full | 94.00% |
| v3 changed-fact test Top-1, 50 queries | 30.00% | LingShu Full | 96.00% |
| LoCoMo evidence R@5, 1,977 QA | 26.82% | BM25 with speaker metadata | 52.31% |
| Stratified LoCoMo test QA score, 100 questions | 28.24 | BM25 + same generation model | 37.20 |

The v3 corpus has 660 turns and 300 templated queries, split into 200 dev / 100 test by project. States are oracle labels, so this evaluates retrieval after governance. Similarity plus state filtering achieves **99%** test Top-1, outperforming the full scorer. LoCoMo uses a retrospective split, and the 100-question score mixes category F1 with refusal accuracy; it is not full-dataset official F1. BM25, RRF, and extractive fact candidates remain offline experiments.

Standard P@K is distinct relevant hits / K; the old hits / min(K, gold count) metric is now `capped_recall`. Old 31% / 32% results were yes/no judge accuracy. Claims that all five targets passed or that unequal-budget token savings establish efficiency have been withdrawn. The [v4 governance audit](docs/research/auto-governance-audit.md) is diagnostic; labels and construction artifacts still need review. Learned weights, real multi-Agent collaboration, and million-scale concurrent service tests remain outstanding.

```powershell
# Run from the repository root; no database or model service required
cargo run -p lingshu-eval --release -- --zte-dataset datasets/zte_memory_v3_test.json --json output/zte_v3_test_results.json
```

See the [experiment report](docs/research/experiment-report.md) for protocols, ablations, evidence, and full reproduction commands.

## Implemented and Compatibility Capabilities

This inventory records existing implementations, rather than the current product scope. Pets, calendars, personality and thought queues remain legacy modules; future work follows the [current scope](docs/product-scope.md).

| Category | Capability | Status |
|----------|-----------|--------|
| MCP Memory Service | stdio MCP server with 6 tools (`lingshu_recall` / `lingshu_store` / `lingshu_grant` / `lingshu_conflicts` / `lingshu_resolve_conflict` / `lingshu_stats`), letting Claude, Cursor, Codex and others share one memory layer | ✅ Implemented |
| Concurrent Writes & Adjudication | Fact-level transactional locks, `context_scope` isolation, competing claims preserved as `disputed`, human adjudication with retained records | ✅ Implemented |
| Desktop Pet | Transparent overlay, draggable, snappable floating window; Tauri `startDragging` native drag | ✅ Tauri 2 shell ready |
| SoulLedger | Layered memory, dedup, forgetting decay (with provenance guardrails), personality evolution, offline LLM semantic consolidation, vector retrieval | ✅ Backend complete |
| Apple Calendar | Natural-language scheduling, L1 confirmation flow, event CRUD, EventKit system calendar write-back/delete, external_event_id sync | ✅ Backend + bridge complete |
| Permission Tiers | L0–L4 graduated system access + audit logging, settings persisted to PostgreSQL | ✅ Backend complete |
| Thought Queue | State machine + anti-nag suppression + lifecycle guards + daily maintenance + explainable confidence | ✅ Backend complete |
| Integration Token Encryption | AES-256-GCM at-rest encryption | ✅ Implemented |
| Signal Telemetry | Append-only event log, wired into memory, personality, and chat paths | ✅ Implemented |
| Chat Tool Calling | Native tool-use loop, supports Ollama / OpenAI-compatible / DeepSeek models | ✅ Implemented |
| Web Search | Search public webpages in chat with titles, snippets, and source URLs; controlled by a separate L1 permission | ✅ Implemented |
| Role Prompts | User-customizable system role prompts, integrated into chat workspace | ✅ Implemented |
| Markdown Rendering | MessageBubble supports formatted Markdown display | ✅ Implemented |
| LLM Settings Persistence | LLM config (model, max_tokens, context_messages, etc.) stored in PostgreSQL, survives restarts | ✅ Implemented |
| Cloud Account & Entitlements | Optional browser PKCE login, device sessions, subscription overview, and download center; Free / Pro / Team entitlements computed centrally in the cloud | ✅ Deployed; encrypted private-memory sync is enabled; remote MCP and advanced governance remain unreleased |
| Team Workspaces | Valid Team subscriptions can create workspaces; invited Free users can join, with team operations governed by roles and memory grants | ✅ Implemented; team MCP and per-seat billing are not enabled |

## Project Status

The current iteration focuses on personal shared memory: memory management, MCP setup and basic chat. The desktop starts only the main window. Pets, calendars, tasks, personality, thought queues and teams leave the personal flow; existing data, APIs and cloud subscriptions remain compatible.

The memory core includes unified online recall, atomic fact extraction, durable indexing, valid-time queries, bounded consolidation and derived-memory invalidation. See the [implementation and migration notes](docs/memory-retrieval-optimization.md). Research results are not shipped capabilities, and source changes are not installer releases.

Next priorities are provenance and conflict UI, MCP connection checks, backup/restore and release reliability. See the [current scope](docs/product-scope.md). The [personal-assistant PRD](./AI-PersonalAssistant-PRD.md) is a historical design.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust 1.88, Axum 0.7, Tokio, sqlx (PostgreSQL), fred (Redis), reqwest |
| Frontend | React 18, TypeScript, Zustand, Vite 6 |
| Integration | MCP (stdio / JSON-RPC 2.0): Claude, Cursor, Codex, Cline, VS Code, Zed, etc. |
| Data | PostgreSQL 16, Redis 7, Qdrant |
| LLM | Ollama (local, default) or any OpenAI-compatible endpoint (DeepSeek, etc.) |
| Desktop | Tauri 2 + macOS EventKit; platform fallback and ICS export elsewhere |
| CI/CD | GitHub Actions, Docker Compose |

## Source Development Prerequisites

- [Rust](https://rustup.rs) 1.88 — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ with npm
- [Docker](https://docker.com) Desktop (infrastructure services)
- [Ollama](https://ollama.com) (local LLM): pull a chat model and the `nomic-embed-text` embedding model
- macOS 14+ Apple Silicon supports EventKit and a self-contained package; Windows 10/11 x64 has a self-contained package; Linux works for CI and backend-only

## Quick Start

### Windows Development Launcher

Start Docker Desktop, then double-click `启动灵枢开发版.cmd` at the repository root. It builds the full stack and opens the memory console at `http://127.0.0.1:5173/?section=memory-lab`. Use `打开灵枢控制台.cmd` if the browser does not open, and `停止灵枢开发版.cmd` to stop containers while preserving data volumes. See the [development guide](开发版使用说明.md).

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

## MCP Integration

MCP is one of the core entry points. Once the backend is running, any MCP-capable AI tool can read and write LingShu as a shared long-term memory layer, without opening the LingShu client.

```bash
cargo build --release -p lingshu-mcp     # → target/release/lingshu-mcp
```

Add a server to your AI tool's MCP configuration (example: Claude Desktop's `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "lingshu": {
      "command": "/absolute/path/to/target/release/lingshu-mcp",
      "env": {
        "LINGSHU_BASE_URL": "http://127.0.0.1:8080"
      }
    }
  }
}
```

| Tool | Purpose |
|------|---------|
| `lingshu_recall` | Retrieve memories, optionally scoped by context and layer |
| `lingshu_store` | Write memories (`episode` / `fact` / `experience`) |
| `lingshu_grant` | Grant another agent access to a memory |
| `lingshu_conflicts` | List current disputes |
| `lingshu_resolve_conflict` | Adjudicate a dispute |
| `lingshu_stats` | Memory statistics |

- Authentication reads `LINGSHU_TOKEN` first; when unset, `lingshu-mcp` calls the local `local-session` endpoint at startup to obtain an owner token.
- The service speaks stdio. **All logs go to stderr** and stdout carries JSON-RPC only — never redirect logs into stdout.
- Write status is returned directly. **"Recorded" is not "confirmed"**: competing claims are preserved as disputes and surfaced during retrieval; re-submitting a higher confidence does not resolve them. See [concurrent memory writes](docs/memory-write-consistency.md).

> Local MCP is a Free-tier capability. Pro users synchronize end-to-end encrypted memory to each device, where local MCP clients use the same continuous memory layer.

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
  lingshu-mcp/         MCP memory service entry (stdio transport)
  lingshu-mcp-core/    MCP protocol layer and the 6 tool implementations
  lingshu-eval/        Offline evaluation: baselines, ablations, task completion, governance metrics
frontend/              React + TypeScript + Vite SPA (includes pet window entry)
  src/components/
    avatar/            Desktop pet (PetWindow, AvatarPlaceholder, AvatarControlPanel)
    chat/              Chat window (ChatWindow, ChatInput, MessageBubble, ChatSettings, RolePromptSettings)
    layout/            App layout (AppLayout)
    memory/            Memory center (MemoryCenter)
    personality/       Personality center (PersonalityCenter)
    projects/          Project manager (ProjectManager)
    settings/          Permission settings (PermissionSettings)
    thoughts/          Thought queue (ThoughtQueue)
    workspace/         Workspace page (WorkspacePage, includes CalendarSection)
  src/stores/          Zustand state management (chatStore, memoryStore, projectStore)
  src/lib/             API client, EventKit bridge, Tauri utilities
src-tauri/             Tauri 2 desktop shell (standalone Cargo workspace): main + pet windows, EventKit bridge
lingshu-cloud/         Cloud account, subscriptions, and team web app
eval/                  Python retrieval, QA, and governance experiments
datasets/              Synthetic regression and public evaluation data
docker/                Docker Compose dev environment and configs
docs/                  Design decisions, MVP status, and other documents
poc/                   Technology proofs of concept (AGE graph, LLM streaming, Qdrant)
```

SoulLedger key modules (`crates/lingshu-server/src/llm/`): `memory.rs` (extraction/dedup), `forgetting.rs` (decay + provenance guardrails), `personality.rs` (personality evolution), `consolidation.rs` (offline semantic merge), `thoughts.rs` (proactive suggestions), `dedup.rs`, `prompts.rs`, `semantic.rs` (semantic retrieval), `ollama.rs` (Ollama adapter), `client.rs` (Ollama/OpenAI + embeddings + tool calling).

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

**Calendar write/export**: After confirming an event (L1), the macOS desktop app writes it to the system calendar via EventKit and syncs back the `external_event_id`; Windows, Linux, and Web export ICS files. macOS 14+ is configured with `NSCalendarsFullAccessUsageDescription`.

## Testing

```bash
# Rust workspace tests (410 passed, 26 DB-gated or network-gated ignored)
cargo test --workspace

# Frontend unit tests (29 test files, 132 tests) + lint + type-check + build
cd frontend && npm run test && npm run lint && npm run type-check && npm run build

# Tauri build (macOS)
cd src-tauri && cargo build

# Full CI check (matching GitHub Actions)
SQLX_OFFLINE=true cargo fmt --all -- --check
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
DATABASE_URL=... REDIS_URL=... cargo test --all
```

CI (GitHub Actions): Rust fmt/clippy/test, frontend lint/type-check/build, Windows package build, OpenAPI contract check, and Docker image build (GHCR).

## API Documentation

OpenAPI 3.1 generated by [utoipa](https://github.com/juhaku/utoipa), covering **70 paths and 107 operations** (system, auth, users, LLM settings, chat, sessions, memory gateway, MCP, teams, evaluation, personality, thoughts, calendar, permissions, integrations, signals, audit, project management, etc.). The committed `openapi.json` is checked by a contract test; Swagger UI is available at `/swagger-ui` while the backend is running.

## Known Limitations

- **Automated governance does not meet its target**: fact-state classification reaches Macro-F1 44.74% (rules) / 22.97% (LLM) with a 23–27% wrong-discard rate, far above the <5% goal. The system therefore relies on conservative dispute preservation and human adjudication rather than automatically deciding truth. See the [governance audit](docs/research/auto-governance-audit.md).
- **Pro is enabled**: manual encrypted private-memory sync is available to updated clients, and synchronized memories can be read by free local MCP. Remote MCP and commercial advanced governance remain unreleased; development-edition retrieval settings remain locally persistent. Team MCP and per-seat billing remain planned. Valid Team subscriptions can already create workspaces, and invited Free users can join. See [version entitlements](docs/version-entitlements.md).
- Desktop packages include the MCP binary, and Settings can write configuration for supported clients in one step. Some clients still require manual configuration; source builds must build `lingshu-mcp` first.
- A generated frontend API client is not included yet; the current OpenAPI gate checks the committed spec against the Rust-generated document.

## Documentation

- [Product Positioning and Editions](./docs/product-positioning.md) — current client, MCP, and HTTP API entry points, planned SDKs, and Free / Pro / Team boundaries
- [Version Entitlements](./docs/version-entitlements.md) — what is enabled today versus still planned per tier
- [Concurrent Memory Writes](./docs/memory-write-consistency.md) — the write protocol shared by MCP and REST, disputes, and adjudication
- [Product Requirements Document](./AI-PersonalAssistant-PRD.md) — full product spec (Chinese; historical, not current positioning)
- [SoulLedger Design Decisions](./docs/soulledger-design-decisions.md)
- [MVP Status](./docs/mvp-status.md)
- [Contributing Guide](./CONTRIBUTING.md) · [Changelog](./CHANGELOG.md)

## License

Proprietary commercial software. Use, copying, modification, and distribution require written commercial authorization. See [LICENSE](./LICENSE).
