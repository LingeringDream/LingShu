# 并发记忆写入与冲突治理

MCP 和 REST 共用 Memory Gateway。业务作用域 `context_scope` 与可见范围
`memory_scope` 独立；旧客户端不传业务作用域时使用 `default`。

```json
{
  "agent_id": "claude",
  "context_scope": "project:lingshu:production",
  "memory_scope": "global",
  "memory_layer": "fact",
  "fact_key": "database.engine",
  "fact_value": "PostgreSQL",
  "content": "生产环境数据库使用 PostgreSQL",
  "request_id": "conversation-42-decision-3"
}
```

`global` 表示当前用户或团队租户内部的公共范围，不是互联网公开。
调用方需要为不同项目、主体及适用条件选择稳定的业务作用域和事实键。
系统不根据两段自然语言自动推断它们是否描述同一个主体。

## 写入协议

- 以用户、业务作用域、事实键取得 PostgreSQL 事务级 advisory lock，
  再插入、检测矛盾并写入裁决事件。首次写入没有旧记录也能锁定。
- MCP 写入、REST 写入、聊天事实补充、共享授权变化和人工裁决使用同一套锁。
- 同一 Agent 的同一 `request_id` 和相同请求内容只生成一个记录；内容变化返回
  HTTP 409。省略 `request_id` 时，每次请求都会保留为独立主张。
- 相同取值保留各自来源；只有有效时间区间重叠的不同取值参与冲突治理。
  时间区间为 `[valid_from, valid_to)`，未给出结束时间表示无上界。
- 不同用户、业务作用域或可见权限域的主张不能互相替代。
  私有记录只在同一所有者内比较；公共记录可跨 Agent 比较；定向共享记录要求
  有效读者集合完全相同（包含所有者），授权变化后重新检查。
- 不同取值统一进入 `disputed`，包括已有争议后的第三条主张。
  自报置信度、读取频次、重要性及请求到达顺序不直接决定事实真伪。

## 检索与裁决

查询可传 `context_scope`，在数据库截取候选前过滤。
自适应检索的 `results` 排除争议、被替代、过期及尚未生效的记录；相关争议
通过 `disputed_memories` 单独返回，并且仍受读取权限约束。两个列表分别最多
返回 `top_k` 条。`static` 模式保留实验基线行为，不应用这些状态过滤。

人工裁决保留原始记录、原始有效时间和审计信息。一个二元裁决不会取消其他
尚未解决的矛盾；获胜主张仍有争议时，不生成已确认的通用经验。
已完成或候选状态失效的裁决不能再次执行以恢复旧事实。

MCP 会显示写入后的状态，并显式提示检索中的争议。调用方不能把“已记录”
等同于“已确认”，也不能仅靠重新提交更高置信度来解除争议。

## 验证

在独立 PostgreSQL 数据库上运行：

```powershell
$env:MEMORY_TEST_DATABASE_URL = 'postgres://postgres@127.0.0.1:55439/postgres'
cargo test -p lingshu-server concurrent_memory_writes_and_reviews -- --ignored --nocapture
cargo test -p lingshu-mcp-core
```

数据库测试应用真实迁移并创建测试租户。它让两个数据库连接同时等待事实锁，
再释放锁以验证首次写入竞争，还覆盖第三条主张、重复请求、时间隔离、私有
权限、定向共享、检索与人工裁决。只应对专用测试数据库运行。

离线 benchmark 中的加权裁决函数保留为历史实验策略，不能把其准确率或
Token 估算结果当作当前保守在线裁决策略的端到端验证结果。
