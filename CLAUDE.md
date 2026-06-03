# 灵枢 (LingShu) — AI 项目经理个人助理

## 项目概述
面向项目经理的 AI 个人助理，具备 3D 虚拟形象、深度项目管理智能、主动感知能力。

## 技术栈
- 后端: Rust (Axum 0.7 + Tokio + sqlx + reqwest)
- 前端: React 18 + TypeScript + Three.js + Vite
- 数据: PostgreSQL 16 + Apache AGE, Redis 7, Qdrant
- 部署: Docker + Docker Compose

## 开发规范
- Rust 代码通过 Docker 容器编译运行（宿主机未安装 Rust）
- 使用 `docker compose -f docker/docker-compose.dev.yml` 启动开发环境
- 前端在 `frontend/` 目录，使用 Vite dev server
- 后端在 `crates/` 目录，使用 cargo-watch 热重载
- API 端点使用 utoipa 自动生成 OpenAPI 文档
- 前后端类型通过 OpenAPI 契约保持同步

## 命名规范
- Rust crate: `lingshu-*`
- API 路径: `/api/v1/*`
- 数据库表: snake_case, 复数形式
- 组件: PascalCase

## 关键路径
- 后端入口: `crates/lingshu-server/src/main.rs`
- 数据库迁移: `crates/lingshu-server/migrations/`
- 前端入口: `frontend/src/main.tsx`
- Docker 编排: `docker/docker-compose.dev.yml`
