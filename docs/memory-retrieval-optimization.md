# 记忆检索与写入优化（2026-09-08）

本轮改动覆盖在线召回和写入一致性。没有改变自动事实真伪判定策略，也没有将离线 LoCoMo 分数作为本轮收益。

## 在线检索

聊天（包括重新生成和多模型整合）、Memory Gateway HTTP 与其 MCP 入口调用 `llm/retrieval.rs`。

- 词法候选从完整授权数据范围召回，不再先截取最近 500 条。长度大于 2 字符的查询使用 `lower(content)` 的 pg_trgm GiST 距离索引取最近 200 条，物化候选后再检查中英文词项重叠，避免高频词命中十万行时逐行分词计数排序。1–2 字符查询沿用 GIN 单字/双字词项过滤，按时间取候选。这不是 BM25，候选排序也不再等价于旧版词项重叠计数。
- 通用词法与向量各保留最多 200 个候选，另有最多 50 条精确短语候选，合并去重后上限 450。精确通道使用转义的参数化正则和 ASCII 标识符边界，复用 GiST 索引，查询至少含 3 个字母/数字/汉字时启用。它用于补回被短近似文本挤出的长正文，不保证超过 50 条精确命中时全部覆盖。所有通道仍必须通过 PostgreSQL 的用户、所有者、空间和授权检查。
- 业务作用域、记忆层、当前有效时间及状态在候选限额之前处理。私有记忆不会因为残留授权被其他 Agent 读取。
- 保留向量真实相似度，避免把每次查询的第一名都当作满分相关。最终仍使用六特征加权评分；2026-09-09 调整其中的词法特征：完整短语/名称匹配为 1，查询中的数字标识符完整匹配至少为 0.9，缺失则将原词法相似度乘 0.2，其余近似匹配最高为 0.5。默认权重下，频次一项不足以盖过完整短语匹配；其他元数据及自定义权重仍可能改变排序。数字、版本和标识符属于启发式匹配，不是实体识别或日期意图理解。未声称权重已经学习或优于 BM25。
- 查询向量通道有 5 秒总超时，故障后使用词法候选。有效事实和待裁决主张分开返回，聊天上下文显式标记争议。
- SQL 候选查询设置事务内 2 秒 `statement_timeout`，失败返回错误，回滚后不污染连接池设置。这不是整个请求的 2 秒保证，连接等待、向量调用与审计另计。
- 每次召回只更新一次访问计数；命中信号批量插入。`candidate_count` 为两通道合并后的候选数，`filtered_count` 只包含之后的过滤，不代表全库过期记录数。

现有 `/api/v1/memories/search` 管理接口保持原接口实现；本轮统一的是聊天与 Memory Gateway/MCP。词法通道仍有候选预算，向量检索仍先按用户召回，复杂授权域可能需要后续自适应补足。实体归一化和学习排序尚未实现。

GiST 通用候选依赖字符三元组相似度，长文、改写和大量同分文本仍可能漏召回；精确通道仅补全字面短语，不能解决所有改写。同分候选在预算边界不保证稳定，最终结果按六特征评分与时间/ID 排序。权限过滤始终在 LIMIT 前执行；高度选择性的用户/授权域可能扫描更多索引项，当前索引不是按租户分区。需要继续用真实语料评估召回质量。

新增迁移 `20260908000005_memory_lexical_knn.sql` 复用已有 pg_trgm 扩展，不引入 btree_gist。GiST 增加磁盘和写入维护开销；旧 GIN 索引仍服务短查询与写入去重。升级建索引期间可能阻塞写入，大库应安排维护窗口。

## 有效时间查询与有界合并

Gateway/MCP adaptive 查询支持可选 `as_of`（RFC3339）。按指定时刻检查有效区间，已过期但当时有效的记录可以被召回；被取代的说法只放在 `historical_memories`，不是已确认事实。查询时间与历史结果 ID 写入审计。权限始终按当前 ACL 检查，已删除记录不可见。这不是历史 ACL 或“当时已知信息”的双时间快照；聊天不会自动解析日期。

后台合并只加载最多 64 条当前有效、尚未归纳的原始记忆；单条最多 4000 字符，候选文本预算约 24000 字符（不是 token 保证），LLM 调用超时 60 秒。当前仅处理灵枢私有默认作用域；其他作用域暂跳过，不能跨空间合并。每组合并必须同类型，来源 UUID 唯一，置信度和重要性必须有限且在范围内。

写入前稳定顺序锁定来源，重新检查有效状态、内容与更新时间，拒绝已变化或已归纳的来源；新归纳记录有效期不超过最早到期来源，索引沿用事务队列。不保证每轮覆盖全部记忆。

## 来源变更与派生失效（后续批次）

迁移 `20260908000004` 增加数据库维护的 `derivation_stale`。来源内容、删除、事实状态、作用域、授权、有效区间和可信度变化会在同一事务内递归标记派生后代；集合去重避免循环来源无限递归。访问次数、重要性降权和普通时间戳更新不触发失效。

- 待复核摘要保留正文与来源，metadata 含 `derivation_review_required=true`；Gateway 展示为 filtered。聊天、Gateway（包括 static/as_of）、管理搜索、人格上下文和目标上下文均不使用它们。
- 原有同步写入不能清除失效标记；导入时保留 metadata 中的复核标记。新摘要必须引用当前可用、同作用域来源，共享授权继承暂不支持；有效期和可信度不超过来源。
- 当前没有“直接确认旧摘要”接口。后台可以基于当前来源生成新摘要，旧摘要仍保留；重建不会因已有摘要而重复降低同一来源的重要性。
- **迁移影响：既有派生摘要缺少来源版本快照，全部保留并标为待复核。** 此迁移未在业务库执行。迁移前应评估已有摘要数量；不会删除原始记忆。
- 本轮覆盖记忆表内的派生链，不撤回已经生成的聊天回答、人格快照等外部产物；也不是跨设备的完整来源版本证明。大量后代的事务开销仍需压测。

## 实际检索链路的最小基线

新增固定夹具 `retrieval_baseline_fixture.json` 和 `production_retrieval_baseline` 数据库测试，调用实际检索服务，关闭 embedding；与保留最近 500 条窗口的词法基线对比。650 条干扰记录下覆盖旧中文事实、英文、过期、争议、权限隔离与作用域正反例，共 7 个查询，输出逐例命中、非预期结果及单次延迟 JSON。

本地回归的 3 个应命中目标中，实际链路命中 3 个、窗口基线命中 2 个；7 个查询均无非预期有效结果。这仅证明针对性回归，不是总体准确率提升，也不是与 mem0 或真实模型的质量对比。既有 `lingshu-eval` 离线策略评测保留；下一步仍需要独立标注集、真实向量与并发压测。

## 持久化索引

新增迁移 `20260908000002`：记忆插入、正文编辑、同步和软/硬删除在原数据库事务内写入 `memory_index_jobs`。启动时有 Qdrant 配置的服务运行后台 worker；未配置 Qdrant 时任务保留，词法检索仍可用。

- 迁移首次为现有记忆建立待处理任务，用于补建和删除对账；大库首次启动会产生后台 embedding 工作量。
- 任务按记忆 ID 合并，并维护递增版本。失败指数退避，任务不丢弃；进程退出后租约到期可重试。
- 单次操作上限 60 秒，租约 120 秒；多 worker 用数据库租约协调。
- 索引期间发生编辑时，旧任务完成不能清除新版本任务；下一次读取最新数据库内容重新索引。
- Qdrant 写入和删除使用 `wait=true`，收到操作完成响应再确认任务。数据库权限与删除状态仍是查询的最终边界。
- 删除、同步和遗忘不再旁路直接修改向量库，避免旧异步请求覆盖 worker 的结果。

该机制提供可重试的最终一致性，不保证保存返回时向量立即可见。远端请求超时与服务端实际执行之间仍存在分布式系统的不确定性；未宣称 exactly-once 或完成跨进程压力验证。

## 原子事实写入

自动抽取统一使用 `persist_memory`：先按事实键锁定，再去重、写入来源和执行保守冲突检查，最后提交。补充结构化字段失败会回滚记忆及索引任务。

重复事实来源集合取并集；结构化去重同时检查事实键和值，不会仅因句子近似就吞掉新主张。去重限定在本地写入方的私有默认作用域，并复用同一数据库事务连接。矛盾仍保留为 `disputed`，由显式裁决解决。

## 回归证据与复现

对专用 PostgreSQL 测试库运行（索引测试要求初始任务队列为空）：

```sh
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_RETRIEVAL_TEST_DB cargo test -p lingshu-server recall_old_evidence_with_acl_and_chat_parity -- --ignored
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_RETRIEVAL_TEST_DB cargo test -p lingshu-server extracted_fact_is_atomic_and_accumulates_sources -- --ignored
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_RETRIEVAL_TEST_DB cargo test -p lingshu-server concurrent_memory_writes_and_reviews -- --ignored
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_RETRIEVAL_TEST_DB cargo test -p lingshu-server consolidation_revalidates_sources -- --ignored
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_RETRIEVAL_TEST_DB cargo test -p lingshu-server derivation_invalidation_is_transitive_and_transactional -- --ignored
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_RETRIEVAL_TEST_DB cargo test -p lingshu-server production_retrieval_baseline -- --ignored --nocapture
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_DISPOSABLE_PERF_DB cargo test -p lingshu-server common_chinese_retrieval_performance -- --ignored --nocapture
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_DISPOSABLE_QUALITY_DB cargo test -p lingshu-server paired_retrieval_quality -- --ignored --nocapture
MEMORY_TEST_DATABASE_URL=postgresql:///YOUR_EMPTY_INDEX_TEST_DB cargo test -p lingshu-server durable_index_retry_edit_and_delete -- --ignored
cargo test --workspace
cargo clippy -p lingshu-server -p lingshu-vector --all-targets -- -D warnings
```

数据库回归覆盖：650 条较新无关记忆、250 条无权记忆、250 条未来记忆不能遮挡两年前的中文事实；语义 ID 可绕过时间窗口但不能绕过 ACL；聊天与网关一致；争议提示；重复来源累积；结构化补充失败回滚；索引重试、处理中编辑、过期租约恢复、删除清理。向量与 embedding 使用本地 HTTP mock，验证协议与故障恢复，不代表真实模型质量或 Qdrant 集群压测。

性能测试在专用可丢弃数据库保留 100000 条合成记录，方便复查执行计划；运行后应删除该测试库。它顺序测量旧词项排序 SQL、新候选查询和完整 `search()` 服务调用（含评分、审计和访问计数），然后测四并发总墙钟时间；关闭 embedding/Qdrant，不含 HTTP 网络与鉴权。不能将候选 SQL 加速比等同于完整软件或真实模型加速比。

2026-09-08 本机 Apple M5 / 16GB、PostgreSQL 16.14、Rust debug 服务测试：99999 条“无关记录 编号 hash”加 1 条旧中文事实，查询“无关记录 42424”。旧排序 SQL 单次 4118.8ms；新候选 10 次中位数 24.0ms、最大 25.6ms；完整 search 10 次中位数 31.6ms、最大 37.7ms；4 个并发 search 总墙钟 62.3ms，均成功。候选部分约 172 倍改善只适用于这个高重复合成压力场景；不是 HTTP 吞吐、P99 或真实语料质量证明。GiST 距离扫描的 EXPLAIN ANALYZE 也已单独确认。

上述首轮结果曾出现精确编号进入候选却未进入前五。2026-09-09 针对此问题加入成对质量回归：10 个手工标注用例，每个作用域包含 1 条目标、250 条访问次数为 100 的近似干扰和 350 条无关文本，覆盖编号、工单、版本、名称边界、中英文短语、改写、双字词和长正文。每例仅标注 1 个相关目标。

| 同一夹具上的方案 | Top1 命中 | 前五命中 | Recall@20 |
|---|---:|---:|---:|
| 原词项计数候选＋原评分 | 4/10 | 4/10 | 40% |
| GiST＋精确候选＋原评分 | 4/10 | 4/10 | 40% |
| GiST＋精确候选＋新词法评分 | 10/10 | 10/10 | 100% |

新方案多了最多 50 条精确候选预算，并非等候选预算消融。测试冻结访问计数进行三组评分，随后调用实际 search 核对新方案目标排名。旧候选覆盖 9/10 目标，新候选覆盖 10/10；初次仅 GiST 的尝试在该夹具上漏掉长文和准确名称，故增加精确通道。该集合用于开发和回归，不是独立测试集；不能推广成真实总体准确率，仍缺日期表达、多实体组合与纯语义改写的独立标注验证。原 `lexical_similarity` 和去重算法保持不变。

同日再次顺序运行十万条高重复压力测试（同库还保留其他回归夹具）：旧排序 SQL 3856.7ms，新候选中位数 29.4ms、最大 31.4ms；完整 search 中位数 35.1ms、最大 48.6ms；四并发 search 总墙钟 70.2ms。相较首轮测量有小幅开销，但不是严格隔离的前后负载实验。以上均关闭向量/模型，不含 HTTP 与鉴权。

最后在全新专用数据库补上“42424 精确编号必须排第一”的断言，10 次完整检索均通过：旧 SQL 3406.4ms，新候选中位数 28.3ms，完整 search 中位数 33.9ms、最大 47.4ms，四并发总墙钟 68.8ms。质量回归、既有权限/历史回归和全工作区测试均通过；测试数据库已清理，可按上述命令重建。
