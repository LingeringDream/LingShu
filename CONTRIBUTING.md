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

- CI must pass the currently active gates: Rust format/clippy/test and frontend type-check/build
- Frontend ESLint and strict OpenAPI contract validation are planned gates, but they are not active yet
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
npm run type-check
npm run build
```

Frontend ESLint is planned, but the repository does not yet include the ESLint 9 flat config required by `npm run lint`.

### Database Migrations

- Use `sqlx migrate add <description>` to create new migrations
- Migrations must be forward-compatible (zero-downtime)
- Once merged to `main`, do not modify existing migration files
- Add columns with `DEFAULT` values; split renames into three steps
- Current range: 001–0021 (21 migrations)

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
# Backend (223 passed / 0 failed / 15 ignored)
cargo test --workspace

# Frontend
cd frontend && npm run type-check && npm run build

# Tauri desktop (macOS only)
cd src-tauri && cargo build

# OpenAPI contract
# Phase 0 exposes Swagger/OpenAPI at runtime, but no strict committed-spec diff is implemented yet.
# 34 routes currently registered in utoipa ApiDoc.
```

When adding new API endpoints, register them with `utoipa` attributes in `crates/lingshu-server/src/routes/mod.rs`. Once the Phase 1 contract gate exists, include a test or generated-spec update that validates the OpenAPI schema.

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

By contributing, you agree that your contributions will be licensed under the [MIT License](./LICENSE).
