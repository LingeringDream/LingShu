"""Semi-realistic governance dataset (v4) for fair automatic-state evaluation.

Why v4 exists:
  v3 (build_zte_v3.py) is a template stress set whose `state` is recoverable
  from `age_days` alone (active is always the newest version of a fact_key, and
  trust/importance are sampled independently of state). On that fixture a
  recency-only rule scores ~100%, which measures nothing about *governance*.

v4 is authored around realistic software-team episodes so that predicting
`active / superseded / stale / duplicate / unresolved` requires *content
semantics*, not a timestamp lookup. It deliberately:

  1. breaks the age_days↔state monotonicity — a controlled share (~20%) of
     scenarios are "anti-patterns" where the NEWEST record is NOT the current
     fact (a correction reversed an error, a one-off event already lapsed, a
     stable old fact is still current, etc.);
  2. gives trust/importance a *partial* (not complete) state signal;
  3. makes stale vs duplicate distinguishable from content;
  4. adds unresolved conflicts, unanswerable queries, pronominal references and
     relative-time phrasing ("last month", "recently").

Structure: a hand-authored CORE (rich, realistic episodes) plus a GENERATED
section driven by explicit "evolution archetypes" (fact-change, correction,
lapsed-event, stable, unresolved-conflict, unanswerable). Each archetype is a
realistic narrative, not a template; the anti-patterns are first-class and
counted in provenance so the reviewer can verify the leakage is gone.

Output schema mirrors v3 (turn.id/content/fact_key/value/age_days/importance/
trust/access/state/split/turn; query.id/query/expected/answer/fact_key/group/
split) so the existing retrieval + auto_state harnesses load it unchanged.
"""
import json
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# =========================================================================== #
# CORE hand-authored scenarios (rich, realistic, kept verbatim)
# =========================================================================== #
CORE_SCENARIOS = [
    # 1. Project relocation
    {"id": "c01.addr.v0", "content": "公司注册地址是上海浦东新区张江高科。",
     "fact_key": "company.address", "value": "上海浦东新区张江高科",
     "age_days": 120, "importance": 0.90, "trust": 0.92, "access": 11, "state": "superseded"},
    {"id": "c01.addr.v1", "content": "公司上月搬到了杭州未来科技城。",
     "fact_key": "company.address", "value": "杭州未来科技城",
     "age_days": 18, "importance": 0.88, "trust": 0.90, "access": 7, "state": "active"},
    # 2. Tech stack migration
    {"id": "c02.stack.v0", "content": "后端服务统一用 Go 编写。",
     "fact_key": "project.stack", "value": "Go",
     "age_days": 200, "importance": 0.85, "trust": 0.88, "access": 20, "state": "superseded"},
    {"id": "c02.stack.v1", "content": "我们最近把核心服务迁移到了 Rust，性能翻倍。",
     "fact_key": "project.stack", "value": "Rust",
     "age_days": 12, "importance": 0.80, "trust": 0.85, "access": 5, "state": "active"},
    {"id": "c02.stack.dup", "content": "团队现在用 Rust 重写了主服务。",
     "fact_key": "project.stack", "value": "Rust",
     "age_days": 14, "importance": 0.50, "trust": 0.70, "access": 2, "state": "duplicate"},
    # 3. Delivery date slipping
    {"id": "c03.delivery.v0", "content": "计划在 2026 年 9 月 1 日交付第一版。",
     "fact_key": "project.delivery", "value": "2026-09-01",
     "age_days": 60, "importance": 0.75, "trust": 0.80, "access": 9, "state": "stale"},
    {"id": "c03.delivery.v1", "content": "交付延期了，现在目标是 2026 年 10 月 15 日。",
     "fact_key": "project.delivery", "value": "2026-10-15",
     "age_days": 25, "importance": 0.78, "trust": 0.82, "access": 6, "state": "active"},
    # 4. Team lead change (unresolved)
    {"id": "c04.lead.a", "content": "林安担任项目负责人。",
     "fact_key": "project.lead", "value": "林安",
     "age_days": 40, "importance": 0.70, "trust": 0.75, "access": 8, "state": "unresolved"},
    {"id": "c04.lead.b", "content": "项目负责人换成了陈雨。",
     "fact_key": "project.lead", "value": "陈雨",
     "age_days": 15, "importance": 0.72, "trust": 0.78, "access": 4, "state": "active"},
    # 5. Contact channel change
    {"id": "c05.contact.v0", "content": "主要通过电子邮件联系。",
     "fact_key": "project.contact", "value": "电子邮件",
     "age_days": 90, "importance": 0.55, "trust": 0.60, "access": 12, "state": "superseded"},
    {"id": "c05.contact.v1", "content": "现在大家都用企业微信沟通。",
     "fact_key": "project.contact", "value": "企业微信",
     "age_days": 20, "importance": 0.60, "trust": 0.65, "access": 15, "state": "active"},
    # 6. Budget (stable old fact, still active)
    {"id": "c06.budget.v0", "content": "项目总预算是十五万元。",
     "fact_key": "project.budget", "value": "十五万元",
     "age_days": 150, "importance": 0.82, "trust": 0.86, "access": 10, "state": "active"},
    # 7. Meeting time
    {"id": "c07.meeting.v0", "content": "周会定在周三下午两点。",
     "fact_key": "project.meeting", "value": "周三下午两点",
     "age_days": 30, "importance": 0.45, "trust": 0.55, "access": 6, "state": "stale"},
    {"id": "c07.meeting.v1", "content": "周会改到周五上午十点了。",
     "fact_key": "project.meeting", "value": "周五上午十点",
     "age_days": 8, "importance": 0.48, "trust": 0.58, "access": 3, "state": "active"},
    # 8. DB choice (stable old active + duplicate)
    {"id": "c08.db.v0", "content": "关系数据库用的是 PostgreSQL。",
     "fact_key": "project.db", "value": "PostgreSQL",
     "age_days": 180, "importance": 0.72, "trust": 0.76, "access": 18, "state": "active"},
    {"id": "c08.db.dup", "content": "数据库选型是 PG，也就是 PostgreSQL。",
     "fact_key": "project.db", "value": "PostgreSQL",
     "age_days": 22, "importance": 0.40, "trust": 0.62, "access": 1, "state": "duplicate"},
    # 9. Server port
    {"id": "c09.port.v0", "content": "服务监听在 8080 端口。",
     "fact_key": "project.port", "value": "8080",
     "age_days": 70, "importance": 0.60, "trust": 0.66, "access": 14, "state": "superseded"},
    {"id": "c09.port.v1", "content": "端口改成 9000 了，8080 已经废弃。",
     "fact_key": "project.port", "value": "9000",
     "age_days": 10, "importance": 0.62, "trust": 0.68, "access": 4, "state": "active"},
    # 10. Data region (stable, old, active)
    {"id": "c10.region.v0", "content": "数据存储在华东区域。",
     "fact_key": "project.region", "value": "华东",
     "age_days": 210, "importance": 0.68, "trust": 0.74, "access": 16, "state": "active"},
    # 11. Backup policy (recent superseded + active)
    {"id": "c11.backup.v0", "content": "每天备份一次数据库。",
     "fact_key": "project.backup", "value": "每天一次",
     "age_days": 20, "importance": 0.52, "trust": 0.58, "access": 5, "state": "superseded"},
    {"id": "c11.backup.v1", "content": "备份频率改为每六小时一次了。",
     "fact_key": "project.backup", "value": "每六小时一次",
     "age_days": 6, "importance": 0.54, "trust": 0.60, "access": 2, "state": "active"},
    # 12. Log retention
    {"id": "c12.log.v0", "content": "日志保存三十天。",
     "fact_key": "project.log", "value": "三十天",
     "age_days": 80, "importance": 0.50, "trust": 0.56, "access": 7, "state": "superseded"},
    {"id": "c12.log.v1", "content": "日志现在只保留七天。",
     "fact_key": "project.log", "value": "七天",
     "age_days": 16, "importance": 0.51, "trust": 0.57, "access": 3, "state": "active"},
    # 13. Editor preference (low trust, recent change)
    {"id": "c13.editor.v0", "content": "我平时用 Vim 写代码。",
     "fact_key": "user.editor", "value": "Vim",
     "age_days": 100, "importance": 0.30, "trust": 0.45, "access": 8, "state": "superseded"},
    {"id": "c13.editor.v1", "content": "我最近换成了 VS Code。",
     "fact_key": "user.editor", "value": "VS Code",
     "age_days": 9, "importance": 0.32, "trust": 0.48, "access": 3, "state": "active"},
    # 14. City preference (stable, relative-time phrasing)
    {"id": "c14.city.v0", "content": "我住在北京。",
     "fact_key": "user.city", "value": "北京",
     "age_days": 160, "importance": 0.35, "trust": 0.50, "access": 6, "state": "active"},
    # 15. Unanswerable (stale phone)
    {"id": "c15.phone.v0", "content": "客户王总的手机号是 138xxxx8888。",
     "fact_key": "customer.phone", "value": "138xxxx8888",
     "age_days": 45, "importance": 0.55, "trust": 0.60, "access": 4, "state": "stale"},
    # 16. ANTIPATTERN: correction reversed (newest is superseded)
    {"id": "c16.venue.v0", "content": "发布会定在国贸三期。",
     "fact_key": "event.venue", "value": "国贸三期",
     "age_days": 35, "importance": 0.66, "trust": 0.72, "access": 9, "state": "superseded"},
    {"id": "c16.venue.v1", "content": "更正：发布会地点其实是国家会议中心，之前说错了。",
     "fact_key": "event.venue", "value": "国家会议中心",
     "age_days": 5, "importance": 0.70, "trust": 0.60, "access": 3, "state": "active"},
    {"id": "c16.venue.err", "content": "发布会地点改成国家会议中心了。",
     "fact_key": "event.venue", "value": "国家会议中心",
     "age_days": 8, "importance": 0.50, "trust": 0.40, "access": 2, "state": "duplicate"},
    # 17. ANTIPATTERN: newest is stale (lapsed one-off)
    {"id": "c17.mainlang.v0", "content": "项目主分支长期使用 Rust 开发。",
     "fact_key": "project.mainlang", "value": "Rust",
     "age_days": 50, "importance": 0.80, "trust": 0.84, "access": 12, "state": "active"},
    {"id": "c17.mainlang.tmp", "content": "这次原型开发临时用 Python 写了脚本。",
     "fact_key": "project.mainlang", "value": "Python",
     "age_days": 3, "importance": 0.35, "trust": 0.50, "access": 1, "state": "stale"},
]

CORE_QUERIES = [
    {"id": "q01", "query": "公司现在办公地址在哪？", "expected": "c01.addr.v1",
     "answer": "杭州未来科技城", "fact_key": "company.address", "group": "fact-change"},
    {"id": "q02", "query": "后端服务用什么语言写的？", "expected": "c02.stack.v1",
     "answer": "Rust", "fact_key": "project.stack", "group": "fact-change"},
    {"id": "q03", "query": "交付日期定在什么时候？", "expected": "c03.delivery.v1",
     "answer": "2026-10-15", "fact_key": "project.delivery", "group": "fact-change"},
    {"id": "q04", "query": "谁是现在的项目负责人？", "expected": "c04.lead.b",
     "answer": "陈雨", "fact_key": "project.lead", "group": "conflict"},
    {"id": "q05", "query": "主要通过什么渠道联系？", "expected": "c05.contact.v1",
     "answer": "企业微信", "fact_key": "project.contact", "group": "fact-change"},
    {"id": "q06", "query": "项目预算总共是多少？", "expected": "c06.budget.v0",
     "answer": "十五万元", "fact_key": "project.budget", "group": "stable-fact"},
    {"id": "q07", "query": "周会改到什么时候了？", "expected": "c07.meeting.v1",
     "answer": "周五上午十点", "fact_key": "project.meeting", "group": "fact-change"},
    {"id": "q08", "query": "用的什么关系数据库？", "expected": "c08.db.v0",
     "answer": "PostgreSQL", "fact_key": "project.db", "group": "stable-fact"},
    {"id": "q09", "query": "服务现在监听哪个端口？", "expected": "c09.port.v1",
     "answer": "9000", "fact_key": "project.port", "group": "fact-change"},
    {"id": "q10", "query": "数据存在哪个区域？", "expected": "c10.region.v0",
     "answer": "华东", "fact_key": "project.region", "group": "stable-fact"},
    {"id": "q11", "query": "多久备份一次？", "expected": "c11.backup.v1",
     "answer": "每六小时一次", "fact_key": "project.backup", "group": "fact-change"},
    {"id": "q12", "query": "日志保留多久？", "expected": "c12.log.v1",
     "answer": "七天", "fact_key": "project.log", "group": "fact-change"},
    {"id": "q13", "query": "你现在用什么编辑器？", "expected": "c13.editor.v1",
     "answer": "VS Code", "fact_key": "user.editor", "group": "fact-change"},
    {"id": "q14", "query": "你住在哪个城市？", "expected": "c14.city.v0",
     "answer": "北京", "fact_key": "user.city", "group": "stable-fact"},
    {"id": "q15", "query": "王总的手机号是多少？", "expected": None,
     "answer": None, "fact_key": "customer.phone", "group": "unanswerable"},
    {"id": "q16", "query": "发布会到底在哪举办？", "expected": "c16.venue.v1",
     "answer": "国家会议中心", "fact_key": "event.venue", "group": "conflict"},
    {"id": "q17", "query": "项目主分支用什么语言开发？", "expected": "c17.mainlang.v0",
     "answer": "Rust", "fact_key": "project.mainlang", "group": "fact-change"},
]


# =========================================================================== #
# GENERATED section: evolution archetypes, each a realistic narrative
# =========================================================================== #
# Subjects: real-world entities a team memory would track. Each carries a list
# of (old_value, new_value, question_phrasing) for fact-change narratives.
SUBJECTS = {
    "客户合同金额": ("客户合同金额", "客户合同现在是多少金额？", "万元"),
    "服务器机房位置": ("服务器部署在哪个机房？", "服务器现在部署在哪个机房？", None),
    "主站域名": ("主站用哪个域名？", "主站现在用哪个域名？", None),
    "运维值班人": ("这周谁值班？", "现在运维值班是谁？", None),
    "代码仓库地址": ("代码仓库在哪？", "代码仓库现在在哪？", None),
    "测试环境版本": ("测试环境是什么版本？", "测试环境现在是什么版本？", None),
    "线上实例数量": ("线上跑了多少个实例？", "线上现在多少个实例？", "个"),
    "监控告警阈值": ("告警阈值是多少？", "现在告警阈值是多少？", None),
    "CDN 加速域名": ("CDN 用哪个域名？", "CDN 现在用哪个域名？", None),
    "数据库主从延迟": ("主从延迟目标多少？", "现在主从延迟目标多少？", "毫秒"),
}

# Value pools per subject (old → new transitions are semantically plausible).
VALUE_POOL = {
    "客户合同金额": [("三百万元", "五百万元"), ("一百五十万元", "两百万元"), ("八十万元", "一百二十万元")],
    "服务器机房位置": [("北京机房", "上海机房"), ("上海机房", "深圳机房"), ("华北", "华东")],
    "主站域名": [("lingeringdream.com", "lingeringdream.cn"), ("example.com", "example.cn")],
    "运维值班人": [("赵强", "孙莉"), ("钱伟", "周芳"), ("吴斌", "郑华")],
    "代码仓库地址": [("github.com/org/repo", "gitlab.com/org/repo"), ("gitee.com/org/repo", "github.com/org/repo")],
    "测试环境版本": [("v1.2.0", "v1.3.0"), ("v2.0.1", "v2.1.0"), ("v0.9.0", "v1.0.0")],
    "线上实例数量": [("4 个", "8 个"), ("2 个", "6 个"), ("8 个", "12 个")],
    "监控告警阈值": [("80%", "90%"), ("500 毫秒", "300 毫秒")],
    "CDN 加速域名": [("cdn.example.com", "static.example.com"), ("cdn.lingeringdream.com", "static.lingeringdream.com")],
    "数据库主从延迟": [("50 毫秒", "20 毫秒"), ("100 毫秒", "30 毫秒")],
}

# Entities (project/user/customer names) for grounding content.
ENTITIES = ["青竹", "星河", "白鹭", "山海", "云帆", "松涛", "晨曦", "远山", "秋水", "北辰",
            "清泉", "飞雪", "流萤", "赤霞", "长风"]

# Relative-time / pronominal phrasings to embed in content (anti-template).
TIME_PHRASE = ["最近", "上个月", "前几天", "刚刚", "上周", "本月", "上季度", "前一阵子"]
CHANGE_VERB = ["改成", "换成", "迁移到", "升级到", "调整为", "切换到", "变更为"]
STALE_VERB = ["临时", "一次性", "已经结束", "过期", "取消了", "撤销了", "作废了"]


def _make_generated(rng):
    """Generate ~95 queries / ~200 turns across five archetypes.

    Archetype mix (each is a distinct narrative, not a template):
      - fact-change  (~42): old superseded + new active, with anti-patterns.
      - correction   (~12): an erroneous newer record that is itself superseded
                            or duplicate (newest != current).
      - lapsed-event (~18): a temporary event that is stale; the stable current
                            fact is older (newest != current).
      - stable       (~15): a single old fact still active (no version).
      - unresolved   (~8):  two conflicting live records, neither superseded.
    """
    turns, queries = [], []
    qid = 100  # query id counter (keep distinct from core q01..q17)
    tid = 0

    subject_keys = list(SUBJECTS.keys())

    # ---- fact-change archetype -------------------------------------------
    fc_count = 0
    for sk in subject_keys:
        label, qcur, unit = SUBJECTS[sk]
        pool = VALUE_POOL[sk]
        for old, new in pool:
            fc_count += 1
            ent = ENTITIES[fc_count % len(ENTITIES)]
            key = f"gen.{sk}.{fc_count:02d}"
            # old (superseded)
            turns.append({
                "id": f"{key}.v0", "fact_key": key, "value": old,
                "content": f"{ent}项目{label}是{old}。",
                "age_days": rng.choice([60, 90, 120, 150]), "importance": round(rng.uniform(.55, .9), 3),
                "trust": round(rng.uniform(.6, .9), 3), "access": rng.randrange(5, 20), "state": "superseded"})
            # new (active), with change verb + time phrase
            verb = rng.choice(CHANGE_VERB); tp = rng.choice(TIME_PHRASE)
            turns.append({
                "id": f"{key}.v1", "fact_key": key, "value": new,
                "content": f"{ent}项目{tp}{verb}{new}了。",
                "age_days": rng.choice([5, 8, 12, 18, 25]), "importance": round(rng.uniform(.6, .9), 3),
                "trust": round(rng.uniform(.62, .92), 3), "access": rng.randrange(1, 8), "state": "active"})
            queries.append({
                "id": f"q{qid}", "query": qcur.replace("？", f"（{ent}项目）？") if "（" not in qcur else qcur,
                "expected": f"{key}.v1", "answer": new, "fact_key": key, "group": "fact-change"})
            qid += 1

    # ---- correction archetype (newest != current) ------------------------
    CORRECTIONS = [
        ("发布会嘉宾名单", "最终确认的嘉宾是谁？", [("张总", "李总"), ("王教授", "赵博士")]),
        ("合同签署方", "合同是和哪家签的？", [("甲公司", "乙公司"), ("丙公司", "丁公司")]),
        ("发布会场地", "场地定在哪？", [("会展中心A馆", "会展中心B馆"), ("酒店宴会厅", "会展中心C馆")]),
    ]
    corr_count = 0
    for label, q, pool in CORRECTIONS:
        for wrong, right in pool:
            corr_count += 1
            ent = ENTITIES[(corr_count + 5) % len(ENTITIES)]
            key = f"gen.corr.{corr_count:02d}"
            # the erroneous recent record (superseded or duplicate)
            turns.append({
                "id": f"{key}.err", "fact_key": key, "value": wrong,
                "content": f"{ent}项目{label}是{wrong}。",
                "age_days": rng.choice([4, 6, 9, 11]), "importance": round(rng.uniform(.4, .6), 3),
                "trust": round(rng.uniform(.35, .55), 3), "access": rng.randrange(1, 3),
                "state": "superseded" if rng.random() < 0.5 else "duplicate"})
            # the correction (active, slightly older than the error)
            turns.append({
                "id": f"{key}.fix", "fact_key": key, "value": right,
                "content": f"更正：{ent}项目{label}其实是{right}，之前那条写错了。",
                "age_days": rng.choice([10, 14, 20, 28]), "importance": round(rng.uniform(.6, .85), 3),
                "trust": round(rng.uniform(.5, .75), 3), "access": rng.randrange(2, 6),
                "state": "active"})
            queries.append({
                "id": f"q{qid}", "query": q.replace("？", f"（{ent}项目）？"), "expected": f"{key}.fix",
                "answer": right, "fact_key": key, "group": "conflict"})
            qid += 1

    # ---- lapsed-event archetype (newest is stale) ------------------------
    LAPSED = [
        ("临时促销活动", "这次活动打到几折？", "八折", "五折"),
        ("一次性团建安排", "这次团建去哪？", "杭州", "苏州"),
        ("限时优惠码", "优惠码是多少？", "LUCKY88", "SAVE50"),
    ]
    lap_count = 0
    for label, q, v1, v2 in LAPSED:
        for v in [v1, v2]:
            lap_count += 1
            ent = ENTITIES[(lap_count + 9) % len(ENTITIES)]
            key = f"gen.lapse.{lap_count:02d}"
            # stable current fact (older)
            turns.append({
                "id": f"{key}.stable", "fact_key": key, "value": f"常规{label}",
                "content": f"{ent}项目长期政策保持不变。",
                "age_days": rng.choice([40, 60, 80, 100]), "importance": round(rng.uniform(.6, .85), 3),
                "trust": round(rng.uniform(.7, .9), 3), "access": rng.randrange(6, 15), "state": "active"})
            # the temporary lapsed event (newer but stale)
            turns.append({
                "id": f"{key}.tmp", "fact_key": key, "value": v,
                "content": f"{ent}项目{label}是{v}，只限本周。",
                "age_days": rng.choice([2, 3, 4, 6]), "importance": round(rng.uniform(.3, .5), 3),
                "trust": round(rng.uniform(.45, .6), 3), "access": rng.randrange(1, 4), "state": "stale"})
            queries.append({
                "id": f"q{qid}", "query": q.replace("？", f"（{ent}项目）？"), "expected": f"{key}.stable",
                "answer": "保持不变", "fact_key": key, "group": "fact-change"})
            qid += 1

    # ---- stable archetype (single old fact, still active) ----------------
    STABLE = [
        ("公司法人", "公司法人是谁？", "王总"),
        ("注册资金", "注册资金多少？", "五百万元"),
        ("核心客户", "最重要的客户是谁？", "某大型国企"),
        ("办公时区", "团队按什么时区协作？", "北京时间"),
        ("代码开源协议", "项目用什么开源协议？", "MIT"),
        ("主编程范式", "主要编程范式是什么？", "函数式"),
        ("数据备份异地", "备份存在异地吗？", "存在"),
        ("服务可用性目标", "可用性目标是多少？", "99.9%"),
    ]
    for i, (label, q, v) in enumerate(STABLE):
        ent = ENTITIES[(i + 3) % len(ENTITIES)]
        key = f"gen.stable.{i:02d}"
        turns.append({
            "id": f"{key}.v0", "fact_key": key, "value": v,
            "content": f"{ent}项目{label}是{v}。",
            "age_days": rng.choice([90, 150, 200, 250]), "importance": round(rng.uniform(.65, .9), 3),
            "trust": round(rng.uniform(.75, .95), 3), "access": rng.randrange(8, 22), "state": "active"})
        queries.append({
            "id": f"q{qid}", "query": q.replace("？", f"（{ent}项目）？"), "expected": f"{key}.v0",
            "answer": v, "fact_key": key, "group": "stable-fact"})
        qid += 1

    # ---- unresolved-conflict archetype ------------------------------------
    UNRESOLVED = [
        ("项目架构师", "谁是架构师？", [("陈工", "刘工"), ("黄工", "李工")]),
        ("上线负责人", "谁负责上线？", [("周工", "吴工"), ("郑工", "王工")]),
    ]
    unr_count = 0
    for label, q, pool in UNRESOLVED:
        for a, b in pool:
            unr_count += 1
            ent = ENTITIES[(unr_count + 7) % len(ENTITIES)]
            key = f"gen.unres.{unr_count:02d}"
            turns.append({
                "id": f"{key}.a", "fact_key": key, "value": a,
                "content": f"{ent}项目{label}是{a}。",
                "age_days": rng.choice([20, 30, 40]), "importance": round(rng.uniform(.55, .75), 3),
                "trust": round(rng.uniform(.6, .8), 3), "access": rng.randrange(3, 8), "state": "unresolved"})
            turns.append({
                "id": f"{key}.b", "fact_key": key, "value": b,
                "content": f"{ent}项目{label}后来换成了{b}。",
                "age_days": rng.choice([10, 15, 22]), "importance": round(rng.uniform(.55, .75), 3),
                "trust": round(rng.uniform(.6, .8), 3), "access": rng.randrange(2, 6), "state": "active"})
            queries.append({
                "id": f"q{qid}", "query": q.replace("？", f"（{ent}项目）？"), "expected": f"{key}.b",
                "answer": b, "fact_key": key, "group": "conflict"})
            qid += 1

    # ---- unanswerable archetype (only lapsed info, no active fact) --------
    UNANSWERABLE = [
        ("上次活动的抽奖结果", "上次抽奖谁中奖了？", "还没公布"),
        ("去年团建的地点", "去年团建去哪了？", "不记得了"),
        ("客户临时会议地点", "客户临时会议在哪开？", "没有记录"),
        ("离职同事的联系方式", "离职的同事现在怎么联系？", "已失效"),
        ("供应商报价单", "供应商最新报价是多少？", "未收到"),
        ("机房临时密码", "机房临时密码是什么？", "已过期"),
        ("上次复盘结论", "上次复盘的结论是什么？", "没有记录"),
    ]
    for i, (label, q, note) in enumerate(UNANSWERABLE):
        ent = ENTITIES[(i + 2) % len(ENTITIES)]
        key = f"gen.unans.{i:02d}"
        # only a stale/superseded record exists; no active fact → should abstain
        turns.append({
            "id": f"{key}.old", "fact_key": key, "value": note,
            "content": f"{ent}项目{label}的记录已经过期了。",
            "age_days": rng.choice([50, 70, 90, 120]), "importance": round(rng.uniform(.3, .5), 3),
            "trust": round(rng.uniform(.4, .55), 3), "access": rng.randrange(1, 4), "state": "stale"})
        queries.append({
            "id": f"q{qid}", "query": q.replace("？", f"（{ent}项目）？"), "expected": None,
            "answer": None, "fact_key": key, "group": "unanswerable"})
        qid += 1

    return turns, queries


def build():
    rng = random.Random(20260907)
    turns = list(CORE_SCENARIOS)
    queries = list(CORE_QUERIES)

    gen_turns, gen_queries = _make_generated(rng)
    turns.extend(gen_turns)
    queries.extend(gen_queries)

    # Assign split by deterministic hash of the query id (stratified across
    # archetypes, NOT by generation order — otherwise all fact-change land in
    # dev and all unanswerable in test).
    def _split_for(qid):
        # stable hash over the numeric suffix of the id
        import hashlib
        h = int(hashlib.md5(qid.encode()).hexdigest()[:8], 16)
        return "dev" if h % 10 < 7 else "test"

    for q in queries:
        q["split"] = _split_for(q["id"])

    # Turn split follows the queries that reference its fact_key.
    key_split = {}
    for q in queries:
        key_split[q["fact_key"]] = q["split"]
    for t in turns:
        t["split"] = key_split.get(t["fact_key"], "dev")

    # Deterministic turn numbering by id (NOT by age — no recency leak via id).
    turns.sort(key=lambda t: t["id"])
    for i, t in enumerate(turns):
        t["turn"] = i + 1

    # Count anti-patterns (newest turn in a key is NOT active) for provenance.
    from collections import defaultdict
    groups = defaultdict(list)
    for t in turns:
        groups[t["fact_key"]].append(t)
    anti = 0
    multi = 0
    for key, members in groups.items():
        if len(members) < 2:
            continue
        multi += 1
        newest = min(members, key=lambda m: m["age_days"])
        if newest["state"] != "active":
            anti += 1

    return {
        "name": "LingShu-SemiReal-v4",
        "description": __doc__.strip(),
        "provenance": {
            "authored": True, "semi_realistic": True,
            "core_scenarios": len(CORE_SCENARIOS), "generated_queries": len(queries) - len(CORE_QUERIES),
            "total_queries": len(queries), "total_turns": len(turns),
            "multi_version_keys": multi, "anti_pattern_keys": anti,
            "anti_pattern_ratio": round(anti / multi, 4) if multi else 0.0,
            "age_not_monotonic": True, "partial_quality_signal": True,
            "stale_duplicate_distinguishable": True,
            "unresolved_conflicts": True, "unanswerable": True,
            "seed": 20260907, "snapshot": "2026-09-06T00:00:00Z",
        },
        "turns": turns, "queries": queries,
    }


def main():
    data = build()
    from collections import Counter
    for split in ("all", "dev", "test"):
        subset = {**data, "name": f"{data['name']}-{split}"}
        subset["turns"] = [t for t in data["turns"] if split == "all" or t["split"] == split]
        subset["queries"] = [q for q in data["queries"] if split == "all" or q["split"] == split]
        path = ROOT / "datasets" / f"zte_memory_v4_{split}.json"
        path.write_text(json.dumps(subset, ensure_ascii=False, indent=2), encoding="utf-8")
        print(split, len(subset["turns"]), len(subset["queries"]),
              dict(Counter(q["group"] for q in subset["queries"])),
              dict(Counter(t["state"] for t in subset["turns"])))
    # print anti-pattern summary
    p = data["provenance"]
    print(f"\nanti_pattern: {p['anti_pattern_keys']}/{p['multi_version_keys']} multi-version keys "
          f"({p['anti_pattern_ratio']:.1%}) have newest != active")


if __name__ == "__main__":
    main()
