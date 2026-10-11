"""Generate the diagnostic governance report; do not modify the main report."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(name):
    return json.loads((ROOT / "output" / name).read_text(encoding="utf-8"))


def main():
    audit = read("governance_v4_audit.json")
    runs = {s: read(f"auto_state_v4_{s}_audited_v2.json") for s in ("all", "test")}
    assert all(r["complete"] and not r["smoke_only"] for r in runs.values())
    lines = ["# 自动治理 v4：评测修复与阶段性审计", "",
             "本报告是诊断性交付，不是自动治理达标证明。保留原始 v4 数据及旧结果，不将本轮数字写入主报告的达标表。",
             "", "## 判断", "",
             "当前应先修复测量、复核标签，再扩容并验证算法。附件中的负面结果值得保留，"
             "但还不能推出“真实数据是唯一解法”“合成数据测不出能力上限”或“LLM 已达到能力上限”。"
             "模型、输出协议、标签定义、输入证据充分性和构造规律尚未被分开验证。",
             "", "## 1. 已确认的问题", "",
             "- 规模实际为 129 turns / 73 queries，未达到原定 200+ turns / 100+ queries；手写场景仍是合成样本，不是有来源的真实对话。",
             "- 按 query ID 哈希划分不等于分层划分。现有 dev/test 的 fact_key 不重叠，但共用演化模板；"
             "test 只有 43 条状态标签，其中 active 21、superseded 14、stale 5、duplicate 1、unresolved 2。",
             "- 原 LLM 解析器把缺失、无效状态静默回退为 active；重复 ID 也可能覆盖先前结果。旧结果未保存逐条预测与原始输出，不能回溯断言误差全来自模型推理。",
             "- 原提示词包含 .dup/.err/.fix 以及 gen.corr/gen.unres 等构造信息。现已改为不含标签线索的局部 ID，并按可观察内容生成稳定的组内打乱顺序。模型只看到正文、相对时间与局部 ID。",
             "- fact_key 分组仍来自构造真值，是 oracle 聚类条件；本次没有评测实体识别或事实归组能力。",
             "", "### 标签与文本复核候选", "",
             "以下均为复核标记，不自动替换 gold。不同问题可能落在同一条记录上，数量不能相加当作错误样本数。", "",
             "| 检查 | 数量 | 需要核对什么 |", "|---|---:|---|"]
    descriptions = {
        "duplicate_without_equal_value": ("duplicate 没有同值同组记录", "不同值可能是冲突或错误，不能任意标为重复"),
        "correction_precedes_different_claim": ("更正早于不同值记录", "事件时间、记录时间、被纠正的具体来源是否完整"),
        "expiry_needs_calendar_evidence": ("本周有效却被标过期", "需要明确参考日期、有效截止时间和取消证据"),
        "question_embedded_in_fact_statement": ("事实句中嵌入问句", "生成器误把问题模板嵌入陈述，需复核内容可读性"),
        "conflict_requires_resolution_policy": ("同组同时存在 unresolved 与 active", "是否存在裁决或权威更新；何以一方确定、另一方未决"),
    }
    for key, (label, explanation) in descriptions.items():
        lines.append(f"| {label} | {audit['flag_counts'].get(key, 0)} | {explanation} |")
    lines += ["", "还应人工区分注册地址与办公地址、被新计划替代与自然过期、历史事实与当前状态。"
              "例如搬办公室不自动证明注册地址发生变更；记录很旧也不自动证明手机号已失效。",
              "", "## 2. 修复后的指标与结果", "",
              "Macro-F1 固定平均五类；Active Recall 是 active 分类召回；错误淘汰率是 gold active 被明确预测为非 active 的比例。"
              "漏报/无效输出单独计数，在分类上算错，不伪装成保留或淘汰。另报包含未知输出的 active_nonrecognition_rate。"
              "五分类只是当前事实筛选的诊断；unresolved 和未知记录不应从存储中删除，应保留证据供复核。",
              "", "`--limit` 现在限制所有方案共同评测的 fact_key 范围，标记 smoke_only，不再将未调用的样本按 active 计分。"
              "每组保存原始输出、局部 ID 映射、解析错误及 token 数；结果保存逐条预测、gold、混淆矩阵、模型摘要和代码/数据校验值。",
              "", "| 范围／方法 | Macro-F1 | Active Recall | 错误淘汰率（条数） | 误保留率 | 输出覆盖率 |",
              "|---|---:|---:|---:|---:|---:|"]
    for split, run in runs.items():
        for arm in ("always_active", "rule", "llm"):
            c = run[arm]["classification"]
            label = {"always_active": "全部保留", "rule": "规则", "llm": "LLM"}[arm]
            scope = "全量129条" if split == "all" else "测试43条"
            lines.append(f"| {scope}／{label} | {c['macro_f1_all']*100:.2f}% | {c['active_recall']*100:.2f}% | "
                         f"{c['false_elimination_rate']*100:.2f}%（{c['false_elimination_count']}/{c['active_total']}） | "
                         f"{c['false_retention_rate']*100:.2f}% | {c['prediction_coverage']*100:.2f}% |")
    lines += ["", "规则和 LLM 的显式错误淘汰比例均未达到 <5%。全部保留虽然误杀为 0，误保留却为 100%，"
              "所以不能只凭错误淘汰率验收。测试部分只有 21 条 active，一条翻转约 4.76pp；"
              "即使观察到 0 次错误，也不能据此宣称真实错误率稳定低于 5%。",
              "", "本轮全量 LLM 有 1 条无效输出；测试部分无无效输出。"
              "移除构造线索、改变局部编号及顺序后，结果发生变化；这提示协议敏感性，需要预注册多种排列的独立实验。"
              "不能从单次变化归因于某一个因素。本次未搜索提示词或按测试分数选择排列。",
              "", "## 3. 时间／元数据探针", "",
              "在 dev 上拟合固定深度 3、最小叶节点 3 的分类树，测试不参与拟合或调参。无正文输入。",
              "", "| 测试基线 | Accuracy | Macro-F1 |", "|---|---:|---:|"]
    for key, label in [("age_only_depth3", "仅 age_days"), ("metadata_only_depth3", "时间＋trust＋importance＋access")]:
        c = audit["baselines"][key]["scores"]["test"]
        lines.append(f"| {label} | {c['accuracy']*100:.2f}% | {c['macro_f1_all']*100:.2f}% |")
    lines += ["", "仅测试多数类基线为 48.84%。单个 age 阈值效果下降不能证明泄漏已消除；"
              "更强的时间／元数据探针仍能预测相当一部分标签。相关性不自动等于非法泄漏，但当前构造"
              "不足以证明模型必须理解语义才能做好任务。", "", "## 4. 后续协议", "",
              "1. 保留纯治理分类作为诊断，同时恢复下游检索任务验收。固定候选池、排序器、预算和问题，"
              "只替换 none/oracle/rule/LLM 的状态预测；Oracle gap 应指同一排序条件下 oracle 与自动治理的任务得分差。"
              "`1 - accuracy` 已改名 classification_error_rate，不能替代检索 Oracle gap。",
              "2. 合法的真实时间戳可以进入生产排序；用去时间消融与时间反例检验依赖程度即可。"
              "词法弱基线的下降不自动意味着评测有 bug，加入强语义基线也不应删除原对照。",
              "3. 先做标签审计：active 要有当前适用证据，superseded 要指向替代记录，stale 要有明确失效条件，"
              "duplicate 要有同一命题的来源，unresolved 要有未裁决冲突。证据不足单独标注，不能强行塞进任一 gold 类。",
              "4. 已生成 73 组盲审包。由两名独立标注者按上述规则复核、引用证据并裁决分歧；"
              "不要把两次模型输出冒充人工标注。修复后另存新版本，按会话/实体及模板家族隔离，"
              "在查看新测试结果前冻结规模、类分布、指标和阈值。真实数据与一致的合成单元测试可以并行使用。",
              "", "## 5. 产物与复现", "",
              "- 审计清单：`output/governance_v4_audit.json`。",
              "- 盲审包：`output/governance_v4_blind_review.jsonl`，尚未人工复核；原 ID 映射独立存放。重跑审计不会覆盖已有盲审文件。",
              "- 新结果：`output/auto_state_v4_{all,test}_audited_v2.json`。旧版和首轮匿名输出均保留。",
              "- LoCoMo 的另一轮结果实际为 278 题而非文件名暗示的 300 题：27.45 → 34.88，差 +7.43 分；"
              "仍为 4 组对话的回溯式子集，应按原协议单独呈现，不与治理分类混合验收。",
              "", "```powershell", "python eval/audit_governance_v4.py",
              "python eval/auto_state.py --dataset datasets/zte_memory_v4_test.json --llm --out output/auto_state_v4_test_newrun.json",
              "python eval/auto_state.py --dataset datasets/zte_memory_v4_all.json --llm --out output/auto_state_v4_all_newrun.json",
              "python -m unittest discover -s eval -p test_auto_state.py",
              "python eval/write_governance_report.py", "```", "",
              "评测器拒绝覆盖既有结果。报告生成器读取上述 audited_v2 冻结文件，不自动替换为分数更高的新运行。", ""]
    dest = ROOT / "docs/research/auto-governance-audit.md"
    dest.write_text("\n".join(lines), encoding="utf-8")
    names = ["governance_v4_audit.json", "auto_state_v4_all_audited_v2.json", "auto_state_v4_test_audited_v2.json"]
    evidence = {"summary": {s: r["summary"] for s, r in runs.items()},
                "protocol": {s: r["protocol"] for s, r in runs.items()},
                "sizes": audit["sizes"], "flag_counts": audit["flag_counts"],
                "baselines": audit["baselines"],
                "sha256": {n: hashlib.sha256((ROOT / "output" / n).read_bytes()).hexdigest() for n in names}}
    dest.with_name("auto-governance-evidence.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(dest)


if __name__ == "__main__":
    main()
