# LingShu Developer Documentation

主要文档入口：[中文 README](../README.md) · [English README](../README.en.md)

## 架构与设计

- [产品定位与版本体系](./product-positioning.md) — 客户端、MCP、HTTP API 现有入口、SDK 规划，以及 Free、Pro、Team、Enterprise、Embedded 版本边界
- [SoulLedger 设计决策](./soulledger-design-decisions.md) — 记忆留存、人格演化、Thought Queue、遗忘机制的设计取舍与实现状态
- [产品需求文档](../AI-PersonalAssistant-PRD.md) — 完整产品规格（中文）
- 架构决策记录仍未单独建立；当前系统架构见 [竞赛架构文档](./competition/architecture.md)

## 项目状态

- [MVP 完成度](./mvp-status.md) — 对照 CLAUDE.md MVP 范围的逐项完成状态
- [Review Findings](./review-findings.md) — 文档/代码审查的待办项
- [竞赛交付索引](./competition/README.md) — Memory Evolution Lab 的架构、部署和实验依据
- [跨平台校赛计划](./2026-09-08-cross-platform-school-competition-plan.md) — 平台适配和验收计划

## 开发

- [贡献指南](../CONTRIBUTING.md) — 开发工作流、PR 流程、编码规范
- [更新日志](../CHANGELOG.md) — 版本历史
- [CLAUDE.md](../CLAUDE.md) — AI Agent 上下文（关键路径、约定、命令）

## API 与数据库

- [记忆检索、写入与派生失效](./memory-retrieval-optimization.md) — 当前源码改动、证据边界、数据库回归及迁移影响

- [Swagger UI](http://localhost:8080/swagger-ui) — OpenAPI 3.1 交互文档（后端运行时可用）
- [OpenAPI 快照](../openapi.json) — 提交的契约文件，由测试校验
- 数据库迁移位于 `crates/lingshu-server/migrations/`；独立数据库架构文档尚未建立

## 运维

- [竞赛部署与演示手册](./competition/deployment.md)
- 配置说明目前集中在 [README 配置章节](../README.md#配置)

---

注：PRD、MVP 状态和审查记录包含历史阶段信息；涉及当前版本、平台和发布能力时，以根目录 README、CHANGELOG 与竞赛交付索引为准。
