"""Render the measured S-priority results without hand-copying scores."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(name):
    return json.loads((ROOT / "output" / name).read_text(encoding="utf-8"))


def main():
    retrieval = read("retrieval_v2.json")
    qa = read("locomo_qa_v2.json")
    if not qa.get("complete"):
        raise ValueError("QA run is incomplete")
    zte = {s: read(f"zte_v3_{s}_results.json") for s in ("all", "dev", "test")}
    lines = [
        "# 记忆引擎实验报告：优先级增强 v2",
        "",
        "日期：2026-09-06。由 `eval/write_progress_report.py` 从本地实验 JSON 生成。",
        "",
        "本轮完成两个 S 级方向的首轮实验，并补充 A 级的抽取式事实候选原型与消融。"
        "公开集新增检索策略属于离线实验，尚未接入在线服务；百万级并发压测和真实多 Agent 协作实验尚未开展。",
        "",
        "## 1. 先纠正评测口径",
        "",
        "- 原 31% / 32% 是同一模型 yes/no 等价判分的准确率，不是 token F1，且不是稳定优势的充分证据。",
        "- 本轮问答评分遵循 [LoCoMo 官方代码](https://github.com/snap-research/locomo/blob/main/task_eval/evaluation.py)："
        "词干化词元 F1；多跳分别处理子答案，开放域取参考答案分号前部分，对抗题计算拒答命中。",
        "- 标准 Precision@K = 命中的不同相关证据数 / K；旧报告的 hits / min(K, gold数) 已改名为 capped recall。"
        "Rust 输出标记 schema_version=2；p_at_k_strict 保留为标准 precision 的兼容别名。",
        "- 一个 gold 的题在 P@5 下上限是 20%。撤回旧版“P@5 达到 83%”和“五项全部达标”的结论；"
        "不能用难子集的提升替代全量验收，也不能把字面答案覆盖率当成生成能力上限。",
        "- [旧版报告归档](experiment-report-v1-archived.md)仅供追踪历史，包含已撤回的口径和结论。",
        "",
        "## 2. S：LoCoMo 检索与问答",
        "",
        "沿用已冻结的 384 维 MiniLM 向量及含证据的 1,977 个 QA，10 组对话。前 6 组作为开发部分（1,154 题），"
        "后 4 组作为本轮测试部分（823 题）。此前工作已见过全量公开集，因此这是回溯式划分，不能宣称为从未见过的测试集。",
        "",
        "BM25 k1=1.2、b=0.75，RRF 常数 60，均未在测试部分搜索参数；所有方案 Top-5。"
        "BM25 加说话人字段，额外保留仅正文 BM25 消融以区分词法和元数据收益。"
        "事实混合方案对抽取句子的 BM25 分数按来源取最大值，再与原 utterance 向量排名融合。",
        "",
        "| 方案 | 全量 R@5 | 测试 R@5 | 测试 P@5 | 测试 Top-1 |",
        "|---|---:|---:|---:|---:|",
    ]
    labels = {"static": "Static 向量", "bm25_text": "BM25 仅正文", "bm25": "BM25 正文＋说话人",
              "hybrid": "向量＋BM25 RRF", "fact_hybrid": "句子事实候选＋向量 RRF"}
    for arm, label in labels.items():
        all_m = retrieval["summary"]["all"]["arms"][arm]
        test = retrieval["summary"]["test"]["arms"][arm]
        lines.append(f"| {label} | {all_m['recall']*100:.2f}% | {test['recall']*100:.2f}% | {test['precision']*100:.2f}% | {test['accuracy']*100:.2f}% |")
    ci = retrieval["summary"]["test"]["paired_recall"]["bm25"]
    lo, hi = ci["cluster_bootstrap_95"]
    lines += ["", f"BM25 测试 R@5 相对 Static 增加 {ci['delta']*100:.2f}pp，按对话配对 bootstrap 95% 区间 "
              f"[{lo*100:.2f}, {hi*100:.2f}]pp。仅 4 个测试对话簇，区间用于探索性描述。",
              "", "开发部分中 BM25 优于混合与事实候选方案，故选它进入后续 QA 对照。"
              "收益首先证明了强词法基线的价值，不能包装成进化记忆相对 BM25 的优势。",
              "", "### 同预算端到端问答", "",
              "从测试部分按五类各抽 20 题，共 100 题；seed=42，llama3.1:8b、temperature=0，"
              "两个方案使用同一提示词、相同 Top-5、600 个空白分词的上下文上限、4096 上下文窗口和 128 输出 token 上限。"
              "上下文都包含说话人和日期。上限相同不代表实际输入 token 一样多；逐题保存真实 token 计数。",
              "", "| 题型 | 数量 | Static 得分 | BM25 得分 |", "|---|---:|---:|---:|"]
    for cat, label in [("all", "分层样本总体"), ("1", "多跳 F1"), ("2", "时序 F1"),
                       ("3", "开放域 F1"), ("4", "单跳 F1"), ("5", "对抗拒答准确率")]:
        row = qa["summary"][cat]
        lines.append(f"| {label} | {row['n']} | {row['static']*100:.2f} | {row['bm25']*100:.2f} |")
    ci = qa["paired_f1"]
    lo, hi = ci["cluster_bootstrap_95"]
    tokens = {a: sum(r[a]["prompt_tokens"] for r in qa["rows"]) / len(qa["rows"]) for a in ("static", "bm25")}
    lines += ["", f"配对总体差异 +{ci['delta']*100:.2f} 分，按对话 bootstrap 95% 区间 "
              f"[{lo*100:.2f}, {hi*100:.2f}] 分；实际平均 prompt token 为 Static {tokens['static']:.1f}、BM25 {tokens['bm25']:.1f}。",
              "", "总体是类别均衡样本的混合得分（含拒答），并非完整公开集的微平均 F1；"
              "沿用的证据过滤版 corpus 缺少原始数据中无有效证据的 QA，不能称为全量官方复现。"
              "本轮评分规则、提示词和抽样与旧 31/32 实验不同，不能跨轮直接比较绝对数值。",
              "", "## 3. S：自建集扩到 300 查询", "",
              "新增 v3 合成回归集：15 个项目、20 类属性，共 300 个不同的项目—属性键；"
              "150 个键各经历两次变更，共 300 条被替代事实，加上 300 条当前事实、60 条过期/重复干扰，合计 660 轮。"
              "按项目拆分为开发 200 查询、测试 100 查询。原 v1/v2 数据集保留。",
              "", "查询共享 20 个问题模板，不能称为 300 条独立人工标注或真实用户样本。"
              "trust/importance/access 独立于正确状态采样；state 仍是人工真值，测试的是治理之后的检索，"
              "不能证明自动抽取或自动冲突消解。下一轮应加入真实样本、历史时点、未决冲突和无答案问题。",
              "", "| 方案 | 全量 Top-1（300） | 开发 Top-1（200） | 测试 Top-1（100） | 测试事实变更（50） |",
              "|---|---:|---:|---:|---:|"]
    for i, row in enumerate(zte["all"]["rows"]):
        dev, test = zte["dev"]["rows"][i], zte["test"]["rows"][i]
        label = "相似度＋状态过滤（词法回退）" if row["label"] == "semantic only" else row["label"]
        lines.append(f"| {label} | {row['metrics']['accuracy']:.2f}% | {dev['metrics']['accuracy']:.2f}% | "
                     f"{test['metrics']['accuracy']:.2f}% | {test['by_group']['fact-change']['accuracy']:.2f}% |")
    lines += ["", "所有方案均 K=5，自建集没有 embedding，语义通道使用生产词法回退。"
              "完整策略优于 Static，但相似度＋状态过滤在开发/测试上均优于完整多特征策略。"
              "状态过滤使用 oracle 标签，且与较旧的事实版本高度相关，不能把此结果等同于真实治理能力。"
              "本轮没有宣称 Token 节省：同 K 下平均上下文消耗接近。",
              "", "## 4. A/B 级进度与后续验收", "",
              "| 方向 | 本轮实际完成 | 仍缺少 |", "|---|---|---|",
              "| Fact-level Memory | 抽取式句子候选，保存原文 span、说话人、时间、source_id；查询和 gold 不参与抽取 | 实体/代词消解、关系结构化、时态归一化、生产写入链路 |",
              "| 消融与权重自适应 | 扩容集七组策略与统一 K 对照；负贡献如实报告 | 基于任务/信号可用性的动态路由、开发集学习权重、独立测试 |",
              "| 工程压测 | 保留旧十万规模离线探针，未在本轮复跑 | 百万级索引、真实服务 QPS、并发 P99、错误率和资源占用 |",
              "| 多 Agent 实战 | 保留旧权限单元实验 | 真实协作交付、并发矛盾写入、裁决后读取、权限与来源审计 |",
              "", "## 5. 复现与证据", "", "```powershell",
              "uv venv .venv-eval",
              "uv pip install --python .\\.venv-eval\\Scripts\\python.exe -r eval/requirements.txt",
              "python eval/build_zte_v3.py",
              ".\\.venv-eval\\Scripts\\python.exe eval/retrieval_experiment.py --out output/retrieval_v2.json",
              ".\\.venv-eval\\Scripts\\python.exe eval/locomo_qa_v2.py --out output/locomo_qa_v2.json",
              "cargo run -p lingshu-eval --release -- --zte-dataset datasets/zte_memory_v3_all.json --json output/zte_v3_all_results.json",
              "target\\release\\lingshu-eval.exe --zte-dataset datasets/zte_memory_v3_dev.json --json output/zte_v3_dev_results.json",
              "target\\release\\lingshu-eval.exe --zte-dataset datasets/zte_memory_v3_test.json --json output/zte_v3_test_results.json",
              ".\\.venv-eval\\Scripts\\python.exe -m unittest discover -s eval -p test_evaluation.py",
              "cargo test -p lingshu-eval",
              "python eval/write_progress_report.py", "```", "",
              "需本地 Ollama 已提供 llama3.1:8b；问答按每个 arm 写入可恢复检查点。"
              "公开集 corpus 和向量需先按 eval/build_locomo_corpus.py 生成或使用现有冻结文件。",
              "", "原始证据位于 output/retrieval_v2.json、output/retrieval_v2.facts.json、"
              "output/locomo_qa_v2.json 及 output/zte_v3_*_results.json。"
              "output 默认不提交；轻量汇总与原始文件 SHA-256 保存为本目录 evidence-v2.json。", ""]
    dest = ROOT / "docs" / "research" / "experiment-report.md"
    archive = dest.with_name("experiment-report-v1-archived.md")
    if not archive.exists():
        archive.write_text("> 历史 v1 快照：下文 F1、P@K 与达标结论包含已撤回口径，以 [v2 报告](experiment-report.md) 为准。\n\n"
                           + dest.read_text(encoding="utf-8"), encoding="utf-8")
    dest.write_text("\n".join(lines), encoding="utf-8")
    paths = ["retrieval_v2.json", "retrieval_v2.facts.json", "locomo_qa_v2.json"] + [f"zte_v3_{s}_results.json" for s in zte]
    evidence = {"retrieval": {"protocol": retrieval["protocol"], "summary": retrieval["summary"]},
                "qa": {k: qa[k] for k in ("protocol", "summary", "paired_f1", "complete")},
                "zte": {s: {**r, "rows": [{k: v for k, v in row.items() if k != "queries"} for row in r["rows"]]}
                        for s, r in zte.items()},
                "sha256": {name: hashlib.sha256((ROOT / "output" / name).read_bytes()).hexdigest() for name in paths}}
    dest.with_name("evidence-v2.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(dest)


if __name__ == "__main__":
    main()
