# 灵枢 · LingShu

[![Version](https://img.shields.io/badge/version-1.0.0-blue)](./CHANGELOG.md)
[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

[English](./README.en.md)

macOS 桌面宠物式 AI 个人助理：常驻桌面的悬浮形象，具备长期记忆（SoulLedger）、可控人格、主动建议（Thought Queue）、分级系统权限，以及 Apple Calendar 日程同步。

---

## 功能概览

| 类别 | 能力 | 状态 |
|------|------|------|
| 桌面宠物 | 透明浮层、可拖拽、贴边浮窗；Tauri `startDragging` 原生拖动 | ✅ Tauri 2 壳就位 |
| SoulLedger | 分层记忆、去重、遗忘衰减（含来源护栏）、人格演化、离线 LLM 语义合并、向量检索 | ✅ 后端完成 |
| Apple Calendar | 自然语言排程、L1 权限确认流、事件 CRUD、EventKit 写入/删除系统日历、external_event_id 回写 | ✅ 后端 + 桥接完成 |
| 权限分级 | L0–L4 逐级系统权限 + 审计日志，权限设置持久化至 PostgreSQL | ✅ 后端完成 |
| 思考队列 | 状态机 + 防打扰抑制 + 生命周期守卫 + 每日维护 + 可解释置信度 | ✅ 后端完成 |
| 集成令牌加密 | AES-256-GCM 静态加密 | ✅ 已实现 |
| 信号埋点 | append-only 事件日志，已接入 memory、personality、chat 等核心路径 | ✅ 已实现 |
| Chat 工具调用 | Native tool-use 循环，支持 Ollama / OpenAI 兼容 / DeepSeek 等多模型 | ✅ 已实现 |
| 角色提示词 | 用户自定义系统角色提示词，集成进对话工作区 | ✅ 已实现 |
| Markdown 渲染 | MessageBubble 支持 Markdown 格式化展示 | ✅ 已实现 |
| LLM 设置持久化 | LLM 配置（模型、max_tokens、context_messages 等）存 PostgreSQL，重启保留 | ✅ 已实现 |

## 项目状态

**v1.0.0 — 核心功能完成，进入桌面/原生集成收尾阶段。** SoulLedger 记忆系统、人格、思考队列、权限分级、向量检索、信号埋点、工具调用、日历 EventKit 写入/删除等均已实现。后端含 **300+ 测试函数（16 项 DB 门控 ignored）**，前端新增 Vitest 套件（stores / lib / 组件，18 项）。

- ✅ Rust + Axum 后端（API、WebSocket、数据库迁移 001–0022）
- ✅ React + TypeScript + Vite 前端（聊天 + 记忆/人格/日历/思绪中心 + 设置 + 角色提示词 + 工作区）
- ✅ PostgreSQL + Redis + Qdrant 开发环境
- ✅ SoulLedger：记忆提取/去重、遗忘衰减 + 来源护栏、人格演化、离线 LLM 语义合并
- ✅ 语义记忆检索（Ollama 嵌入 + Qdrant，带 SQL 回退）
- ✅ 思考队列状态机 + 防打扰 + 生命周期守卫 + 每日维护
- ✅ 权限分级 L0–L4 + 审计日志，权限设置持久化至 PostgreSQL（migration 0022）
- ✅ 集成令牌 AES-256-GCM 静态加密
- ✅ 信号埋点事件层，已接入核心业务路径
- ✅ Tauri 2 桌面壳（main + pet 窗口）+ EventKit 桥接（macOS 14+ `NSCalendarsFullAccessUsageDescription`）
- ✅ Apple Calendar：自然语言解析 + CRUD + L1 确认 + EventKit 写入/删除 + external_event_id 回写
- ✅ Chat：工具调用（tool-use loop）、角色提示词、Markdown 渲染、流式 SSE 响应
- ✅ LLM 设置持久化至 PostgreSQL（模型、max_tokens、context_messages 等，重启保留）
- ✅ 前端 Vitest 单测套件（chatStore / memoryStore / projectStore / MessageBubble / api / tauri / eventkit）

完整产品规格：[AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md)

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust 1.88, Axum 0.7, Tokio, sqlx (PostgreSQL), fred (Redis), reqwest |
| 前端 | React 18, TypeScript, Zustand, Vite 6 |
| 数据 | PostgreSQL 16, Redis 7, Qdrant |
| LLM | Ollama（本地，默认）或任意 OpenAI 兼容端点（DeepSeek 等） |
| 桌面 | Tauri 2 + macOS EventKit |
| CI/CD | GitHub Actions, Docker Compose |

## 环境要求

- [Rust](https://rustup.rs) 1.88 — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ 与 npm
- [Docker](https://docker.com) Desktop（基础设施服务）
- [Ollama](https://ollama.com)（本地 LLM）：需 pull 一个对话模型与嵌入模型 `nomic-embed-text`
- 推荐 macOS 开发（桌面端 / EventKit）；Linux 可用于 CI 与纯后端

## 快速开始

### 1. 启动基础设施

```bash
docker compose -f docker/docker-compose.dev.yml up -d   # PostgreSQL (5432) / Redis (6379) / Qdrant (6333)
```

### 2. 配置环境

```bash
cp config.example.toml config.toml   # gitignored；勿提交真实密钥/个人模型名
cp .env.example .env
```

本地开发将主机名改为 `localhost`/`127.0.0.1`：

```env
DATABASE_URL=postgres://lingshu:lingshu@localhost:5432/lingshu
REDIS_URL=redis://localhost:6379
QDRANT_URL=http://localhost:6333
OLLAMA_URL=http://localhost:11434
SERVER_HOST=127.0.0.1
LLM_DEFAULT_MODEL=<你本机已安装的对话模型>
```

### 3. 安装 CLI 并运行迁移

```bash
# sqlx-cli 锁定 0.8.x 以兼容 rustc 1.88（0.9+ 需要 1.94+）
cargo install sqlx-cli --version '^0.8' --locked --no-default-features --features postgres,rustls
sqlx migrate run --source crates/lingshu-server/migrations
```

### 4. 拉取模型（Ollama）

```bash
ollama pull <你的对话模型>      # 对应 LLM_DEFAULT_MODEL
ollama pull nomic-embed-text   # 嵌入，用于语义记忆检索
```

### 5. 启动后端

```bash
cargo run -p lingshu-server
```

| 端点 | URL |
|------|-----|
| 健康检查 | http://127.0.0.1:8080/api/v1/system/health |
| Swagger UI | http://127.0.0.1:8080/swagger-ui |
| OpenAPI 规范 | http://127.0.0.1:8080/api-docs/openapi.json |

### 6. 启动前端（新终端）

```bash
cd frontend && npm ci && npm run dev      # http://localhost:5173，/api 代理到后端
```

> 默认权限为 L0。日历相关功能需在「权限」面板开启 **L1**，否则 `/api/v1/calendar/*` 返回 403（预期行为）。

### 备选：完整 Docker 模式

```bash
docker compose -f docker/docker-compose.dev.yml --profile full up
```

## 配置

分层加载：先 `config.toml`，再环境变量覆盖（后端启动时自动加载 `.env`）。关键变量：

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | `postgres://lingshu:lingshu@localhost:5432/lingshu` | PostgreSQL 连接 |
| `DATABASE_MAX_CONNECTIONS` | `20` | 连接池大小 |
| `REDIS_URL` | `redis://localhost:6379` | Redis（可选；空则跳过） |
| `QDRANT_URL` | `http://localhost:6333` | Qdrant（可选；空则跳过向量检索） |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama 端点 |
| `LLM_DEFAULT_MODEL` | — | 对话模型（本机已安装） |
| `LLM_EMBED_MODEL` | `nomic-embed-text` | 嵌入模型 |
| `LLM_EMBED_DIM` | `768` | 嵌入维度，需与模型匹配（Qdrant 集合大小） |
| `LLM_API_KEY` | — | 设置后聊天/嵌入走 OpenAI 兼容端点，否则走 Ollama |
| `LLM_API_BASE_URL` | — | OpenAI 兼容基础 URL；设置即启用云端后端 |
| `SERVER_HOST` | `127.0.0.1` | 后端绑定地址（本地单用户，勿暴露公网） |
| `SERVER_PORT` | `8080` | 监听端口 |
| `JWT_SECRET` | — | JWT 签名密钥（生产环境必换） |
| `ENCRYPTION_KEY` | — | 集成令牌静态加密密钥；创建集成前必须配置 |
| `RUST_LOG` | `info,lingshu_server=debug` | 日志级别 |

> 安全提示：后端默认绑 `127.0.0.1`。本地会话端点（`POST /api/v1/auth/local-session`）无密码即发放 owner 令牌，**切勿**改绑 `0.0.0.0` 或暴露到网络。

## 项目结构

```
crates/
  lingshu-server/      Rust Axum 后端（API 路由、数据库、LLM、WebSocket、加密、遥测）
  lingshu-vector/      Qdrant 向量搜索客户端
frontend/              React + TypeScript + Vite 单页应用（含 pet 窗口入口）
  src/components/
    avatar/            桌面宠物（PetWindow、AvatarPlaceholder、AvatarControlPanel）
    chat/              聊天窗口（ChatWindow、ChatInput、MessageBubble、ChatSettings、RolePromptSettings）
    layout/            应用布局（AppLayout）
    memory/            记忆中心（MemoryCenter）
    personality/       人格中心（PersonalityCenter）
    projects/          项目管理（ProjectManager）
    settings/          权限设置（PermissionSettings）
    thoughts/          思考队列（ThoughtQueue）
    workspace/         工作区（WorkspacePage，含日历区块 CalendarSection）
  src/stores/          Zustand 状态管理（chatStore、memoryStore、projectStore）
  src/lib/             API 客户端、EventKit 桥接、Tauri 工具函数
src-tauri/             Tauri 2 桌面壳（独立 Cargo workspace）：main + pet 窗口，EventKit 桥接
docker/                Docker Compose 开发环境与配置
docs/                  设计决策、MVP 状态等文档
poc/                   技术验证（AGE 图、LLM 流式、Qdrant）
```

SoulLedger 关键模块（`crates/lingshu-server/src/llm/`）：`memory.rs`（抽取/去重）、`forgetting.rs`（衰减 + 来源护栏）、`personality.rs`（人格演化）、`consolidation.rs`（离线语义合并）、`thoughts.rs`（主动建议）、`dedup.rs`、`prompts.rs`、`semantic.rs`（语义检索）、`ollama.rs`（Ollama 适配）、`client.rs`（Ollama/OpenAI + 嵌入 + 工具调用）。

后端路由（`crates/lingshu-server/src/routes/`）：`chat.rs`（对话 + 工具调用）、`calendar.rs`、`memories.rs`、`personality.rs`、`thoughts.rs`、`permissions.rs`、`settings.rs`（LLM 设置 + 角色提示词）、`integrations.rs`、`signals.rs`、`audit.rs`、`conversations.rs`、`projects.rs`、`tasks.rs`、`users.rs`、`auth.rs`、`system.rs`、`sessions.rs`、`project_members.rs`、`task_dependencies.rs`。

## Tauri 桌面应用

两个窗口：**main**（完整控制面板，1200×800）与 **pet**（frameless + transparent + always-on-top 桌面宠物，200×260）。后端作为独立进程运行，前端经 `127.0.0.1:8080` 调用；无 Tauri 环境（浏览器）时，EventKit 等桌面专属能力优雅降级。

```
┌─ src-tauri/ (Tauri 2, 独立 Cargo workspace) ─────────────────────┐
│  main.rs          ── 桌面入口，启动 main + pet 两个窗口              │
│  eventkit.rs      ── EventKit 桥接 (macOS only)                    │
└──────────────────────────────────────────────────────────────────┘
         │ Tauri invoke (仅桌面)          │ HTTP (127.0.0.1:8080)
         ▼                                ▼
┌─ frontend/ (Vite + React) ──┐   ┌─ lingshu-server (Axum) ────────┐
│  main.tsx → App.tsx          │   │  API routes, PostgreSQL, LLM   │
│  pet.tsx  → PetWindow        │   │  作为独立进程运行               │
│  lib/tauri.ts (优雅降级)      │   └────────────────────────────────┘
└──────────────────────────────┘
```

```bash
# 浏览器模式（无需 Tauri）
cargo run -p lingshu-server &
cd frontend && npm run dev

# Tauri 桌面模式（需 macOS + Xcode 命令行工具）
cargo run -p lingshu-server &
cd frontend && ./node_modules/.bin/tauri dev
./node_modules/.bin/tauri build     # → src-tauri/target/release/bundle/macos/LingShu.app
```

**Apple Calendar 写入**：确认事件（L1）后，桌面端通过 EventKit Tauri 命令写入系统日历并回写 `external_event_id`；macOS 14+ 已配置 `NSCalendarsFullAccessUsageDescription`，需在系统设置授予日历权限。

## 测试

```bash
# 后端测试（300+ 测试函数，16 项 DB 门控 ignored）
cargo test --workspace

# 前端单测（Vitest）+ 类型检查 + 构建
cd frontend && npm run test && npm run type-check && npm run build

# Tauri 构建（macOS）
cd src-tauri && cargo build

# CI 完整检查（与 GitHub Actions 一致）
SQLX_OFFLINE=true cargo fmt --all -- --check
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
DATABASE_URL=... REDIS_URL=... cargo test --all
```

CI（GitHub Actions）：Rust fmt/clippy/test、前端 type-check/build、Docker 镜像构建（GHCR）。

## API 文档

OpenAPI 3.0 由 [utoipa](https://github.com/juhaku/utoipa) 生成，覆盖全部 **65 个已注册接口**（系统、认证、用户、LLM 设置、对话、会话、记忆、人格、思绪、日历、权限、集成、信号、审计、项目管理等）。后端运行时访问 `/swagger-ui`。

## 已知局限

- WebSocket 仍为回声桩，主动建议的实时推送待实现。
- Thought Queue 的「跨会话实体计数」触发器为后续增强。
- 前端 ESLint（ESLint 9 flat config）与 OpenAPI 契约门禁（committed spec + 生成客户端 + diff gate）仍待 Phase 1。

## 文档索引

- [产品需求文档](./AI-PersonalAssistant-PRD.md) — 完整产品规格
- [SoulLedger 设计决策](./docs/soulledger-design-decisions.md)
- [MVP 完成度](./docs/mvp-status.md)
- [贡献指南](./CONTRIBUTING.md) · [更新日志](./CHANGELOG.md)

## 许可证

MIT — 详见 [LICENSE](./LICENSE)。
