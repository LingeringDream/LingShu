# 自动记忆治理：遗忘保护与证据规则候选

2026-09-09。本次生产改动聚焦避免后台遗忘清理仍需保留的证据；分类候选只进入离线评测，不替换生产冲突裁决。

## 生产遗忘保护

`run_forgetting_sweep` 在派生来源保护查询失败时终止本轮清理，与人格来源查询失败的行为一致。

实际软删除由 `forget_if_unchanged` 执行：先在事务内锁定候选记忆，再使用新的 SQL 语句快照检查重要性、访问时间、创建时间、更新时间是否与扫描时一致；变化的候选留到下轮重新评估。同时保留 disputed/unresolved 记忆、存在 pending 冲突事件的记忆、derivation_stale 记忆，以及活跃人格和未删除派生记忆引用的来源。

派生创建已有来源行锁，删除前先锁来源、再查询引用，可以看到锁等待期间提交的新派生引用。自动人格演化写入方现已按相同来源锁顺序协作，锁等待后重新检查来源有效性和用于推理的内容；验证失败时取消本次演化并保留原人格。正常未受保护、未发生变化的低价值旧记忆仍按原半衰期策略软删除；向量索引删除仍由现有数据库触发器入队。

## 离线证据规则候选

`eval/auto_state.py` 增加 `evidence_rule` 对照，保留旧 `rule` 和 `always_active`。每条决定附状态、原因、证据 ID 和是否需复核：

- 年龄和“临时”本身不作为失效证据。
- 仅窄语法的明确失效陈述支持 stale；否定、疑问、条件和相对时间句保守保留。
- 相同主体的明确变更且严格晚于不同主张，才支持 superseded；普通新陈述和时间相同的冲突标 unresolved。
- 仅明确陈述的相同文本且存在唯一最新记录时折叠旧重复；不隐藏争议。
- 不支持的正文保留 active 并标 review_required。这是保留决策，不是确认事实正确。

仍使用 oracle fact_key 分组，不支持完整自然语言解析。该候选不能接入生产自动淘汰。

首次开发集运行（86 条记忆，44 条 gold active）：

| 方法 | Macro-F1 | 错误淘汰 | 误保留 |
|---|---:|---:|---:|
| 原规则 | 44.16% | 12/44（27.27%） | 47.62% |
| 证据规则候选 | 13.54% | 0/44（0%） | 100% |

结果说明证据语法覆盖不足，不能因误淘汰降低就称治理质量提升。本轮未运行测试集或 LLM，也未替换既有冻结报告。下一步需补足有来源、可审计的事实证据和人工复核标签，并按原审计协议验证下游检索收益。

## 验证与复现

```sh
python3 -m unittest discover -s eval -p test_auto_state.py
python3 eval/auto_state.py --dataset datasets/zte_memory_v4_dev.json --out output/auto_state_v4_dev_evidence_newrun.json
cargo test -p lingshu-server forgetting
# 仅指向独立测试数据库；测试会应用迁移。
MEMORY_TEST_DATABASE_URL=<dedicated-db-url> cargo test -p lingshu-server forgetting_rechecks -- --ignored
```

Python 14 项测试覆盖旧输出协议及新规则的年龄、否定、主体、时间冲突、重复、分组隔离、金标独立和证据追踪。Rust 遗忘单元测试 25 项通过。新增真实 PostgreSQL 测试覆盖正常删除、访问后保护、编辑后保护、争议保护、派生复核保护、派生引用保护、人格引用保护和待裁决事件保护。

### 补充完整验证（2026-09-09）

在临时虚拟环境 `/tmp/pa-governance-test-venv` 中按 `eval/requirements.txt` 安装 numpy 2.5.2、nltk 3.10.3，解决此前依赖缺失。

- `cargo test --workspace`：412 项通过、0 失败，29 项默认忽略。
- 完整 Python 套件 `python -m unittest discover -s eval -p 'test_*.py' -v`：20 项通过、0 失败。
- 在独立临时 PostgreSQL 16 数据库中显式补跑 5 项原本忽略的测试，全部通过：新增遗忘保护测试、3 项已有来源保护测试、Gateway 并发写入与裁决测试。
- 新增遗忘保护测试包含 8 种删除/保护场景，并补充真实锁等待场景：先创建未提交派生引用，确认遗忘操作等待来源锁，再提交派生写入；遗忘操作返回删除 0 行。
- `rustfmt --check` 与 `git diff --check` 通过。

本轮数据库测试使用临时实例，测试结束后已停止。剩余默认忽略测试未全部执行；以上结果证明被测代码行为，不代表分类候选达标或所有并发时序均已覆盖。


## 人格并发与指标修复（2026-09-09）

本轮限定两项修改：

1. 自动人格快照在更新旧快照之前，按 UUID 顺序以 `FOR SHARE` 锁定来源记忆，再以新语句快照复核。空来源、缺失来源、其他用户来源、已删除/失效/争议/待复核来源及正文、类型、重要性变化均拒绝写入。证据加载阶段也排除未生效、已过期和非 active 记忆。锁保持到快照提交，避免后台遗忘穿过来源检查与引用落库之间的窗口。
2. Rust 治理评测的 stale_filter_rate 统计真正被过滤的 stale 与 superseded 样本数；duplicate_filter_rate 统计真正被过滤的 duplicate 样本数。保留既有输出字段，不覆盖冻结报告。

新增 4 项指标回归测试，验证生产过滤、完全关闭过滤、部分过滤及错误过滤全部记录。关闭过滤器时两种过滤率均为 0%；仅过滤 stale 时，含 superseded 的综合过滤率为 50%。

独立 PostgreSQL 测试验证两种实际锁等待顺序：人格先锁来源时遗忘等待，引用提交后删除数为 0；删除先锁来源时人格等待，删除提交后拒绝来源。另覆盖来源编辑、争议、失效、未生效、待复核、空来源和跨用户来源。

本轮 `cargo test --workspace` 共 416 项通过、0 失败、30 项默认忽略；新并发测试显式运行通过（1 项，含上述多场景）。格式与差异检查通过。Python 本轮未改动，沿用上一轮完整套件 20 项通过的记录。

复现新增数据库测试：

```sh
MEMORY_TEST_DATABASE_URL=<dedicated-db-url> cargo test -p lingshu-server personality_sources_serialize -- --ignored
```
