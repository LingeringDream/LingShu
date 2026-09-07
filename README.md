# 灵枢 · LingShu

> 中国国际大学生创新大赛（2026）产业赛道 · 中兴通讯第 1529 题可运行原型。完整源码、部署说明、架构、创新点、数据集与可复现实验见 [竞赛交付索引](docs/competition/README.md)。

[![Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2FLingeringDream%2FLingShu%2Fmain%2FCargo.toml&query=%24.workspace.package.version&label=version&prefix=v&color=blue)](./CHANGELOG.md)
[![License](https://img.shields.io/badge/license-Commercial-red)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

[English](./README.en.md)

灵枢是连接不同 AI 工具的个人、团队与企业记忆层，同时提供桌面宠物式 AI 助手。用户既可以直接使用灵枢客户端，也可以通过 MCP 让 Claude、Cursor、Codex 等工具共享同一套记忆，或通过当前 HTTP API 将记忆能力嵌入其他产品；Python / TypeScript SDK 属于后续规划。完整的产品定位、版本权益与付费边界见[产品定位与版本体系](docs/product-positioning.md)。

> **灵枢不是另一个 AI。灵枢是连接所有 AI 的记忆层。**

桌面客户端具备长期记忆（SoulLedger）、可控人格、主动建议（Thought Queue）、分级系统权限和平台适配的日程能力；记忆内核已扩展出面向多智能体协同的动态记忆沉淀、冲突治理与自适应检索原型。

---

## 产业赛道作品

| 项目 | 内容 |
|------|------|
| 企业 | 中兴通讯股份有限公司 |
| 正式命题 | **面向智能体自主进化的动态记忆沉淀与自适应检索系统** |
| 命题编号 | 1529 |
| 原型名称 | **灵枢 · Memory Evolution Lab** |
| 作品形态 | Rust/Axum 后端 + React/Tauri 可视化原型 + PostgreSQL/Qdrant 数据层 |

原型已经打通赛题要求的核心链路：

- **三层异构记忆**：`episode` 原始交互片段、`fact` 结构化事实、`experience` 通用经验，支持时序索引、场景标签与来源追溯。
- **多 Agent Memory Gateway**：提供私有、定向共享、全局公共空间，以及按记忆授权、权限隔离和访问审计。
- **动态冲突治理**：按业务作用域、可见权限和有效时间识别矛盾；并发写入在事实级事务锁内处理，不同主张保留为争议，人工裁决后标记被替代记忆并保留记录。详见[并发写入协议](docs/memory-write-consistency.md)。
- **自适应检索**：融合语义、词法、时序、可信度、频次和重要性进行重排，支持与传统静态 RAG 使用同一数据和接口对照。
- **可运行评测**：内置 660 轮记忆、300 个查询的自建回归集 v3，并输出完整模型、静态基线、消融实验和阈值判定结果；早期 60 轮 / 12 查询的 v1 只作历史归档。
- **可视化演示**：在侧栏进入“记忆进化实验台”，可查看分层记忆、权限空间、冲突事件、检索评分和基线对比。

当前可复现结果分两条线：自建合成回归集 v3 与公开数据集 LoCoMo。两者均为离线评测，公开集的检索策略尚未接入在线服务。

**自建回归集 v3**（15 个项目 × 20 类属性，300 个查询；150 个键各经历两次变更）

| 方案 | 全量 Top-1（300） | 测试 Top-1（100） | 测试事实变更（50） |
|---|---:|---:|---:|
| Static RAG | 64.33% | 62.00% | 30.00% |
| LingShu Full | 91.67% | 94.00% | 96.00% |
| 相似度＋状态过滤（词法回退） | 99.00% | 99.00% | 100.00% |

> 状态过滤使用 oracle 标签，与较旧的事实版本高度相关，**不能等同于真实的自动治理能力**；查询共享 20 个问题模板，不是 300 条独立人工标注。自动治理当前未达标，详见[自动治理审计](docs/competition/auto-governance-audit.md)。

**公开数据集 LoCoMo**（10 组对话、1,977 个 QA，统一 Top-5）

| 方案 | 全量 R@5 | 测试 R@5 | 测试 P@5 |
|---|---:|---:|---:|
| Static 向量 | 26.82% | 25.15% | 5.93% |
| BM25 正文＋说话人 | 52.31% | 51.82% | 12.13% |

同预算端到端问答（分层样本 100 题）总体得分 Static 28.24 → BM25 37.20，配对差异 +8.96 分。该收益首先证明**强词法基线的价值，不能包装成进化记忆相对 BM25 的优势**。

```bash
# 无需启动数据库，现场重算静态基线、自适应检索、消融与任务完成度
cargo run -p lingshu-eval --release -- --json output/eval_results.json
```

> **证据边界：**早期 v1 结果（60 轮记忆 / 12 查询）的指标口径与「五项全部达标」结论已纠正并撤回，仅供历史追踪，见 [v1 归档报告](docs/competition/experiment-report-v1-archived.md)。评测仍缺生产检索接入、真实用户样本、权重路由、百万级并发压测与真实多 Agent 协作。详见[实验报告](docs/competition/experiment-report.md)。

## 功能概览

| 类别 | 能力 | 状态 |
|------|------|------|
| MCP 记忆服务 | stdio MCP Server，6 个工具（`lingshu_recall` / `lingshu_store` / `lingshu_grant` / `lingshu_conflicts` / `lingshu_resolve_conflict` / `lingshu_stats`），让 Claude、Cursor、Codex 等共享同一记忆层 | ✅ 已实现 |
| 并发写入与裁决 | 事实级事务锁、业务作用域隔离、不同主张保留为争议、人工裁决保留记录与审计 | ✅ 已实现 |
| 桌面宠物 | 透明浮层、可拖拽、贴边浮窗；Tauri `startDragging` 原生拖动 | ✅ Tauri 2 壳就位 |
| SoulLedger | 分层记忆、去重、遗忘衰减（含来源护栏）、人格演化、离线 LLM 语义合并、向量检索 | ✅ 后端完成 |
| Memory Evolution Lab | 三层异构记忆、多 Agent 权限空间、冲突治理、自适应检索、静态 RAG 基线与消融评测 | ✅ 全链路原型完成 |
| Apple Calendar | 自然语言排程、L1 权限确认流、事件 CRUD、EventKit 写入/删除系统日历、external_event_id 回写 | ✅ 后端 + 桥接完成 |
| 权限分级 | L0–L4 逐级系统权限 + 审计日志，权限设置持久化至 PostgreSQL | ✅ 后端完成 |
| 思考队列 | 状态机 + 防打扰抑制 + 生命周期守卫 + 每日维护 + 可解释置信度 | ✅ 后端完成 |
| 集成令牌加密 | AES-256-GCM 静态加密 | ✅ 已实现 |
| 信号埋点 | append-only 事件日志，已接入 memory、personality、chat 等核心路径 | ✅ 已实现 |
| Chat 工具调用 | Native tool-use 循环，支持 Ollama / OpenAI 兼容 / DeepSeek 等多模型 | ✅ 已实现 |
| 联网搜索 | 对话内搜索公开网页，返回标题、摘要和来源链接；由独立 L1 权限控制 | ✅ 已实现 |
| 角色提示词 | 用户自定义系统角色提示词，集成进对话工作区 | ✅ 已实现 |
| Markdown 渲染 | MessageBubble 支持 Markdown 格式化展示 | ✅ 已实现 |
| LLM 设置持久化 | LLM 配置（模型、max_tokens、context_messages 等）存 PostgreSQL，重启保留 | ✅ 已实现 |
| 云账户与版本权益 | 可选云账户、网页登录、设备撤销、订阅概览与下载中心；Free / Pro / Team 权益由云端统一计算 | ✅ 已部署；Pro 端到端加密同步、跨设备连续 MCP 与高级记忆治理已开放 |
| 团队工作区 | 有效 Team 订阅可创建工作区；受邀 Free 用户可加入，团队内按角色和记忆授权执行 | ✅ 已实现；团队 MCP、统一账单尚未开放 |

## 项目状态

**核心功能与跨平台桌面基础设施已完成，当前进入竞赛证据完善和云账户配套收尾阶段。** SoulLedger 记忆系统、Memory Gateway、冲突治理、自适应检索、人格、思考队列、权限分级、工具调用、日历平台适配等均已实现。当前 Rust workspace 为 **408 passed、19 ignored**，前端为 **29 个测试文件、132 项测试通过**，云账户 Worker 为 **55 项测试通过**。

- ✅ Rust + Axum 后端（API、WebSocket、29 个数据库迁移）
- ✅ MCP 记忆服务（`lingshu-mcp` + `lingshu-mcp-core`）：stdio 传输，6 个工具，与 REST 共用同一套 Memory Gateway、权限与审计
- ✅ 并发写入与裁决：事实级事务锁、业务作用域 `context_scope` 隔离、不同主张保留为 `disputed`、人工裁决保留记录
- ✅ 版本权益：Free / Pro / Team 三档由云端统一计算，客户端只展示结果，受限操作仍由后端校验
- ✅ React + TypeScript + Vite 前端（聊天 + 记忆/人格/日历/思绪中心 + 设置 + 角色提示词 + 工作区）
- ✅ PostgreSQL + Redis + Qdrant 开发环境
- ✅ SoulLedger：记忆提取/去重、遗忘衰减 + 来源护栏、人格演化、离线 LLM 语义合并
- ✅ Memory Evolution Lab：三层记忆协议、多 Agent Gateway、冲突治理、自适应检索、静态基线与消融评测
- ✅ 语义记忆检索（Ollama 嵌入 + Qdrant，带 SQL 回退）
- ✅ 思考队列状态机 + 防打扰 + 生命周期守卫 + 每日维护
- ✅ 权限分级 L0–L4 + 审计日志，权限设置持久化至 PostgreSQL（migration 0022）
- ✅ 集成令牌 AES-256-GCM 静态加密
- ✅ 信号埋点事件层，已接入核心业务路径
- ✅ Tauri 2 桌面壳（main + pet 窗口）+ EventKit 桥接（macOS 14+ `NSCalendarsFullAccessUsageDescription`）
- ✅ Apple Calendar：macOS 使用 EventKit；Windows、Linux 和 Web 使用 ICS 导出；均支持自然语言解析、CRUD 与 L1 确认
- ✅ macOS / Windows 独立版：Sidecar 内置 PostgreSQL 运行时，安装包可独立启动，不依赖 Docker 或外部数据库
- ✅ 云账户：网页登录 PKCE、设备会话、订阅概览和下载中心；Pro 提供端到端加密同步、跨设备连续 MCP 与高级记忆治理
- ✅ Chat：工具调用（tool-use loop）、角色提示词、Markdown 渲染、流式 SSE 响应
- ✅ 联网搜索：独立 L1 权限开关、网页结果解析、来源链接保留、超时与响应大小限制
- ✅ LLM 设置持久化至 PostgreSQL（模型、max_tokens、context_messages 等，重启保留）
- ✅ 前端 Vitest 单测套件（chatStore / memoryStore / projectStore / MessageBubble / api / tauri / eventkit）

完整产品规格：[AI-PersonalAssistant-PRD.md](./AI-PersonalAssistant-PRD.md)

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust 1.88, Axum 0.7, Tokio, sqlx (PostgreSQL), fred (Redis), reqwest |
| 前端 | React 18, TypeScript, Zustand, Vite 6 |
| 集成 | MCP（stdio / JSON-RPC 2.0）：Claude、Cursor、Codex、Cline、VS Code、Zed 等 |
| 数据 | PostgreSQL 16, Redis 7, Qdrant |
| LLM | Ollama（本地，默认）或任意 OpenAI 兼容端点（DeepSeek 等） |
| 桌面 | Tauri 2 + macOS EventKit；Windows/Linux/Web 使用平台降级和 ICS 导出 |
| CI/CD | GitHub Actions, Docker Compose |

## 环境要求

- [Rust](https://rustup.rs) 1.88 — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ 与 npm
- [Docker](https://docker.com) Desktop（基础设施服务）
- [Ollama](https://ollama.com)（本地 LLM）：需 pull 一个对话模型与嵌入模型 `nomic-embed-text`
- macOS 14+ Apple Silicon 可使用 EventKit 与自包含安装包；Windows 10/11 x64 支持独立安装包；Linux 可用于 CI 与纯后端

## 快速开始

### Windows 一键启动（推荐用于演示）

安装并启动 Docker Desktop 后，直接双击仓库根目录的：

```text
启动灵枢竞赛版.cmd
```

启动器会自动构建并启动 PostgreSQL、Redis、Qdrant、Rust 后端和 React 前端，待服务就绪后直接打开“动态记忆引擎”控制台。首次构建可能需要几分钟，后续会复用 Docker 缓存。

如果系统没有自动唤起默认浏览器，可双击 `打开灵枢控制台.cmd`，或访问 `http://127.0.0.1:5173/?section=memory-lab`。

演示结束后双击 `停止灵枢开发版.cmd`。停止操作只关闭开发容器，不会删除数据库卷和已有记忆数据。

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

## MCP 接入

MCP 是灵枢的核心入口之一。后端启动后，任何支持 MCP 的 AI 工具都能把灵枢当作共享长期记忆层来读写，无需打开灵枢客户端。

```bash
cargo build --release -p lingshu-mcp     # → target/release/lingshu-mcp
```

在 AI 工具的 MCP 配置中加一个 server（以 Claude Desktop 的 `claude_desktop_config.json` 为例）：

```json
{
  "mcpServers": {
    "lingshu": {
      "command": "/绝对路径/target/release/lingshu-mcp",
      "env": {
        "LINGSHU_BASE_URL": "http://127.0.0.1:8080"
      }
    }
  }
}
```

| 工具 | 作用 |
|------|------|
| `lingshu_recall` | 检索记忆，可带业务作用域与记忆层 |
| `lingshu_store` | 写入记忆（`episode` / `fact` / `experience`） |
| `lingshu_grant` | 把某条记忆授权给其他 Agent |
| `lingshu_conflicts` | 列出当前争议 |
| `lingshu_resolve_conflict` | 人工裁决争议 |
| `lingshu_stats` | 记忆统计 |

- 鉴权优先读环境变量 `LINGSHU_TOKEN`；未设置时，`lingshu-mcp` 启动时调用本地 `local-session` 自取 owner 令牌。
- 服务以 stdio 通信，**所有日志写 stderr**，stdout 只走 JSON-RPC，不要把日志重定向到 stdout。
- 写入后会直接返回状态。**「已记录」不等于「已确认」**：不同主张会保留为争议，检索时显式提示，不能靠重复提交更高置信度来解除。详见[并发写入与冲突治理](docs/memory-write-consistency.md)。

> 本机 MCP 属于免费版能力。Pro 用户在各设备完成端到端加密同步后，本机 MCP 会访问同一套连续记忆；同步口令始终留在客户端。

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
  lingshu-mcp/         MCP 记忆服务入口（stdio 传输，stdout 只走 JSON-RPC）
  lingshu-mcp-core/    MCP 协议层与 6 个工具实现（recall / store / grant / conflicts / resolve / stats）
  lingshu-eval/        离线评测：静态基线、消融、任务完成度与治理指标
frontend/              React + TypeScript + Vite 单页应用（含 pet 窗口入口）
  src/components/
    avatar/            桌面宠物（PetWindow、AvatarPlaceholder、AvatarControlPanel）
    chat/              聊天窗口（ChatWindow、ChatInput、MessageBubble、ChatSettings、RolePromptSettings）
    layout/            应用布局（AppLayout）
    memory/            记忆中心（MemoryCenter）与记忆进化实验台（MemoryEvolutionLab）
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

SoulLedger 关键模块（`crates/lingshu-server/src/llm/`）：`memory.rs`（抽取/去重）、`evolutionary_memory.rs`（记忆协议、权限判定、冲突评分与自适应重排）、`forgetting.rs`（衰减 + 来源护栏）、`personality.rs`（人格演化）、`consolidation.rs`（离线语义合并）、`thoughts.rs`（主动建议）、`dedup.rs`、`prompts.rs`、`semantic.rs`（语义检索）、`client.rs`（Ollama/OpenAI + 嵌入 + 工具调用）。

后端路由（`crates/lingshu-server/src/routes/`）：`chat.rs`（对话 + 工具调用）、`memories.rs`、`memory_gateway.rs`（Agent、分层空间、权限、冲突与检索）、`memory_benchmark.rs`（基线、完整模型与消融评测）、`calendar.rs`、`personality.rs`、`thoughts.rs`、`permissions.rs`、`settings.rs`（LLM 设置 + 角色提示词）、`integrations.rs`、`signals.rs`、`audit.rs`、`conversations.rs`、`projects.rs`、`tasks.rs`、`users.rs`、`auth.rs`、`system.rs`、`sessions.rs`、`project_members.rs`、`task_dependencies.rs`。

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
# 一键启动（推荐）：基础设施 + 迁移 + 后端 + 前端，浏览器模式无需编译 Tauri
./scripts/dev.sh          # 或 npm run dev；就绪后浏览器打开 http://localhost:5173

# 浏览器模式（手动分步，无需 Tauri）
cargo run -p lingshu-server &
cd frontend && npm run dev

# Tauri 桌面模式（需 macOS + Xcode 命令行工具）
cargo run -p lingshu-server &
cd frontend && ./node_modules/.bin/tauri dev
./node_modules/.bin/tauri build     # → src-tauri/target/release/bundle/macos/LingShu.app
```

**日历写入/导出**：确认事件（L1）后，macOS 桌面端通过 EventKit 写入系统日历并回写 `external_event_id`；Windows、Linux 和 Web 通过 ICS 导出。macOS 14+ 已配置 `NSCalendarsFullAccessUsageDescription`。

## 测试

```bash
# Rust workspace 测试（当前 408 passed、19 ignored）
cargo test --workspace

# 前端单测（当前 29 个测试文件、132 项测试）+ lint + 类型检查 + 构建
cd frontend && npm run test && npm run lint && npm run type-check && npm run build

# Tauri 构建（macOS）
cd src-tauri && cargo build

# CI 完整检查（与 GitHub Actions 一致）
SQLX_OFFLINE=true cargo fmt --all -- --check
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
DATABASE_URL=... REDIS_URL=... cargo test --all
```

CI（GitHub Actions）：Rust fmt/clippy/test、前端 lint/type-check/build、Windows 安装包构建、OpenAPI 契约检查和 Docker 镜像构建（GHCR）。

## API 文档

OpenAPI 3.1 由 [utoipa](https://github.com/juhaku/utoipa) 生成，当前覆盖 **70 条路径、107 个操作**（系统、认证、用户、LLM 设置、对话、会话、记忆网关、MCP、团队、评测、人格、思绪、日历、权限、集成、信号、审计、项目管理等）。后端运行时访问 `/swagger-ui`；提交的 `openapi.json` 由契约测试校验。

## 已知局限

- **自动治理未达标**：事实状态自动判定的 Macro-F1 为规则 44.74% / LLM 22.97%，错误淘汰率 23–27%，远高于 <5% 目标。当前依赖保守的争议保留与人工裁决，不自动判定事实真伪。详见[自动治理审计](docs/competition/auto-governance-audit.md)。
- **Pro 已开放**：端到端加密云同步、跨设备连续 MCP、高级记忆治理和检索权重自定义均可用。团队 MCP 和按席位账单仍属规划能力；有效 Team 订阅已可创建团队工作区，受邀 Free 用户可加入。详见[版本权益实施说明](docs/version-entitlements.md)。
- 桌面安装包已携带 MCP 二进制，设置页可为支持的客户端一键写入配置；部分客户端仍需手动粘贴配置，源码运行仍需先构建 `lingshu-mcp`。
- 竞赛评测已增加 LoCoMo 强词法基线、按官方类别规则计分的本地问答、300 查询合成集与消融；尚缺生产检索接入、真实样本、权重路由、百万级并发压测与真实多 Agent 协作。旧版指标口径已纠正，详见[实验报告](docs/competition/experiment-report.md)。
- 前端暂未提供生成式 API 客户端；当前 OpenAPI 门禁校验代码生成结果与提交的 `openapi.json` 一致性。

## 文档索引

- [产品定位与版本体系](./docs/product-positioning.md) — 客户端、MCP、HTTP API 现有入口、SDK 规划与 Free / Pro / Team 边界
- [版本权益实施说明](./docs/version-entitlements.md) — 各档当前已开放与尚未开放的能力
- [并发写入与冲突治理](./docs/memory-write-consistency.md) — MCP 与 REST 共用的写入协议、争议与裁决
- [产品需求文档](./AI-PersonalAssistant-PRD.md) — 完整产品规格（历史设计稿，不代表当前定位）
- [中兴 1529 竞赛交付索引](./docs/competition/README.md) — 命题映射、部署演示、架构、创新点与实验报告
- [竞赛实验报告](./docs/competition/experiment-report.md) — LoCoMo 与自建集的检索、问答、消融与证据边界
- [自动治理审计](./docs/competition/auto-governance-audit.md) — 事实状态自动判定的当前结果与后续协议
- [SoulLedger 设计决策](./docs/soulledger-design-decisions.md)
- [MVP 完成度](./docs/mvp-status.md)
- [贡献指南](./CONTRIBUTING.md) · [更新日志](./CHANGELOG.md)

## 许可证

商业专有软件，未经书面商业授权不得使用、复制、修改或分发。详见 [LICENSE](./LICENSE)。
