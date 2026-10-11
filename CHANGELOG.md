# Changelog

## [Unreleased]

## [1.6.2] - 2026-10-11

### Added
- 命令行客户端 `lingshu`：后台 start / stop / status、按需启动的 stdio MCP、Cursor / Claude / Codex 接入配置及备份、网页控制台、升级与卸载。
- macOS / Linux shell 和 Windows PowerShell 安装器；预构建包自带 PostgreSQL、MCP 与网页，无需 Docker。校验归档后切换版本，升级与卸载默认保留 CLI 数据。
- 四平台 CLI 打包与发布流程，包含真实安装、记忆、MCP、升级、卸载和显式清除数据的回归。

### Changed
- 个人客户端收敛为记忆管理、MCP 接入与基础聊天；移除辅助主入口、首页日历提醒轮询与桌宠默认窗口。
- 后端支持 CLI 网页静态资源、独立内置数据库目录与端口，桌面默认数据库配置保持兼容。

- `fix(retrieval)`: supplement nearest candidates with a bounded exact-phrase channel, strengthen literal name/identifier evidence in lexical scoring, and preserve the duplicate-similarity metric. Add a paired 10-query quality regression; synthetic gains are not a real-world accuracy claim.

- `perf(retrieval)`: use pg_trgm GiST nearest candidates before bounded token checks for common Chinese/English queries; retain short-query term lookup and filter ACL/validity before the candidate limit. Add a transaction-local SQL timeout and a reproducible 100k-row production-service regression. Candidate ranking changes; this is not a general recall-quality claim.
- Upgrade notice: migration `20260908000005` builds an additional GiST index, with disk/write overhead and possible write blocking during migration. No business database migration or installer deployment is implied.

## 2026-09-08

### Version 1.6.2

- `fix(desktop)`: keep the Memory Console (动态记忆引擎 / experiment lab) out of the packaged client. Editions are now decided by `frontend/src/lib/edition.ts` — client builds fold `isMemoryLabAvailable` to `false`, so the lab route, its nav entry and its component are dropped from the bundle instead of merely being hidden.
- Unify chat and Gateway/MCP recall with indexed Chinese/English lexical candidates and scored vector candidates; remove the recent-500-row candidate restriction.
- Make extracted facts, provenance and conflict handling transactional; add durable, retryable vector indexing with edit/delete reconciliation (eventual consistency, not exactly-once).
- Add explicit `as_of` valid-time queries using current ACLs; superseded claims remain separate from confirmed results.
- Bound consolidation input and revalidate sources before commit. Propagate source changes to pending-review derived summaries; exclude them from recall and avoid repeated source demotion on replacement.
- Add production-path regression fixtures and database tests. These are not real-model quality or large-scale performance benchmarks.
- Upgrade notice: four new migrations; index backfill can incur embedding work. Existing derived summaries are preserved but marked pending review because source revision snapshots are unavailable. No new installer, deployment or migration on a business database is implied by this source update.

## 2026-09-07

### Version 1.6.1

- `build(desktop)`: release the 1.6.1 installer — a self-contained macOS package (`.app` + `.dmg`) that bundles the PostgreSQL-backed `lingshu-server` sidecar and the local `lingshu-mcp` sidecar, so no database or runtime needs to be installed first.
- `chore(release)`: roll the version across the Cargo workspace, the Tauri desktop manifest, the frontend manifest, and the committed OpenAPI contract.

## 2026-09-06

### Version 1.6.1

- `feat(eval)`: add the standalone `crates/lingshu-eval` harness. It links the production ranking code directly, so measured behaviour is the behaviour that ships — no second copy of the scorer.
- `feat(eval)`: report the full official metric set — Accuracy, P@1/P@3/P@5, R@5, Hit@5, MRR and evidence-support Sup@5 — instead of Hit@5 alone.
- `feat(eval)`: add the public LoCoMo dataset (10 conversations / 5,882 memories / 1,977 QA, CC BY-NC 4.0) with frozen offline MiniLM embeddings, plus ablation, conflict-governance, multi-Agent permission, latency, memory-footprint and open-domain task-completion experiments.
- `feat(eval)`: add an end-to-end QA F1 measurement (retrieval + local generation + LLM-as-judge) and a real resident-set-size probe measured in isolated child processes.
- `fix(retrieval)`: teach `for_query` English before/after markers and a new aggregation intent, so summary-style questions stop being ranked by recency.
- `perf(retrieval)`: rewrite `lexical_similarity` as packed-u64 merge scanning — roughly 20x faster, with an equivalence test proving identical output.
- `docs`: rewrite the memory experiment report with a requirement-to-evidence mapping table, per-difficulty results, measured RSS, and an explicit resolved / unresolved evidence boundary section.
- `ux(downloads)`: drop the GitHub download buttons from the download hub. Builds are now distributed through the network drive only, and the docs, FAQ and status copy were updated to match.

### Version 1.6.0

- `feat(mcp)`: add `crates/lingshu-mcp` and `crates/lingshu-mcp-core`, a stdio MCP server that exposes the memory gateway to any MCP-capable AI tool (`lingshu_recall` / `lingshu_store` / `lingshu_grant` / `lingshu_conflicts` / `lingshu_resolve_conflict` / `lingshu_stats`). Each tool identifies itself with an `agent_id`, which maps naturally onto the multi-Agent memory model.
- `feat(teams)`: add team workspaces — create and join a team, member roles, email and in-app invitations, Agent credentials, team memory read/write, and an audit trail.
- `feat(entitlements)`: centralise the three-tier `free` / `pro` / `team` policy on the server. Subscription rows are never accepted from clients, and an expiry date always wins over a stale `active` status.
- `fix(entitlements)`: show team entitlements to invited members. Membership now grants the team feature set even when the member keeps a free personal plan — only creating a *new* workspace requires a paid team subscription.
- `feat(desktop)`: add an "AI tools" settings panel and bundle the `lingshu-mcp` sidecar in the desktop installer.
- `feat(memory)`: add write coordination for concurrent memory writes and a dedicated team memory space.

## 2026-09-05

### Version 1.5.3

- `feat(macOS)`: add a self-contained macOS package with embedded PostgreSQL and a platform-correct application-data directory.
- `build(macOS)`: add package and notarization scripts, enable embedded PostgreSQL in sidecar builds by default, and reduce release binary size.

## 2026-09-04

### Version 1.5.2

- `fix(desktop)`: show the control panel immediately on launch and make LingShu single-instance; reopening the app now reveals and focuses the existing panel instead of spawning duplicate windows and sidecars.
- `fix(desktop)`: generate persistent per-install JWT and encryption secrets for the bundled backend.
- `fix(windows)`: recover automatically when a crashed embedded PostgreSQL process leaves a stale `postmaster.pid` file behind.
- `fix(windows)`: bundle the x64 Microsoft C runtime in the installer, retry incomplete PostgreSQL runtime extraction, and persist sidecar startup output to `sidecar.log`.
- `fix(desktop)`: preserve the packaged Tauri origins in sidecar CORS even when machine-level environment variables or a local config file override the normal origin list.
- `feat(account)`: add the optional cloud-account panel, browser-based PKCE login, device-session management, subscription overview, and account-center links.
- `feat(downloads)`: add the production account center's download hub with versioned SHA-256 metadata, Lanzou distribution, and private GitHub Release fallback.
- `ux(desktop)`: replace the opaque `Failed to fetch` startup error with an actionable local-service message.

## 2026-09-03

### Version 1.5.1

- `fix(desktop)`: keep the Windows first-launch session handshake alive while the embedded database is extracted and initialized.
- `fix(windows)`: reliably hide the main control-panel window when its close button is clicked while keeping LingShu and the desktop pet running.
- `ux(desktop)`: explain that the first local database initialization can take one to two minutes.
## 2026-09-02

### Version 1.5.0

- `feat(memory)`: add a three-layer `episode` / `fact` / `experience` memory protocol with temporal metadata, scene tags, and provenance.
- `feat(memory-gateway)`: add multi-Agent private, shared, and global memory spaces with per-memory grants and access isolation.
- `feat(retrieval)`: add temporal conflict governance, adaptive six-factor reranking, stale-memory filtering, and a static RAG comparison mode.
- `feat(benchmark)`: add a reproducible 60-turn dataset, baseline comparison, ablation experiments, threshold reporting, and the Memory Evolution Lab UI.
- `feat(desktop)`: add embedded PostgreSQL support for the self-contained Windows sidecar package.
- `docs`: add the memory engine research index, architecture, deployment guide, innovation brief, experiment report, and evidence boundaries.

## 2026-08-31

### Licensing

- Replaced the MIT License with the proprietary LingShu Commercial Software License.
- Updated Cargo package metadata, contribution terms, and Chinese and English README license notices.

## 2026-07-26

### Version 1.4.1

- `fix(chat)`: recover DeepSeek DSML tool calls emitted as plain response content so approved tools execute instead of leaking raw protocol markup into chat.
- `security(chat)`: accept only complete DSML envelopes whose tools were offered in the current iteration; reject malformed calls, unknown tools, duplicate parameters, and mixed ordinary text.
- `test(chat)`: cover full-width, ASCII, HTML-escaped, multi-call, and rejection-path DSML responses.

## 2026-07-10

### Web Search

- `feat(search)`: add a read-only `web_search` chat tool backed by public web results, with titles, snippets, and source URLs preserved for cited answers.
- `feat(permissions)`: add a persisted, default-off `L1 · 联网搜索` permission and settings toggle.
- `security(search)`: enforce query/response limits, HTTPS/HTTP result filtering, a short request timeout, and server-side-only outbound requests.
- `fix(search)`: recognize DuckDuckGo HTTP 202/challenge pages, fall back across independent search sources, reject unrelated fallback results, and record the successful provider in audit details.
- `fix(chat)`: allow at most one outbound web search per user turn and terminate failures with server-authored text, preventing retry storms and fabricated training-cutoff explanations.
- `test`: cover HTML parsing, redirect URL decoding, source formatting, permission persistence, UI toggling, and an ignored live-network smoke test.

## 2026-06-15

### Version 1.1.0

- `chore`: bump all version numbers to 1.1.0 (Cargo.toml, package.json, tauri.conf.json, openapi.json).
- Updated README.md, README.en.md, CLAUDE.md, and docs/mvp-status.md with v1.1.0.

## 2026-06-13

### Version 1.0.3

- `fix(desktop)`: Tauri-spawned `lingshu-server` now self-terminates when its parent app process dies, preventing orphaned sidecars from holding port 8080 after dev Ctrl+C, Rust rebuild restarts, crashes, or force quits.
- `test`: add parent-watch env/PPID tests and a Tauri env-contract test for the sidecar lifecycle handoff.

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
