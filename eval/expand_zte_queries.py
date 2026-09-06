#!/usr/bin/env python3
"""Expand the in-house ZTE regression set's queries from 12 to 54.

The original set has 60 turns but only 12 queries, so a single query carries
8.33pp of resolution -- which makes repeated-experiment stddev artificially
large (one query flips = ±8.33pp). Covering every *active* fact raises the
query count to 54 and drops per-query resolution to ~1.85pp, making the mean
robust. This is test enhancement (more coverage over the same memories), not
data fabrication: the memories are unchanged, and every new query has a single
unambiguous gold id.

Output: datasets/zte_memory_60turn_v2.json (v1 left untouched).
"""

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "datasets" / "zte_memory_50turn_v1.json"
DST = ROOT / "datasets" / "zte_memory_60turn_v2.json"

# fact_key -> natural-language query. Expected id is resolved from the active turn.
# `HARD_FACT_KEYS` are the fact-change facts (superseded + active), the core
# "long-horizon memory" scenario: the system must retrieve the *current* fact
# over the stale one. These are tagged group "fact-change" for a separate
# hard-vs-easy breakdown in the report.
HARD_FACT_KEYS = {
    "profile.city",
    "project.stack",
    "project.deadline",
    "team.size",
    "preference.editor",
    "deployment.target",
}

QUERIES = {
    "profile.city": "用户现在常住哪里",
    "project.stack": "项目现在主要使用什么技术栈",
    "project.deadline": "项目最新交付截止日期是什么时候",
    "team.size": "团队目前共有多少名成员",
    "preference.editor": "用户现在偏好的编辑器是什么",
    "deployment.target": "系统当前优先部署在哪里",
    "target.latency": "项目检索平均时延要求是多少",
    "target.turns": "项目至少要连续测试多少轮",
    "feature.memory_scope": "记忆网关如何区分私有和公共空间",
    "layer.experience": "三层记忆中的通用经验层是什么",
    "feature.static_baseline": "系统是否支持静态 RAG 基线模式",
    "feature.adaptive_rerank": "系统怎样进行自适应多特征重排",
    # --- 新增 42 条，覆盖其余 active fact ---
    "preference.coffee": "用户喝咖啡加不加糖",
    "project.weekly": "项目周会安排在什么时间",
    "profile.pet": "用户养了什么宠物",
    "project.database": "项目当前使用什么数据库",
    "preference.theme": "用户偏好浅色还是深色界面",
    "project.repository": "项目代码托管在哪个平台",
    "preference.focus_time": "用户通常在什么时段处理复杂任务",
    "project.license": "项目采用什么开源许可证",
    "preference.reply_style": "用户希望回答保持什么风格",
    "project.port": "项目后端监听哪个端口",
    "project.cache": "项目用哪个组件做可选缓存",
    "profile.exercise": "用户每周运动几次",
    "project.frontend": "项目界面使用哪个前端框架",
    "profile.language": "用户常用什么语言",
    "project.vector": "项目向量库使用哪个引擎",
    "preference.external_action": "用户是否允许自动发送外部消息",
    "project.desktop": "项目桌面端基于什么框架",
    "preference.input": "用户偏好用什么方式操作输入",
    "project.platform": "项目需要支持哪个桌面平台",
    "preference.weekend": "用户周末是否安排会议",
    "project.logging": "项目日志默认采用什么格式",
    "preference.privacy": "用户希望数据如何存储",
    "project.frontend_test": "项目前端测试使用什么框架",
    "preference.provenance": "用户是否希望保留记忆来源链",
    "target.accuracy": "项目要求记忆准确率达到多少",
    "target.ablation": "项目是否需要输出消融实验",
    "feature.scene_tags": "检索候选是否包含场景标签",
    "feature.source_trust": "记忆系统是否记录来源可信度",
    "feature.conflict_audit": "冲突事件是否必须可审计",
    "feature.retrieval_run": "系统是否保存每次检索运行证据",
    "layer.episode": "三层记忆中的原始片段层是什么",
    "layer.fact": "三层记忆中的结构化事实层是什么",
    "feature.stale": "过期记忆是否会被标记为 stale",
    "feature.superseded": "被替代事实是否会进入自适应召回",
    "preference.music": "用户喜欢在什么天气听什么音乐",
    "project.docs": "项目文档使用什么格式",
    "preference.configurable": "用户是否希望重要参数可配置",
    "target.video": "项目演示视频目标时长是多少",
    "target.deploy_doc": "项目交付是否包含部署手册",
    "target.architecture": "项目交付是否包含架构图",
    "target.dataset": "项目交付是否包含标准化数据集",
    "target.report": "项目交付是否包含对比实验报告",
}


def main():
    data = json.loads(SRC.read_text(encoding="utf-8"))

    # Resolve active id per fact_key.
    active_id = {}
    for turn in data["turns"]:
        if turn["state"] == "active":
            active_id[turn["fact_key"]] = turn["id"]

    queries = []
    missing = []
    for fact_key, text in QUERIES.items():
        if fact_key in active_id:
            q = {"query": text, "expected": active_id[fact_key]}
            if fact_key in HARD_FACT_KEYS:
                q["group"] = "fact-change"
            queries.append(q)
        else:
            missing.append(fact_key)

    if missing:
        raise SystemExit(f"missing active fact for: {missing}")

    data["name"] = "LingShu-ZTE-60Turn-v2"
    data["description"] = (
        "可复现的 60 轮时序记忆、冲突、过期与噪声检索样本；"
        "v2 将查询从 12 扩充到 54，覆盖全部 active 事实，单查询分辨率约 1.85pp；"
        "6 个事实变更查询标记 group=fact-change，用于难易分层的达标对照。"
    )
    data["queries"] = queries

    DST.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"queries: {len(data['queries'])} (was 12)")
    print(f"wrote {DST}")


if __name__ == "__main__":
    main()
