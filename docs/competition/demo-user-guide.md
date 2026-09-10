# 灵枢多 Agent 记忆演示使用手册

本文用于本地展示灵枢的多 Agent 记忆共享、权限隔离、冲突治理和自适应检索。演示页面属于开发版 / 竞赛版；正式桌面客户端不显示“动态记忆引擎”入口。

## 1. 先选择数据模式

灵枢正式 App 与源码开发版默认使用两套独立数据库。

| 模式 | 后端与数据库 | 适用场景 |
| --- | --- | --- |
| App 记忆模式 | 正式 App 后端 `127.0.0.1:8080`；内置 PostgreSQL `127.0.0.1:55432` | 在实验台查看 App 已有的对话记忆 |
| 独立演示模式 | 源码后端 `127.0.0.1:8080`；本机或 Docker PostgreSQL `127.0.0.1:5432` | 准备独立、可重复的竞赛演示数据 |

两个模式的数据不会自动互通。切换模式时，应先确认 8080 端口上运行的是哪一个后端：

```bash
lsof -nP -iTCP:8080 -sTCP:LISTEN
lsof -nP -iTCP:5432 -sTCP:LISTEN
lsof -nP -iTCP:55432 -sTCP:LISTEN
```

不要同时启动正式 App 后端和源码后端，它们都会占用 8080。

## 2. 使用 App 中已有的记忆

这种方式只启动竞赛版前端，浏览器通过 Vite 代理访问正式 App 的后端。

1. 退出正在运行的 `cargo run -p lingshu-server`。
2. 打开正式灵枢 App，等待聊天和记忆中心可以正常访问。
3. 确认 8080 和 55432 已监听：

   ```bash
   lsof -nP -iTCP:8080 -sTCP:LISTEN
   lsof -nP -iTCP:55432 -sTCP:LISTEN
   ```

4. 单独启动前端：

   ```bash
   cd /Users/ymqz/projects/PA/frontend
   npm run dev
   ```

5. 浏览器打开 <http://localhost:5173>，进入“动态记忆引擎”。

如果前端与已安装 App 的后端版本不兼容，应更新 App，或改用下一节的独立演示模式。不要直接修改 App 内置数据库文件。

## 3. 使用独立演示数据库

### 3.1 5432 未被占用

在仓库根目录执行：

```bash
cd /Users/ymqz/projects/PA
./scripts/dev.sh
```

脚本会启动 PostgreSQL、Redis、Qdrant、源码后端和竞赛版前端，并执行数据库迁移。

### 3.2 已有 PostgreSQL 占用 5432

如果 5432 已由本机 PostgreSQL 占用，并且 `.env` 与 `config.toml` 已指向该数据库，可复用本机 PostgreSQL，只让 Docker 启动 Redis 和 Qdrant：

```bash
cd /Users/ymqz/projects/PA
docker compose -f docker/docker-compose.dev.yml up -d --wait redis qdrant
sqlx migrate run --source crates/lingshu-server/migrations
cargo run -p lingshu-server
```

保持后端终端运行，另开一个终端：

```bash
cd /Users/ymqz/projects/PA/frontend
npm run dev
```

执行迁移前应确认这是允许修改的开发或演示数据库，不要对未备份的业务数据库运行迁移。

## 4. 注册演示 Agent

打开“动态记忆引擎”，切换到 **Agent Governance**，在“注册协作 Agent”中填写 Agent Key 和显示名称。

建议准备以下三个身份：

| Agent Key | 显示名称 | 演示职责 |
| --- | --- | --- |
| `sales_agent` | 销售 Agent | 写入客户与合同信息 |
| `tech_agent` | 研发 Agent | 读取获准共享的技术或需求信息 |
| `hr_agent` | 人事 Agent | 验证未授权身份无法读取 |

Agent Key 只允许使用字母、数字、下划线和连字符，长度为 1–64。

这里注册的是当前工作区内的逻辑记忆身份，用于演示 ACL。它不会启动一个执行任务的子 Agent，也不会生成外部接入凭证。

## 5. 演示私有记忆与定向共享

### 5.1 写入私有记忆

在 Agent 列表选择“销售 Agent”，填写：

| 字段 | 演示值 |
| --- | --- |
| 记忆层级 | 结构化事实 |
| 权限空间 | 私有 |
| 事实键 | `customer.bottom_price` |
| 事实值 | `80万元` |
| 场景标签 | `customer, contract` |
| 记忆内容 | 客户可接受的最低合同金额为80万元 |

点击“写入记忆”。

### 5.2 验证隔离

进入 **Memory Console**，查询“客户可以接受的最低合同金额是多少？”。

- 销售 Agent 能够检索到该记忆。
- 研发 Agent 和人事 Agent 检索不到该记忆。

权限过滤发生在数据库候选查询阶段，无权内容不会进入模型上下文。

### 5.3 定向授权

回到 **Agent Governance**，保持销售 Agent 为当前身份。在“定向授权”中选择研发 Agent，点击“授权读取”。

再次检索：

- 研发 Agent 现在可以读取。
- 人事 Agent 仍然无法读取。

### 5.4 团队公共记忆

再写入一条“全局公共”记忆：

```text
事实键：project.tech_stack
事实值：Rust + React + PostgreSQL
内容：项目当前采用 Rust、React 和 PostgreSQL 技术栈
```

销售、研发和人事 Agent 均应能够检索。这里的“全局”只表示当前用户或团队记忆空间内公共，不表示互联网公开。

## 6. 演示冲突治理

1. 进入 **Memory Console**，点击“载入冲突演示”。
2. 系统会为同一个 `user.location` 事实键写入“北京”和“深圳”两个不同取值。
3. 查询“我现在住在哪里？”，点击“运行双路检索”。
4. 对比 Static RAG 和 Dynamic Memory：Dynamic Memory 会把未裁决争议从正常结果中隔离。
5. 进入 **Agent Governance → 冲突与生命周期账本**。
6. 点击“采用新值”，确认将“深圳”设为有效事实。
7. 回到 Memory Console 重新检索，检查有效事实、被替代记录和经验来源链。

系统不会用最后写入覆盖旧值。冲突双方会先进入 `disputed`，完成显式裁决后再恢复正常召回。

## 7. 外部 Agent 接入

逻辑 Agent 注册适用于本地实验台。外部 Agent 接入团队记忆需要独立凭证：

1. 在“动态记忆引擎”进入 **团队工作区**。
2. 在侧栏“云账户”登录；创建团队或加入已有团队。
3. 管理员在“Agent 接入凭证”中填写 Agent 标识。
4. 选择“只读”或“读写”，设置 1–365 天有效期。
5. 点击“签发凭证”，立即保存只显示一次的 `lsk_...` 凭证。

外部 Agent 使用凭证检索：

```http
POST /api/v1/teams/<team-id>/search
Authorization: Bearer <Agent 凭证>
Content-Type: application/json

{
  "query": "项目约定",
  "mode": "adaptive",
  "top_k": 5
}
```

服务端根据凭证确定 Agent 身份并覆盖请求中的 `agent_id`。凭证可以过期或撤销，团队管理员可以在审计记录中查看相关操作。

## 8. 3–5 分钟推荐演示顺序

1. 用 20 秒说明多 Agent 记忆混杂与隐私泄露问题。
2. 注册销售、研发、人事三个 Agent。
3. 写入销售私有记忆，证明另外两个 Agent 无法读取。
4. 定向授权研发 Agent，证明研发可读而人事仍不可读。
5. 写入团队公共记忆，证明三个 Agent 均可读取。
6. 载入事实冲突，展示争议隔离、人工裁决和经验沉淀。
7. 展示 Static RAG 与 Dynamic Memory 的双路检索和评分拆解。
8. 时间允许时重跑标准评测，说明数据集和指标边界。

## 9. 常见问题

### 端口 5432 被占用

这是本机 PostgreSQL 与 Docker PostgreSQL 的端口冲突。使用第 3.2 节复用本机 PostgreSQL，或停止确定不再使用的本机 PostgreSQL 服务后再运行 `./scripts/dev.sh`。

### 端口 8080 被占用

正式 App 后端或另一个源码后端正在运行。执行：

```bash
lsof -nP -iTCP:8080 -sTCP:LISTEN
```

需要查看 App 记忆时保留正式 App 后端，只启动前端；需要独立演示时退出正式 App，再启动源码后端。

### 浏览器实验台与 App 记忆不同

检查 8080 当前连接的是正式 App 后端还是源码后端。正式 App 默认使用 55432，源码开发版默认使用 5432。

### 注册 Agent 后无法在正式客户端侧栏看到

正式客户端不提供实验台入口。注册结果属于 Memory Gateway 的记忆身份，可在竞赛版 Agent Governance 中查看；外部接入应使用团队 Agent 凭证。

### 重复点击“载入冲突演示”后出现多条冲突

演示数据会持久化。正式录制前应使用专门的演示数据库，并提前走完一次流程确认状态；不要在未备份的业务数据库中清理或重置数据。

## 10. 能力边界

- 已实现：分层记忆、私有/定向共享/公共空间、数据库 ACL、冲突保留、人工裁决、自适应检索、团队 Agent 凭证与审计。
- “增强输出”是多个模型分别回答同一问题后统一整合，不是自主拆分子任务的 Agent 调度器。
- 当前聊天不能自行创建或派发子 Agent。
- 真实多 Agent 协作任务、百万级并发、自动治理目标和部分公开数据集策略仍需继续验证。
