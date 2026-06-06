# LingShu · 灵枢

[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

macOS desktop companion AI personal assistant — a floating desktop pet with memory, personality, and calendar integration.

macOS 桌面宠物式 AI 个人助理，具备长期记忆、可控人格、主动建议和 Apple Calendar 日程管理。

---

## 功能概览 · Features

| 类别 Category | 能力 Capability | 状态 Status |
|----------|-----------|--------|
| 桌面宠物 Desktop Pet | 透明浮层、可拖拽、可缩放、贴边吸附 Transparent overlay, draggable, resizable, snap-to-edge | 规划中 Planned (Phase 1) |
| SoulLedger | 分层记忆、遗忘机制、人格演化、可编辑记忆中心 Layered memory, forgetting, personality evolution, editable memory center | 规划中 Planned (Phase 2) |
| Apple Calendar | 自然语言排程、冲突检测、EventKit 集成 Natural language scheduling, conflict detection, EventKit integration | 规划中 Planned (Phase 3) |
| 权限分级 Permission Tiers | L0–L4 逐级系统权限 + 审计日志 Graduated system access with audit logging | 规划中 Planned (Phase 3) |
| 思考队列 Thought Queue | 主动建议 + 可解释置信度 Proactive suggestions with explainable confidence | 规划中 Planned (Phase 4) |

## 项目状态 · Project Status

**Phase 0 / MVP 预研** — 基础设施与 API 脚手架已就位，功能实现从 Phase 1 开始。
**Phase 0 / MVP prework** — infrastructure and API scaffolding is in place; feature implementation begins in Phase 1.

- ✅ Rust Axum 后端骨架（API、WebSocket、数据库迁移）· Rust Axum backend skeleton (API, WebSocket, DB migrations)
- ✅ React + TypeScript + Vite 前端骨架 · React + TypeScript + Vite frontend skeleton
- ✅ PostgreSQL + Redis + Qdrant 开发环境 · PostgreSQL + Redis + Qdrant development environment
- ✅ 聊天、会话、项目、任务、记忆 API 桩 · Chat, conversations, projects, tasks, and memories API stubs
- ✅ Tauri 2 桌面壳 — Phase 1 · Tauri 2 desktop shell — Phase 1
- ⏳ SoulLedger 记忆引擎 — Phase 2 · SoulLedger memory engine — Phase 2
- ✅ Apple Calendar 解析与 API — 部分完成 · Apple Calendar parse + API — partial

完整产品规格：[AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md)
Full product spec: [AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md) (Chinese)

## 技术栈 · Tech Stack

| 层级 Layer | 技术 Technology |
|-------|-----------|
| 后端 Backend | Rust 1.88, Axum 0.7, Tokio, sqlx (PostgreSQL), reqwest |
| 前端 Frontend | React 18, TypeScript, Zustand, Vite 6 |
| 数据 Data | PostgreSQL 16, Redis 7, Qdrant |
| 桌面 Desktop | Tauri 2 + macOS APIs (EventKit, Shortcuts, Accessibility) |
| CI/CD | GitHub Actions, Docker Compose |

## 环境要求 · Prerequisites

- [Rust](https://rustup.rs) 1.88+ — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ 与 npm · and npm
- [Docker](https://docker.com) Desktop（基础设施服务）· for infrastructure services
- 推荐 macOS 开发；Linux 可用于 CI/后端 · macOS recommended for development; Linux works for CI/backend

## 快速开始 · Quick Start

### 1. 克隆仓库 · Clone

```bash
git clone https://github.com/LingeringDream/LingShu.git
cd LingShu
```

### 2. 启动基础设施 · Start infrastructure

```bash
docker compose -f docker/docker-compose.dev.yml up -d
```

启动 PostgreSQL (5432)、Redis (6379) 和 Qdrant (6333)。
This starts PostgreSQL (5432), Redis (6379), and Qdrant (6333).

### 3. 配置环境 · Configure environment

```bash
cp config.example.toml config.toml
cp .env.example .env
```

- `config.toml` 是后端启动时加载的基础配置文件，已加入 **gitignore**——请勿提交真实密钥或个人模型名称。· `config.toml` is the base configuration file loaded by the backend at startup. It is **gitignored** — do not commit real keys or personal model names.
- `.env` 提供环境变量覆盖。两个文件都包含安全默认值；根据本地环境编辑。· `.env` provides environment variable overrides after it is loaded by the backend. Both files ship with safe defaults; edit them for your local setup.

本地开发时，将 `.env` 中的主机名从 Docker 服务名改为 `localhost`：
For local dev, update `.env` hostnames from Docker service names to `localhost`:

```env
DATABASE_URL=postgres://lingshu:lingshu@localhost:5432/lingshu
REDIS_URL=redis://localhost:6379
QDRANT_URL=http://localhost:6333
OLLAMA_URL=http://localhost:11434
SERVER_HOST=127.0.0.1
```

### 4. 运行数据库迁移 · Run database migrations

```bash
cargo install sqlx-cli --no-default-features --features postgres,rustls
sqlx migrate run --source crates/lingshu-server/migrations
```

### 5. 启动后端 · Start backend

```bash
cargo run -p lingshu-server
```

| 端点 Endpoint | URL |
|----------|-----|
| 健康检查 Health check | http://localhost:8080/api/v1/system/health |
| Swagger UI | http://localhost:8080/swagger-ui |
| OpenAPI 规范 OpenAPI spec | http://localhost:8080/api-docs/openapi.json |

### 6. 启动前端（新终端）· Start frontend (separate terminal)

```bash
cd frontend
npm ci
npm run dev
```

前端访问：http://localhost:5173（API 调用代理至后端）· Frontend: http://localhost:5173 (API calls proxied to backend)

### 备选方案：完整 Docker 模式 · Alternative: Full Docker mode

```bash
docker compose -f docker/docker-compose.dev.yml --profile full up
```

在 Docker 容器中构建并运行后端 + 前端，与基础设施服务一起启动。
Builds and runs backend + frontend inside Docker containers alongside infrastructure.

## 配置 · Configuration

灵枢采用分层配置：先加载 `config.toml`，再由环境变量覆盖。后端启动时自动加载 `.env`，因此 `.env` 中的值与 shell 变量属于同一环境覆盖层。
LingShu loads configuration in layers. `config.toml` is loaded first, then environment variables override it. The backend loads `.env` at startup, so values from `.env` participate in the same environment override layer as shell variables.

1. **`config.toml`** — 本地基础配置（gitignored，从 [`config.example.toml`](./config.example.toml) 创建）· local base config (gitignored, created from [`config.example.toml`](./config.example.toml))
2. **环境变量 Environment variables** — 来自 [`.env`](./.env.example) 或命令行覆盖的值，如 · values from [`.env`](./.env.example) or inline shell overrides, e.g. `LLM_DEFAULT_MODEL=<your-local-model> cargo run -p lingshu-server`

关键环境变量 · Key environment variables:

| 变量 Variable | 默认值 Default | 说明 Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://lingshu:lingshu@localhost:5432/lingshu` | PostgreSQL 连接 · connection |
| `DATABASE_MAX_CONNECTIONS` | `20` | 数据库连接池大小 · DB connection pool size |
| `REDIS_URL` | `redis://localhost:6379` | Redis 连接 · connection |
| `QDRANT_URL` | `http://localhost:6333` | Qdrant 连接 · connection |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama LLM 端点 · endpoint |
| `LLM_DEFAULT_MODEL` | — | Ollama 聊天模型。设置为本机已安装的模型。· Ollama model for chat. Set this to a model installed on your machine. |
| `LLM_EMBED_MODEL` | `nomic-embed-text` | Ollama 嵌入模型，用于语义记忆检索。· Ollama embedding model for semantic memory search. |
| `LLM_API_KEY` | — | 云端 LLM API 密钥（Phase 1+ — 尚未使用；所有调用走 Ollama）· Cloud LLM API key (Phase 1+ — not yet used; all calls go to Ollama) |
| `LLM_API_BASE_URL` | `https://api.openai.com/v1` | 云端 LLM 基础 URL（Phase 1+ — 尚未使用）· Cloud LLM base URL (Phase 1+ — not yet used) |
| `SERVER_HOST` | `127.0.0.1` | 后端绑定地址 · Backend bind address for the local management interface |
| `SERVER_PORT` | `8080` | 后端监听端口 · Backend listen port |
| `JWT_SECRET` | — | JWT 签名密钥（生产环境请更换）· JWT signing secret (change in production) |
| `ENCRYPTION_KEY` | — | 集成 token 静态加密密钥；创建集成前必须配置。· Token-at-rest encryption key for integrations; required before creating integrations. |
| `RUST_LOG` | `info,lingshu_server=debug` | 日志级别 · Logging verbosity |

## 项目结构 · Project Structure

```
crates/
  lingshu-server/      Rust Axum 后端（API 路由、数据库、WebSocket、LLM 客户端）
                        Rust Axum backend (API routes, DB, WebSocket, LLM client)
  lingshu-vector/      向量搜索（Qdrant 客户端）
                        Vector search capabilities (Qdrant client)
frontend/              React + TypeScript + Vite 单页应用 · SPA
src-tauri/             Tauri 2 桌面壳（独立 Cargo workspace）
                        Tauri 2 desktop shell (standalone Cargo workspace)
  src/main.rs          桌面入口：main 面板窗口 + pet 透明浮窗
                        Desktop entry: main control panel + transparent pet window
docker/                Docker Compose 开发环境与配置 · dev environment and configs
poc/                   技术验证 · Technology proofs of concept
  poc-age-graph/       Apache AGE 图查询验证 · graph query validation
  poc-llm-streaming/   Rust → Ollama 流式传输验证 · streaming validation
  poc-qdrant-search/   Qdrant HNSW 搜索验证 · HNSW search validation
```

## Tauri 桌面应用 · Tauri Desktop App

灵枢的桌面壳使用 Tauri 2，提供两个窗口：

LingShu's desktop shell is built with Tauri 2 and provides two windows:

| 窗口 Window | 类型 | 说明 |
|------|------|------|
| **main** | 常规窗口 Regular | 1200×800，承载完整控制面板（首页/聊天/日历/记忆/人格/设置）。Houses the full control panel. |
| **pet** | 透明无边框浮窗 Frameless transparent overlay | 200×260，always-on-top，可拖拽的桌面宠物。Floating desktop pet, draggable. |

### 架构 · Architecture

```
┌─ src-tauri/ (Tauri 2, 独立 Cargo workspace) ─────────────────────┐
│  main.rs          ── 桌面入口，启动 main + pet 两个窗口               │
│  eventkit.rs      ── Phase B: EventKit 桥接 (macOS only)           │
└──────────────────────────────────────────────────────────────────┘
         │ Tauri invoke (仅桌面)          │ HTTP (127.0.0.1:8080)
         ▼                                ▼
┌─ frontend/ (Vite + React) ──┐   ┌─ lingshu-server (Axum) ────────┐
│  main.tsx → App.tsx          │   │  API routes, PostgreSQL, LLM   │
│  pet.tsx  → PetWindow        │   │  作为独立进程运行                │
│  lib/tauri.ts (优雅降级)      │   │  Runs as independent process   │
└──────────────────────────────┘   └────────────────────────────────┘
```

- **后端独立运行**：`cargo run -p lingshu-server`（或 Docker），监听 `127.0.0.1:8080`
- **Backend runs independently** on `127.0.0.1:8080` — managed separately, not as a Tauri sidecar
- **前端以两种模式运行**：浏览器 (`npm run dev`) 或 Tauri webview (`tauri dev`)
- **Frontend runs in two modes**: browser (`npm run dev`) or Tauri webview (`tauri dev`)
- **Tauri API 优雅降级**：浏览器中 `isTauri()` 返回 false，Tauri 特有功能（如 EventKit）自动静默跳过
- **Tauri API degrades gracefully**: `isTauri()` returns false in browser, Tauri-only features (like EventKit) silently skip

### 开发命令 · Dev Commands

```bash
# 浏览器模式（无需 Tauri）· Browser mode (no Tauri needed)
# 1. 启动后端 Start backend
cargo run -p lingshu-server

# 2. 启动前端 Start frontend
cd frontend && npm run dev
# → http://localhost:5173

# Tauri 桌面模式（需要 macOS）· Tauri desktop mode (macOS required)
# 启动全部：后端 + 前端 Vite + Tauri 窗口
cargo run -p lingshu-server &   # 先启动后端 start backend first
cd /path/to/repo && ./frontend/node_modules/.bin/tauri dev

# 生产构建 · Production build
cd /path/to/repo && ./frontend/node_modules/.bin/tauri build
# → src-tauri/target/release/bundle/macos/LingShu.app
```

### Tauri 窗口行为 · Window Behavior

- **Pet 窗口**：底部右侧定位，frameless + transparent + always-on-top。点击打开主窗口，双击显示气泡提示。
- **Pet window**: positioned bottom-right, frameless + transparent + always-on-top. Click to open main window, double-click for bubble hints.
- **Main 窗口**：居中，最小 800×600，可调整大小。
- **Main window**: centered, min 800×600, resizable.
- **macOS Private API**：已启用（透明窗口需要），见 `src-tauri/tauri.conf.json` 中的 `macOSPrivateApi: true`。
- **macOS Private API** is enabled (required for transparent windows). See `src-tauri/tauri.conf.json`.

## 测试 · Testing

```bash
# 后端单元 + 集成测试 · Backend unit + integration tests
cargo test --workspace

# 前端类型检查 · Frontend type check
cd frontend && npm run type-check

# 前端生产构建 · Frontend production build
cd frontend && npm run build
```

CI 当前运行 Rust lint/test、前端 type-check/build 和 OpenAPI 占位任务。前端 ESLint 和严格 OpenAPI 契约检查已在计划中，尚未作为合入门禁。
CI currently runs Rust lint/test, frontend type-check/build, and an OpenAPI placeholder job. Frontend ESLint and strict OpenAPI contract checks are planned but not yet active gates.

## API 文档 · API Documentation

OpenAPI 3.0 规范由 [utoipa](https://github.com/juhaku/utoipa) 自动生成。后端运行时可通过 `/swagger-ui` 访问 Swagger UI。Phase 0 仅注册了部分已实现的路由；严格的前后端契约验证计划在 Phase 1 实施。
OpenAPI 3.0 spec is generated by [utoipa](https://github.com/juhaku/utoipa). Swagger UI is available at `/swagger-ui` when the backend is running. Phase 0 registers only part of the implemented routes; strict backend↔frontend contract validation is planned for Phase 1.

## 文档索引 · Documentation

- [产品需求文档 Product Requirements Document](./AI-PersonalAssistant-PRD.md) — 完整产品规格（中文）· full product spec (Chinese)
- [贡献指南 Contributing Guide](./CONTRIBUTING.md) — 如何贡献 · how to contribute
- [更新日志 Changelog](./CHANGELOG.md) — 版本历史 · version history
- [开发者文档 Developer Docs](./docs/) — 技术文档索引（规划中）· planned technical documentation index

## 许可证 · License

MIT License。详见 [LICENSE](./LICENSE)。
