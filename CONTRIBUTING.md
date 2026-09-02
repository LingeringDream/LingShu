# Contributing to LingShu (灵枢)

Thanks for your interest in contributing! LingShu is a macOS desktop AI personal assistant in active development.

## Getting Started

1. Read the [中文 README](./README.md) or [English README](./README.en.md) to set up your development environment
2. Read the [PRD](./AI-PersonalAssistant-PRD.md) to understand the product vision
3. Pick an issue labeled `good first issue` or `help wanted`

### Prerequisites

- Rust 1.88 with `rustfmt` and `clippy`
- Node.js 22+ and npm
- Docker Desktop (for PostgreSQL, Redis, Qdrant)
- macOS recommended for desktop/EventKit development; Linux works for CI and backend-only

## Development Workflow

1. Fork the repository
2. Create a feature branch from `main`:
   ```bash
   git checkout -b feat/my-feature
   ```
3. Make your changes
4. Run tests locally (see below)
5. Commit using Conventional Commits (see below)
6. Open a pull request against `main`

## Pull Request Process

- CI must pass the currently active gates: Rust format/clippy/test, frontend lint/type-check/build, and OpenAPI contract validation
- At least one approving review is required
- Squash merge is preferred — keep commit history clean
- Update documentation if your change affects public APIs or developer setup

## Coding Standards

### Rust

All Rust code must pass CI checks:

```bash
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
SQLX_OFFLINE=true cargo fmt --all -- --check
cargo test --workspace   # requires DATABASE_URL + REDIS_URL for DB integration tests
```

Configuration: [`clippy.toml`](./clippy.toml) and [`rustfmt.toml`](./rustfmt.toml)

### TypeScript / React

```bash
cd frontend
npm run lint
npm run type-check
npm run build
```

Frontend ESLint 9 flat config is included (`eslint.config.mjs`). Run `npm run lint` to check.

### Database Migrations

- Use `sqlx migrate add <description>` to create new migrations
- Migrations must be forward-compatible (zero-downtime)
- Once merged to `main`, do not modify existing migration files
- Add columns with `DEFAULT` values; split renames into three steps
- Current range: 001–0022 (22 migrations)

### Tauri (macOS Desktop)

- `src-tauri/` is a standalone Cargo workspace
- EventKit bridge in `src-tauri/src/eventkit.rs` (macOS only)
- Browser mode gracefully degrades: `lib/tauri.ts` checks `isTauri()` before calling native APIs
- Build: `cd src-tauri && cargo build`

## Commit Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

| Prefix | Use for |
|--------|---------|
| `feat:` | New features |
| `fix:` | Bug fixes |
| `chore:` | Build, deps, tooling |
| `docs:` | Documentation changes |
| `refactor:` | Code restructuring without behavior change |
| `test:` | Adding or updating tests |
| `style:` | Formatting, whitespace (non-functional) |

Scope is optional but encouraged: `feat(chat): add streaming response`.

## Testing

```bash
# Backend (300+ test functions, 16 DB-gated ignored)
cargo test --workspace

# Frontend
cd frontend && npm run lint && npm run type-check && npm run build

# Tauri desktop (macOS only)
cd src-tauri && cargo build

# OpenAPI contract
# Committed openapi.json + openapi_spec_matches_committed_file test.
# 65 routes currently registered in utoipa ApiDoc.
```

When adding new API endpoints, register them with `utoipa` attributes in `crates/lingshu-server/src/routes/mod.rs`. Once the Phase 1 contract gate exists, include a test or generated-spec update that validates the OpenAPI schema.

## Releasing / Version Bump

The project version lives in **one source of truth**: the `[workspace.package]` version
in the root `Cargo.toml`. Everything else derives from it or is updated by one script —
you never hand-edit version strings across the tree.

```bash
# Bump every manifest + regenerate the OpenAPI contract in one command
./scripts/bump-version.sh 1.2.0
```

This updates `Cargo.toml`, `src-tauri/Cargo.toml`, `frontend/package.json`, and
regenerates `openapi.json`. These derive automatically and need no edits:

- **`tauri.conf.json`** — `version` points at `../frontend/package.json`.
- **OpenAPI `info.version`** — omitted in code, so `utoipa` uses `CARGO_PKG_VERSION`.
- **README version badge** — a shields.io `dynamic/toml` badge reads `Cargo.toml` on `main`.

After running the script: write a `CHANGELOG.md` entry, commit, and tag `vX.Y.Z`.
`CHANGELOG.md` is the only place a version is written by hand (it's the release log).

## Project Structure

```
crates/
  lingshu-server/      Backend (API, DB, LLM, WebSocket, crypto, telemetry)
  lingshu-vector/      Qdrant vector search client
frontend/              React + TypeScript + Vite SPA
src-tauri/             Tauri 2 desktop shell (standalone workspace)
docker/                Docker Compose dev environment
docs/                  Design decisions, MVP status
```

## Issues

- **Bug reports**: include steps to reproduce, expected vs actual behavior, and environment details (OS, Rust version, Docker version)
- **Feature requests**: describe the problem, proposed solution, and which product phase it aligns with
- **Labels**: watch for `good first issue`, `help wanted`, `bug`, `enhancement`

## License

By submitting a contribution, you represent that you have the right to do so
and grant the LingShu copyright holders a perpetual, worldwide, non-exclusive,
irrevocable, royalty-free license to use, reproduce, modify, distribute,
sublicense, and commercialize that contribution as part of LingShu.
Contributions included in LingShu are distributed under the
[LingShu Commercial Software License](./LICENSE).
