# 部署与演示手册

## 环境

- Rust stable（仓库提供 `rust-toolchain.toml`）
- Node.js 22+
- PostgreSQL 16
- 可选：Redis 7、Qdrant

## 本地启动

### 2026-09-08 源码升级注意事项

升级前备份业务数据库并确认恢复方法。新增 4 个迁移：词法索引、持久化向量索引队列、历史查询审计和派生有效性。首次启动可能为旧记忆补建 embedding；既有派生摘要因缺少来源版本快照而保留为待复核，不再参与召回。原始记忆不删除，后台仅在满足合并条件时产生新摘要，不保证立即恢复所有旧摘要。详情及专用数据库验证命令见[优化说明](../memory-retrieval-optimization.md)。

仓库推送不等于客户端发布；本轮只在专用测试库运行迁移，未执行业务库迁移。

```bash
cd <repository-root>
./scripts/dev.sh
```

脚本会检查环境、启动基础设施、执行 SQL 迁移并启动后端和 Vite。浏览器访问 `http://localhost:5173`，选择左侧“记忆进化实验台”。

分步启动：

```bash
docker compose -f docker/docker-compose.dev.yml up -d
cargo run -p lingshu-server
cd frontend && npm run dev
```

Windows 演示可在 Docker Desktop 启动后双击仓库根目录的 `启动灵枢开发版.cmd`；停止时使用
`停止灵枢开发版.cmd`。Windows 独立安装包不需要 Docker 或外部 PostgreSQL，云账户网页登录和
下载中心说明见 [`lingshu-cloud/README.md`](../../lingshu-cloud/README.md)。

## 演示顺序（3–5 分钟）

1. 展示三层统计和数据流。
2. 使用 `fact_key=project.stack` 写入不同取值，先展示争议保留，再显式人工裁决并展示审计与被替代项过滤。
3. 创建第二个 Agent；演示私有记忆不可见、定向授权后可见、全局公共可见。
4. 输入“项目现在主要使用什么技术栈”，同时运行静态与自适应检索，比较排序、过滤、Token 和评分拆解。
5. 展示固定数据集和消融结果；说明标签、划分与指标局限，不将演示结果表述为“五项全部达标”。
6. 明确说明自建集与公开集的证据边界。

## 验证命令

```bash
cargo test --workspace
cd frontend
npm run test
npm run lint
npm run type-check
npm run build
```

API 文档：`http://localhost:8080/swagger-ui`。
