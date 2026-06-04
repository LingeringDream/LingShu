# Changelog

## 2026-06-04

### Documentation Restructure

- Rewrote `README.md` as comprehensive developer documentation: badges, features, prerequisites, quick start (native + Docker), config, testing, project structure.
- Trimmed `CLAUDE.md` to AI-agent-only context (key paths, conventions, quick commands, constraints).
- Deleted `AGENTS.md` — content consolidated into `CLAUDE.md`.
- Added `CONTRIBUTING.md` with dev workflow, PR process, coding standards, commit conventions, testing.
- Added `docs/` directory with documentation index.
- Added `CHANGELOG.md`.
- Added MIT `LICENSE`.
- Deleted PRD review artifacts (`AI-PersonalAssistant-PRD-Review*.md`) — feedback incorporated into PRD.

### Product Direction

- Repositioned LingShu from AI project-manager assistant to macOS desktop AI personal assistant.
- Updated PRD to v0.4: macOS desktop pet MVP, Apple Calendar, permission tiers, SoulLedger memory/forgetting/personality, Memory & Personality Center.
- Rust 1.88 toolchain now supports native `aarch64-apple-darwin` compilation alongside Docker.
- Docker Compose uses profiles: infrastructure starts by default, `--profile full` for backend + frontend containers.
- Clarified Tauri 2, EventKit, Shortcuts, Accessibility, and full SoulLedger are planned (Phase 1+), not current.

### Notes

- Current implementation: Phase 0 / MVP prework — Rust Axum backend, React/Vite frontend, PostgreSQL/Redis/Qdrant via Docker Compose.
- PoC READMEs now record current results: AGE failed on the standard Postgres image, LLM streaming is blocked by missing Ollama, and Qdrant missed the original latency targets.
