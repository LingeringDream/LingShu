# 部署与演示手册

## 环境

- Rust stable（仓库提供 `rust-toolchain.toml`）
- Node.js 20+
- PostgreSQL 16
- 可选：Redis 7、Qdrant

## 本地启动

```bash
cd /Users/ymqz/projects/PA
./scripts/dev.sh
```

脚本会检查环境、启动基础设施、执行 SQL 迁移并启动后端和 Vite。浏览器访问 `http://localhost:5173`，选择左侧“记忆进化实验台”。

分步启动：

```bash
docker compose -f docker/docker-compose.dev.yml up -d
cargo run -p lingshu-server
cd frontend && npm run dev
```

## 演示顺序（3–5 分钟）

1. 展示三层统计和数据流。
2. 使用 `fact_key=project.stack` 写入旧值与新值，展示冲突审计和被替代事实过滤。
3. 创建第二个 Agent；演示私有记忆不可见、定向授权后可见、全局公共可见。
4. 输入“项目现在主要使用什么技术栈”，同时运行静态与自适应检索，比较排序、过滤、Token 和评分拆解。
5. 点击“重跑标准评测”，展示 60 轮结果、五项阈值和消融表。
6. 明确说明自建集与公开集的证据边界。

## 验证命令

```bash
cargo test --workspace
cd frontend
npm run test
npm run type-check
npm run build
```

API 文档：`http://localhost:8080/swagger-ui`。

