"""300 synthetic current-fact queries over 300 distinct entity/attribute keys.

150 keys have two superseded versions. This is generated stress/regression
coverage, not 300 independently authored or real-world observations. Splits
are by project entity (10 development, 5 test), not by paraphrase.
States are oracle annotations: this tests retrieval after governance, not
automatic fact extraction or conflict detection.
"""
import json
import random
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ENTITIES = ["青竹", "星河", "白鹭", "山海", "云帆", "松涛", "晨曦", "远山", "秋水", "北辰",
            "清泉", "飞雪", "流萤", "赤霞", "长风"]
# Attribute, question, three possible versions. Values are synthetic fixtures.
FIELDS = [
    ("办公城市", "办公地点在哪座城市", ["杭州", "成都", "武汉"]),
    ("主要编程语言", "主要使用哪种编程语言", ["Python", "Rust", "Go"]),
    ("交付日期", "计划什么时候交付", ["2026年10月8日", "2026年11月12日", "2026年12月20日"]),
    ("成员人数", "团队有多少位成员", ["5人", "8人", "12人"]),
    ("默认编辑器", "开发默认用什么编辑器", ["Vim", "VS Code", "Zed"]),
    ("部署平台", "优先部署在哪个平台", ["本地服务器", "私有云", "边缘设备"]),
    ("关系数据库", "使用什么关系数据库", ["PostgreSQL", "MySQL", "SQLite"]),
    ("周会时间", "每周什么时候开例会", ["周一上午九点", "周三下午两点", "周五上午十点"]),
    ("项目负责人", "由谁担任项目负责人", ["林安", "陈雨", "周宁"]),
    ("预算额度", "预算总额度是多少", ["十万元", "十五万元", "二十万元"]),
    ("服务端口", "服务监听哪个端口", ["8080", "9000", "3000"]),
    ("代码托管平台", "代码在哪个平台托管", ["GitHub", "GitLab", "Gitea"]),
    ("备份周期", "多长时间备份一次", ["每天一次", "每周一次", "每六小时一次"]),
    ("日志保留时长", "日志保存多久", ["七天", "三十天", "九十天"]),
    ("默认界面语言", "默认界面使用哪种语言", ["简体中文", "英文", "繁体中文"]),
    ("接口响应目标", "接口响应时间目标是多少", ["100毫秒", "300毫秒", "500毫秒"]),
    ("主联系渠道", "主要通过什么渠道联系", ["电子邮件", "企业微信", "飞书"]),
    ("测试框架", "使用什么测试框架", ["pytest", "Vitest", "Playwright"]),
    ("发布周期", "每隔多久发布一个版本", ["一周", "两周", "一个月"]),
    ("数据存储区域", "数据存储在哪个区域", ["华东", "华北", "华南"]),
]


def build():
    rng = random.Random(20260906)
    turns, queries = [], []
    for ei, entity in enumerate(ENTITIES):
        split = "dev" if ei < 10 else "test"
        for fi, (field, question, values) in enumerate(FIELDS):
            changed = (ei + fi) % 2 == 0
            key = f"project-{ei:02d}.field-{fi:02d}"
            order = list(values)
            rng.shuffle(order)
            versions = order if changed else order[-1:]
            for vi, value in enumerate(versions):
                active = vi == len(versions) - 1
                mid = f"{key}.v{vi}"
                turns.append({"id": mid, "content": f"{entity}项目的{field}是{value}。",
                              "fact_key": key, "value": value,
                              "age_days": (5 + fi % 5) if active else 90 - vi * 30,
                              # Quality features sampled independently of state; no gold proxy.
                              "importance": round(rng.uniform(.3, .9), 3),
                              "trust": round(rng.uniform(.65, .95), 3), "access": rng.randrange(10),
                              "state": "active" if active else "superseded", "split": split})
            queries.append({"id": f"q-{ei:02d}-{fi:02d}", "query": f"{entity}项目当前{question}？",
                            "expected": mid, "answer": value, "fact_key": key,
                            "group": "fact-change" if changed else "stable-fact", "split": split})
        # Stale and duplicate distractors exercise filtering; they are not additional queries.
        for ni in range(4):
            turns.append({"id": f"noise-{ei:02d}-{ni}", "content": f"{entity}项目临时会议安排在周二下午三点。",
                          "fact_key": f"project-{ei:02d}.temporary-{ni}", "value": "周二下午三点",
                          "age_days": 45, "importance": .7, "trust": .8, "access": 3,
                          "state": "stale" if ni < 2 else "duplicate", "split": split})
    turns.sort(key=lambda t: (-t["age_days"], t["id"]))
    for i, turn in enumerate(turns):
        turn["turn"] = i + 1
    validate(turns, queries)
    return {"name": "LingShu-ZTE-Synthetic-v3", "description": __doc__.strip(),
            "provenance": {"synthetic": True, "seed": 20260906, "independent_entity_attribute_keys": 300,
                           "changed_keys": 150, "question_templates": 20, "oracle_states": True,
                           "split_unit": "project entity", "snapshot": "2026-09-06T00:00:00Z"},
            "turns": turns, "queries": queries}


def validate(turns, queries):
    ids = {t["id"]: t for t in turns}
    assert len(ids) == len(turns)
    assert len({q["query"] for q in queries}) == len(queries) == 300
    assert len({q["fact_key"] for q in queries}) == 300
    assert sum(q["group"] == "fact-change" for q in queries) == 150
    for q in queries:
        gold = ids[q["expected"]]
        assert gold["state"] == "active" and gold["fact_key"] == q["fact_key"]
        assert q["answer"] == gold["value"] and gold["split"] == q["split"]
        chain = [t for t in turns if t["fact_key"] == q["fact_key"]]
        assert sum(t["state"] == "active" for t in chain) == 1
        if q["group"] == "fact-change":
            assert len(chain) == 3 and len({t["value"] for t in chain}) == 3
            assert all(t["age_days"] > gold["age_days"] for t in chain if t != gold)


def main():
    data = build()
    for split in ("all", "dev", "test"):
        subset = {**data, "name": f"{data['name']}-{split}"}
        subset["turns"] = [t for t in data["turns"] if split == "all" or t["split"] == split]
        subset["queries"] = [q for q in data["queries"] if split == "all" or q["split"] == split]
        path = ROOT / "datasets" / f"zte_memory_v3_{split}.json"
        path.write_text(json.dumps(subset, ensure_ascii=False, indent=2), encoding="utf-8")
        print(split, len(subset["turns"]), len(subset["queries"]), dict(Counter(q["group"] for q in subset["queries"])))


if __name__ == "__main__":
    main()
