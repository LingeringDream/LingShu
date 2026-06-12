# Changelog

## 2026-06-12

### Version 1.0.0

- `chore`: bump all version numbers to 1.0.0 (Cargo.toml, package.json, tauri.conf.json, openapi.json).
- Updated README.md and README.en.md with version badge and v1.0.0 status.

## 2026-06-10

### Permissions Persistence

- `feat(permissions)`: persist L0–L4 permission tiers to PostgreSQL (migration 0022, `users.permissions` JSONB) with DB load/save; tiers now survive server restarts.

### Calendar

- `feat(calendar)`: delete events with double-click confirmation; cancel via click-away or Escape.
- `feat(calendar)`: sync event deletions to the Apple system calendar via the EventKit bridge.
- `feat(calendar)`: refresh the event list when chat tool-calls modify events (`calendar-changed` event).
- `refactor(calendar)`: remove the standalone `CalendarPanel` component; calendar UI now lives as `CalendarSection` inside `WorkspacePage`.

### Projects

- `feat(projects)`: add delete confirmation (cancel via click-away or Escape).

### Testing

- `test`: add frontend Vitest suite covering stores (chat, memory, project), `lib` (api, tauri, eventkit), and the `MessageBubble` component.
- `test`: add unit tests across backend route modules (audit, project_members, task_dependencies, settings, calendar, etc.).

### Documentation

- Updated `README.md`, `README.en.md`, `CLAUDE.md`, `docs/mvp-status.md`, and `CHANGELOG.md`: migrations 001–0022, permission persistence, calendar UI consolidation, 65 OpenAPI operations, and expanded test inventory.

## 2026-06-09

### LLM Settings Persistence

- `feat(settings)`: persist LLM settings (model, max_tokens, context_messages) to PostgreSQL (migration 0021) so they survive restarts.
- `feat(settings)`: lift max_tokens ceiling and add context_messages for large-context models.
- Frontend ChatSettings panel reads/writes persisted settings via `/api/v1/settings/llm`.

### Chat & Tool Calling

- `feat`: add native tool-use loop to chat, supporting Ollama / OpenAI-compatible / DeepSeek models.
- `fix(llm)`: include id + type on tool calls so DeepSeek accepts the echo.
- `fix(llm)`: surface provider error body instead of bare "400 Bad Request".
- `fix`: optimize tool-call message flow to avoid unnecessary pushes.

### Role Prompts & Markdown

- `feat`: add role prompts for user customization (migration 0020), integrated into chat workspace.
- `feat`: add Markdown support to MessageBubble component (react-markdown + remark-gfm).

### Tauri & Desktop

- `fix(pet)`: use Tauri native `startDragging` so the desktop pet can be moved.
- `fix(cors)`: allow bundled Tauri webview origins so the local console can boot.
- `fix(web)`: gate dashboard calendar fetch on L1 permission to avoid 403 noise.

### Documentation

- Rewrote `README.md` in Chinese with English `README.en.md` (separate files, cross-linked).
- Updated `CLAUDE.md`, `docs/mvp-status.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, and `docs/soulledger-design-decisions.md` to reflect current progress.

## 2026-06-08

### Calendar & EventKit

- `fix(desktop)`: fix EventKit macOS 14+ access detection and harden write-back endpoint.
- `feat(web)`: sync confirmed events into system calendar via Tauri EventKit bridge.
- `feat(desktop)`: add `NSCalendarsFullAccessUsageDescription` for macOS 14+.
- `feat(calendar)`: `external_event_id` column (migration 0019) + write-back endpoint.

### Memory & Consolidation

- `feat(memory)`: offline LLM consolidation with provenance-preserving merge.
- `feat(memory)`: add derived tier and `source_memory_ids` columns (migration 0018).
- `feat(forgetting)`: extend protection set to consolidation sources.

### Thoughts & Telemetry

- `feat(thoughts)`: lifecycle guards, snooze scheduling, active cap, daily maintenance.
- `fix(thoughts)`: reconcile status vocabulary between API and DB CHECK (migration 0017).
- `chore`: wire telemetry signals into memory, personality, chat paths.

## 2026-06-07

### Testing & CI

- `fix(test)`: make db tests self-contained and isolated for clean-db CI runs.
- `fix(ci)`: clean clippy `--all-targets` warnings and test-suite failures.
- `fix(ci)`: pin sqlx-cli to version 0.8 for compatibility with rustc 1.88.
- `test(routes)`: router-level coverage for parameterized paths.
- `fix(routes)`: use axum 0.7 `:param` path syntax so id routes match (was 404).

### Security & Hardening

- `fix`: harden memory write and crypto, deduplicate semantic search.
- `fix`: declare telemetry module and fix doc indentation.

## 2026-06-06

### Frontend & Desktop

- `fix(frontend)`: fix wsTarget https→wss regex, tighten dev server bind.
- CORS configuration for local development.

### SoulLedger

- SoulLedger core mechanisms: memory extraction/dedup, forgetting decay with provenance guardrails, personality evolution.
- Signal telemetry event layer (migration 0015).
- Integration token AES-256-GCM at-rest encryption.

## 2026-06-05

### Initial SoulLedger

- Memory, personality, thought queue, and calendar event data models.
- Migrations 0012–0014: personality_snapshots, thought_queue, calendar_events.
- Permission tiers L0–L4 with audit log.
- Vector search client (lingshu-vector) with Qdrant.

## 2026-06-04

### Documentation Restructure

- Rewrote `README.md` as comprehensive developer documentation.
- Trimmed `CLAUDE.md` to AI-agent-only context (key paths, conventions, quick commands, constraints).
- Deleted `AGENTS.md` — content consolidated into `CLAUDE.md`.
- Added `CONTRIBUTING.md` with dev workflow, PR process, coding standards, commit conventions, testing.
- Added `docs/` directory with documentation index.
- Added `CHANGELOG.md`.
- Added MIT `LICENSE`.
- Deleted PRD review artifacts.

### Product Direction

- Repositioned LingShu from AI project-manager assistant to macOS desktop AI personal assistant.
- Updated PRD to v0.4: macOS desktop pet MVP, Apple Calendar, permission tiers, SoulLedger.
- Rust 1.88 toolchain now supports native `aarch64-apple-darwin` compilation.
- Docker Compose uses profiles: infrastructure starts by default, `--profile full` for backend + frontend containers.
- Clarified Tauri 2, EventKit, and full SoulLedger are planned (Phase 1+).

### Notes

- Current implementation: Phase 0 / MVP prework — Rust Axum backend, React/Vite frontend, PostgreSQL/Redis/Qdrant via Docker Compose.
- PoC READMEs: AGE failed on standard Postgres image, LLM streaming blocked by missing Ollama, Qdrant missed original latency targets.
