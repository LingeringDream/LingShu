# 灵枢 · LingShu

> 中国国际大学生创新大赛（2026）产业赛道 · 中兴通讯第 1529 题可运行原型。完整源码、部署说明、架构、创新点、数据集与可复现实验见 [竞赛交付索引](docs/competition/README.md)。

[![Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2FLingeringDream%2FLingShu%2Fmain%2FCargo.toml&query=%24.workspace.package.version&label=version&prefix=v&color=blue)](./CHANGELOG.md)
[![License](https://img.shields.io/badge/license-Commercial-red)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88-orange)](./rust-toolchain.toml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)](./CONTRIBUTING.md)

[English](./README.en.md)

灵枢是连接不同 AI 工具的长期记忆层，当前软件面向个人与小团队，同时提供桌面宠物式 AI 助手。用户可以直接使用灵枢客户端，也可以通过 MCP 让支持该协议的 AI 工具按权限读写同一套记忆。企业版与 Embedded / OEM 属于独立产品规划，API / SDK 产品化不作为当前软件已交付能力。完整定位见[产品定位与版本体系](docs/product-positioning.md)。

> **灵枢不是另一个 AI。灵枢是连接所有 AI 的记忆层。**

桌面客户端具备长期记忆（SoulLedger）、可控人格、主动建议（Thought Queue）、分级系统权限和平台适配的日程能力；记忆内核已扩展出面向多智能体协同的动态记忆沉淀、冲突治理与自适应检索原型。

---

## 当前版本与使用入口

当前源码版本为 **1.6.1**。安装包版本、平台和校验值以账户中心的[下载清单](lingshu-cloud/public/downloads.json)及下载页面为准；源码版本不代表所有平台已同步发布。

| 入口 | 用途 | 运行方式 |
|------|------|----------|
| 个人客户端 | 日常聊天、记忆管理、桌宠、AI 工具接入、团队工作区 | Windows x64 / macOS Apple Silicon 桌面安装包；内置 PostgreSQL 与后端、MCP sidecar，无需 Docker 或外置数据库 |
| 开发版 / 竞赛版 | 动态记忆引擎、Agent 治理、检索对照与评测演示 | Docker 全栈或源码启动，在浏览器访问 |
| 云账户与团队网页 | 登录、设备管理、订阅、下载、团队成员与共享记忆 | 云账户服务；与桌面本地会话隔离 |

当前工作树已将 Memory Console 限定在开发版 / 竞赛版，桌面构建使用 `npm run build:client`；开发构建使用 `npm run build`。这属于发行形态区分，与 Free / Pro / Team 订阅档位独立。模型仍需配置本地 Ollama 或自备 OpenAI 兼容服务。

## Free / Pro / Team 权益

| 档位 | 当前可用 | 尚未开放 |
|------|----------|----------|
| Free | 本地聊天、动态记忆、本地 MCP、备份；可接受团队邀请 | — |
| Pro | Free 本地能力 | 加密云同步、跨设备 MCP、高级记忆治理 |
| Team | Free 本地能力、创建团队工作区；成员按角色和授权访问团队记忆 | Pro 规划能力、按席位账单 |

免费账号受邀后显示“团队版 Team（受邀成员）”，可使用所属团队；创建新团队仍需账号自身有效的 Team 订阅。权益由服务端校验，订阅失效不删除本地记忆或关闭本地 MCP，现有团队也不会因此自动删除。团队共享记忆与个人跨设备同步是两项独立能力，后者尚未开放。详见[权益实施说明](docs/version-entitlements.md)。

## 产业赛道作品

| 项目 | 内容 |
|------|------|
| 企业 | 中兴通讯股份有限公司 |
| 正式命题 | **面向智能体自主进化的动态记忆沉淀与自适应检索系统** |
| 命题编号 | 1529 |
| 原型名称 | **灵枢——面向多智能体协同的动态记忆与自适应检索系统** |
| 作品形态 | Rust/Axum 后端 + React/Tauri 可视化原型 + PostgreSQL/Qdrant 数据层 |

原型已经打通赛题要求的核心链路：

- **三层异构记忆**：`episode` 原始交互片段、`fact` 结构化事实、`experience` 通用经验，支持时序索引、场景标签与来源追溯。
- **多 Agent Memory Gateway**：提供私有、定向共享、全局公共空间，以及按记忆授权、权限隔离和访问审计。
- **动态冲突治理**：按业务作用域、可见权限和有效时间识别矛盾；并发写入在事实级事务锁内处理，不同主张保留为争议，人工裁决后标记被替代记忆并保留记录。详见[并发写入协议](docs/memory-write-consistency.md)。
- **自适应检索**：融合语义、词法、时序、可信度、频次和重要性进行重排，支持与传统静态 RAG 使用同一数据和接口对照。
- **可运行评测**：独立 `lingshu-eval` 评测器复用生产排序代码；保留 v1/v2 回归集，新增 v3 合成集（660 轮、300 查询）及 LoCoMo 离线检索与问答实验。
- **可视化演示**：开发版 / 竞赛版提供 Memory Console、Agent Governance、Benchmark & Integration 三视图，可查看分层记忆、权限空间、冲突事件、检索评分和基线对比。

截至 2026-09-06 的离线实验摘要（详见[实验报告](docs/competition/experiment-report.md)）：

| 数据与指标 | Static 基线 | 对照方案 | 结果 |
|------------|------------:|----------|-----:|
| v3 测试集 100 查询 Top-1 | 62.00% | LingShu Full | 94.00% |
| v3 测试集 50 个事实变更查询 Top-1 | 30.00% | LingShu Full | 96.00% |
| LoCoMo 1,977 QA 证据 R@5 | 26.82% | BM25 正文＋说话人 | 52.31% |
| LoCoMo 测试部分分层 100 题问答得分 | 28.24 | BM25 检索＋同模型生成 | 37.20 |

```powershell
# 在仓库根目录运行；使用已提交数据，无需启动数据库或模型
cargo run -p lingshu-eval --release -- --zte-dataset datasets/zte_memory_v3_test.json --json output/zte_v3_test_results.json
```

- v3 的记忆状态为 oracle 真值，查询共享模板，衡量治理后的检索；相似度＋状态过滤的测试 Top-1 为 **99%**，优于完整多特征策略的 94%，不能据此宣称已学得最优权重或完成自动治理。
- LoCoMo 采用回溯式划分；100 题问答是类别均衡混合得分（含拒答准确率），不是全量官方 F1。BM25 / RRF / 抽取式事实候选尚未接入在线服务；收益证明强词法基线的价值。
- 标准 P@K 为命中不同相关证据数 / K；旧 `hits / min(K, gold数)` 已改名 `capped_recall`。旧 31% / 32% 为 yes/no judge accuracy，旧“五项全部达标”及不等预算 Token 节省结论不再使用。
- [自动治理 v4 审计](docs/competition/auto-governance-audit.md)属于诊断实验：129 轮、73 查询，标签与构造线索仍待复核，不能替代固定检索下的自动治理验收。真实多 Agent 协作、百万级并发和学习权重仍待验证。

## 功能概览

| 类别 | 能力 | 状态 |
|------|------|------|
| 桌面宠物 | 透明浮层、可拖拽、贴边浮窗；Tauri `startDragging` 原生拖动 | ✅ Tauri 2 壳就位 |
| SoulLedger | 分层记忆、去重、遗忘衰减（含来源护栏）、人格演化、离线 LLM 语义合并、向量检索 | ✅ 后端完成 |
| Memory Evolution Lab | 三层异构记忆、多 Agent 权限空间、冲突治理、自适应检索、静态 RAG 基线与消融评测 | ✅ 可运行原型；自动治理仍待验收 |
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
| MCP 与团队记忆 | 本地 MCP、AI 工具配置、团队成员与角色、Agent 凭证、共享记忆和审计 | ✅ 已实现 |
| 云账户与设备会话 | 可选云账户、网页登录、设备撤销、订阅概览与下载中心 | ✅ 已部署；个人加密云同步暂未开放 |

## 项目状态

**当前源码已具备本地记忆、MCP、团队工作区和竞赛演示原型，重点转向评测证据完善。** 代码实现与实验验收分开记录；测试结果以对应提交的 CI 或本地运行输出为准，不沿用旧版本的测试数量。

- ✅ Rust + Axum 后端（API、WebSocket、团队工作区与并发写入迁移）
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
- ✅ Windows / macOS 独立版：Sidecar 内置 PostgreSQL 运行时，安装包可独立启动，不依赖 Docker 或外部数据库
- ✅ 云账户：网页登录 PKCE、设备会话、订阅概览和下载中心；个人加密云同步与跨设备 MCP 暂未开放；团队工作区已实现
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
| 数据 | PostgreSQL 16, Redis 7, Qdrant |
| LLM | Ollama（本地，默认）或任意 OpenAI 兼容端点（DeepSeek 等） |
| 桌面 | Tauri 2 + macOS EventKit；Windows/Linux/Web 使用平台降级和 ICS 导出 |
| CI/CD | GitHub Actions, Docker Compose |

## 源码开发环境要求

- [Rust](https://rustup.rs) 1.88 — `rustup default 1.88`
- [Node.js](https://nodejs.org) 22+ 与 npm
- [Docker](https://docker.com) Desktop（基础设施服务）
- [Ollama](https://ollama.com)（本地 LLM）：需 pull 一个对话模型与嵌入模型 `nomic-embed-text`
- macOS 14+ Apple Silicon 可使用 EventKit；Windows 10/11 x64 支持独立安装包；Linux 可用于 CI 与纯后端

## 快速开始

### Windows 一键启动（开发版 / 竞赛演示）

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

## MCP 接入与团队记忆

先启动灵枢本地服务，再在客户端设置的「AI 工具」面板选择目标工具和传输方式，生成配置；支持的桌面工具可直接写入配置文件，其余工具手动复制。桌面安装包包含 `lingshu-mcp`，源码开发可运行 `cargo build -p lingshu-mcp --release` 生成它。

本地 stdio MCP 默认连接 `http://127.0.0.1:8080`，支持 `LINGSHU_BASE_URL` 和 `LINGSHU_TOKEN`；未提供令牌时通过本地会话获取身份。提供 `lingshu_recall`、`lingshu_store`、`lingshu_grant`、`lingshu_conflicts`、`lingshu_resolve_conflict`、`lingshu_stats` 六个工具，按 Agent 身份与记忆权限执行。

团队工作区支持创建/加入、成员角色、邀请、Agent 凭证、共享记忆和访问审计。远程接入需使用对应服务的地址与凭证，不能把本地免密会话直接暴露到网络。并发写入通过事实级事务锁协调，冲突主张保留来源，裁决后再标记被替代项，详见[并发写入协议](docs/memory-write-consistency.md)。

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
  lingshu-mcp/         stdio MCP 服务入口
  lingshu-mcp-core/    共用 MCP 协议与记忆工具
  lingshu-eval/        独立离线检索与消融评测器
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
lingshu-cloud/         云账户、订阅权益与团队网页（Cloudflare Worker）
eval/                  Python 检索、问答与自动治理实验
datasets/              自建回归集及公开评测数据
docker/                Docker Compose 开发环境与配置
docs/                  设计决策、MVP 状态等文档
poc/                   技术验证（AGE 图、LLM 流式、Qdrant）
```

SoulLedger 关键模块（`crates/lingshu-server/src/llm/`）：`memory.rs`（抽取/去重）、`evolutionary_memory.rs`（记忆协议、权限判定、冲突评分与自适应重排）、`forgetting.rs`（衰减 + 来源护栏）、`personality.rs`（人格演化）、`consolidation.rs`（离线语义合并）、`thoughts.rs`（主动建议）、`dedup.rs`、`prompts.rs`、`semantic.rs`（语义检索）、`client.rs`（Ollama/OpenAI + 嵌入 + 工具调用）。

后端路由（`crates/lingshu-server/src/routes/`）：`chat.rs`（对话 + 工具调用）、`memories.rs`、`memory_gateway.rs`（Agent、分层空间、权限、冲突与检索）、`memory_benchmark.rs`（基线、完整模型与消融评测）、`mcp.rs`（MCP 接入）、`teams.rs`（团队成员、记忆与审计）、`calendar.rs`、`personality.rs`、`thoughts.rs`、`permissions.rs`、`settings.rs`（LLM 设置 + 角色提示词）、`integrations.rs`、`signals.rs`、`audit.rs`、`conversations.rs`、`projects.rs`、`tasks.rs`、`users.rs`、`auth.rs`、`system.rs`、`sessions.rs`、`project_members.rs`、`task_dependencies.rs`。

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
# 后端测试（部分集成测试需要数据库或网络）
cargo test --workspace

# 前端单测 + lint + 类型检查 + 构建
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

OpenAPI 3.1 由 [utoipa](https://github.com/juhaku/utoipa) 生成，覆盖已声明的 API 路径与操作（系统、认证、用户、LLM 设置、对话、会话、记忆网关、评测、人格、思绪、日历、权限、集成、信号、审计、项目管理等）。后端运行时访问 `/swagger-ui`；提交的 `openapi.json` 由契约测试校验。

## 已知局限

- Thought Queue 的「跨会话实体计数」触发器为后续增强。
- 竞赛评测已增加 LoCoMo 强词法基线、按官方类别规则计分的本地问答、300 查询合成集与消融；尚缺生产检索接入、真实样本、权重路由、百万级并发压测与真实多 Agent 协作。旧版指标口径已纠正，详见[实验报告](docs/competition/experiment-report.md)。
- 前端暂未提供生成式 API 客户端；当前 OpenAPI 门禁校验代码生成结果与提交的 `openapi.json` 一致性。

## 文档索引

- [产品需求文档](./AI-PersonalAssistant-PRD.md) — 完整产品规格
- [中兴 1529 竞赛交付索引](./docs/competition/README.md) — 命题映射、部署演示、架构、创新点与实验报告
- [竞赛实验报告](./docs/competition/experiment-report.md) — 静态基线、自适应检索、消融实验与证据边界
- [产品定位与版本体系](./docs/product-positioning.md) · [权益实施说明](./docs/version-entitlements.md)
- [并发写入协议](./docs/memory-write-consistency.md) · [自动治理审计](./docs/competition/auto-governance-audit.md)
- [开发版使用说明](./开发版使用说明.md)
- [SoulLedger 设计决策](./docs/soulledger-design-decisions.md)
- [MVP 完成度](./docs/mvp-status.md)
- [贡献指南](./CONTRIBUTING.md) · [更新日志](./CHANGELOG.md)

## 许可证

商业专有软件，未经书面商业授权不得使用、复制、修改或分发。详见 [LICENSE](./LICENSE)。
