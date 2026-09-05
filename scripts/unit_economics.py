# -*- coding: utf-8 -*-
"""
灵枢（LingShu）订阅分级 · 每用户成本自底向上测算模型

收款方案：微信支付 + 支付宝（直连），并与原 Stripe 方案对比。

口径：C 端云服务的"直接成本"（营业成本口径），不含研发人力（计入期间费用）、
      不含官方托管模型推理成本（文档中已声明独立核算，单列参考行）。
单价：2026-09 公开报价，USD/CNY = 7.20
输出：订阅分级每人成本测算报告.html
"""
from __future__ import annotations

from datetime import date

FX = 7.20  # USD -> CNY

# ---------------------------------------------------------------- 云资源单价
WORKERS_BASE_USD = 5.00          # Workers Paid 月保底，含 10M 请求 + 30M CPU-ms
WORKERS_REQ_USD_PER_M = 0.30     # 超额请求
WORKERS_CPU_USD_PER_M_MS = 0.02  # 超额 CPU-ms
D1_READ_USD_PER_M = 0.001        # 超额 rows read（含 25B/月）
D1_WRITE_USD_PER_M = 1.00        # 超额 rows written（含 50M/月）
D1_STORAGE_USD_PER_GB = 0.75     # 超额存储（含 5GB）
R2_STORAGE_USD_PER_GB = 0.015    # 标准存储（10GB 免费额度）
R2_A_USD_PER_M = 4.50            # Class A：写入/列取
R2_B_USD_PER_M = 0.36            # Class B：读取（egress 免费）
RESEND_FREE_EMAILS = 3_000       # 免费额度
RESEND_PRO_USD = 20.00           # Pro：5 万封/月
RESEND_OVER_USD_PER_K = 0.90
SUPPORT_HOURLY = 60.00           # 客服人力（学生团队兼职口径，元/小时）
DOMAIN_YEAR = 100.00             # 域名（元/年）

# ---------------------------------------------------------------- 收款渠道
# pct=费率；fixed=每笔固定费；dispute_fee=单笔争议费；dispute_rate=争议率；
# compliance_year=主体与记账的年度维护成本（元/年）
CHANNELS = {
    "cn": dict(
        name="微信支付 + 支付宝（直连）",
        pct=0.006, fixed=0.00, dispute_fee=0.00, dispute_rate=0.000,
        compliance_year=2_400,
        desc="费率 0.6%，无单笔固定费、无拒付罚金；微信退款退还手续费，提现免费",
    ),
    "stripe": dict(
        name="Stripe（原方案）",
        pct=0.029, fixed=2.00, dispute_fee=150.00, dispute_rate=0.002,
        compliance_year=5_000,
        desc="境内 2.9% + ¥2.00/笔，国际卡 4.4% + ¥2.00；争议 ¥150/笔",
    ),
}
PRIMARY = "cn"   # 主方案

# ---------------------------------------------------------------- 主体与收款路径
# compliance_year：主体注册 + 代账 + 年检的年度维护成本（元/年）
ENTITY = {
    "individual": dict(name="个体工商户", compliance_year=2_400,
                       desc="注册门槛低（在校大学生可办），代账约 ¥200/月；"
                            "可开通微信 Native/JSAPI、支付宝网站支付等<b>普通收单</b>产品"),
    "company": dict(name="有限责任公司", compliance_year=6_000,
                    desc="注册 + 代账 + 开户约 ¥500/月；注意新《公司法》要求认缴资本 <b>5 年内实缴</b>；"
                         "是申请<b>自动续费</b>能力的前提"),
}

# 自动续费（周期扣款）能力门槛 —— 2025 年 7 月微信支付新规 + 支付宝现行规则
AUTOPAY_GATES = [
    ("微信支付 委托代扣（自动续费）",
     "主体仅支持<b>企业、政府/事业单位、社会组织及其服务商</b>——<b>个体工商户已不在准入范围</b>"
     "（2025 年 7 月《微信支付自动续费管理规则调整公告》）；"
     "业务场景限水电煤、通信、保险、公益、以及「电商平台/游戏/在线图书·视频·音乐/短剧/交友/资讯」等，"
     "<b>工具类 SaaS 是否归入开放场景需与支付客户经理确认</b>。"
     "自动续费权限分三档：最近 4 周有效订单去重用户数 ≥300 且升级投诉率 ≤0.05% → 结算周期不变（T+1）；"
     "20–300 且投诉率 ≤0.1% → <b>T+7</b>；新主体 &lt;20 用户 → <b>T+14</b>；按月动态考核。审核 5 个工作日。"),
    ("支付宝 商家扣款（周期自动扣款）",
     "仅<b>企业账号</b>（个体工商户不可）；注册资本 <b>≥ 2,000 万元</b>；"
     "已开通收单产品且<b>近半年每月主动支付用户数 ≥ 300 人</b>。——学生创业团队<b>不可能满足</b>。"),
    ("支付宝 周期扣款",
     "企业账号<b>或个体工商户</b>；提交营业执照且<b>注册资本 ≥ 100 万元</b>；企业无经营风险记录。"
     "（认缴制下注册资本写 100 万可行，但受 5 年实缴约束。）"
     "注：与「商家扣款」为不同产品页，两处口径不一致，<b>以实际申请结果为准</b>。"),
]

# 三条落地路径
PATHS = [
    dict(key="A", name="路径 A：会员体系 + 时长卡（手动续费）",
         entity="individual", autopay=False,
         desc="会员分级（免费/开发者/高级/团队）保持不变，但权益按<b>时长</b>出售："
              "1 个月 / 3 个月 / 1 年，到期前站内 + 邮件提醒手动续费。只依赖普通收单能力。",
         pros="个体工商户即可，2 周内可上线；0.6% 费率下按不按次付费都不增加成本；"
              "没有自动扣款纠纷与投诉率考核，不触发结算周期延长。",
         cons="续费率显著低于自动续费（行业经验值：自动续费 70%–90% vs 手动续费 30%–50%，"
              "<b>需实测验证</b>）；现金流按月/季到账而非年付预付。",
         cost_note="支付成本与订阅制完全相同（0.6%，无单笔固定费）"),
    dict(key="B", name="路径 B：注册公司 + 申请自动续费",
         entity="company", autopay=True,
         desc="先注册有限责任公司（认缴注册资本 100 万起），再申请微信委托代扣 / 支付宝周期扣款，做真正的自动续费。",
         pros="续费率最优、现金流最好（年付预付）、最贴近商业计划书中的「订阅制」叙事。",
         cons="合规成本升至 ¥6,000/年且承担 5 年实缴义务；微信侧还受「4 周去重用户数」考核，"
              "按文档假设第 1 年仅 100 付费用户，<b>会落到 T+7 甚至 T+14 结算周期</b>；"
              "工具类 SaaS 的场景归类存在过审不确定性；支付宝商家扣款（2,000 万）完全够不到。",
         cost_note="合规成本 +¥3,600/年；T+7 结算占用现金流"),
    dict(key="C", name="路径 C：分阶段（推荐）",
         entity="individual", autopay=False,
         desc="第 1 年走路径 A（个体户 + 时长卡）验证付费意愿；<b>同期注册公司</b>，"
              "待付费用户数、投诉率、经营记录达标后，第 2 年再申请自动续费并切换。",
         pros="不阻塞上线，也不放弃长期最优形态；公司主体同时是 B 端客户对公转账与签约的前提。",
         cons="需要做一次支付通道的二次改造（订单表设计成渠道无关可将代价降到最低）。",
         cost_note="第 1 年 ¥2,400/年，切换后 ¥6,000/年"),
]

# ---------------------------------------------------------------- 档位定义
TIERS = [
    {
        "key": "free", "name": "免费版", "alias": "免费本地版（商业计划书口径）",
        "note": "云账户 + 设备会话管理 + 本地使用，无 R2 同步",
        "price_monthly": 0.0, "price_yearly": 0.0,
        "usage": dict(req=150, cpu_ms=4, d1_read=800, d1_write=300, d1_mb=0.005,
                      r2_gb=0.0, r2_a=0, r2_b=0, email=0.20,
                      ticket_rate=0.01, ticket_min=15),
    },
    {
        "key": "sync", "name": "开发者版", "alias": "云同步订阅版（商业计划书口径）",
        "note": "加密增量同步 + 多设备冲突历史（R2 稳态 0.15 GB）",
        "price_monthly": 15.0, "price_yearly": 150.0,
        "usage": dict(req=900, cpu_ms=6, d1_read=6_000, d1_write=1_200, d1_mb=0.01,
                      r2_gb=0.15, r2_a=300, r2_b=400, email=0.30,
                      ticket_rate=0.04, ticket_min=20),
    },
    {
        "key": "pro", "name": "高级同步版", "alias": "高级同步版（商业计划书口径）",
        "note": "版本历史回溯 + 冲突消解可视化（R2 稳态 0.45 GB）",
        "price_monthly": 29.0, "price_yearly": 288.0,
        "usage": dict(req=1_400, cpu_ms=6, d1_read=9_000, d1_write=2_500, d1_mb=0.015,
                      r2_gb=0.45, r2_a=500, r2_b=700, email=0.30,
                      ticket_rate=0.05, ticket_min=25),
    },
    {
        "key": "team", "name": "团队版（每席位）", "alias": "团队版（规划中）",
        "note": "共享记忆空间 + 审计日志 + 按席位账单（R2 稳态 0.30 GB/席）",
        "price_monthly": 50.0, "price_yearly": 540.0,
        "usage": dict(req=1_200, cpu_ms=6, d1_read=8_000, d1_write=2_000, d1_mb=0.015,
                      r2_gb=0.30, r2_a=400, r2_b=600, email=0.40,
                      ticket_rate=0.06, ticket_min=25),
    },
    # 决策：不做一次性买断，取消「本地永久版」档位（2026-09-05 定）
]

SCENARIOS = {
    "低": dict(usage=0.50, storage=0.40, ticket=0.50, dispute=0.0),
    "基准": dict(usage=1.00, storage=1.00, ticket=1.00, dispute=None),  # None=用渠道默认
    "高": dict(usage=2.50, storage=2.20, ticket=1.80, dispute=0.005),
}

# 路径 C（分阶段）：第 1 年个体户主体，第 2 年起公司主体
SCALES = [("第1年", 2_000, 0.05, "individual"),
          ("第2年", 8_000, 0.07, "company"),
          ("第3年", 20_000, 0.08, "company")]
YEAR_C_REVENUE = {2_000: 120_000, 8_000: 450_000, 20_000: 1_200_000}
ANNUAL_MIX = 0.60  # 年付占比假设


def usage_of(tier: dict, scen: str) -> dict:
    u, m = tier["usage"], SCENARIOS[scen]
    return dict(
        req=u["req"] * m["usage"], cpu_ms=u["cpu_ms"] * m["usage"],
        d1_read=u["d1_read"] * m["usage"], d1_write=u["d1_write"] * m["usage"],
        d1_mb=u["d1_mb"] * m["storage"], r2_gb=u["r2_gb"] * m["storage"],
        r2_a=u["r2_a"] * m["usage"], r2_b=u["r2_b"] * m["usage"],
        email=u["email"] * m["usage"],
        ticket_rate=u["ticket_rate"] * m["ticket"], ticket_min=u["ticket_min"],
    )


def infra_cost(u: dict) -> dict:
    """按公开单价全额计的每用户基础设施成本（元/月，保守上界：不扣包含额度）。"""
    return dict(
        workers_req=u["req"] / 1e6 * WORKERS_REQ_USD_PER_M * FX,
        workers_cpu=u["req"] * u["cpu_ms"] / 1e6 * WORKERS_CPU_USD_PER_M_MS * FX,
        d1_read=u["d1_read"] / 1e6 * D1_READ_USD_PER_M * FX,
        d1_write=u["d1_write"] / 1e6 * D1_WRITE_USD_PER_M * FX,
        d1_storage=u["d1_mb"] / 1024 * D1_STORAGE_USD_PER_GB * FX,
        r2_storage=u["r2_gb"] * R2_STORAGE_USD_PER_GB * FX,
        r2_ops=(u["r2_a"] / 1e6 * R2_A_USD_PER_M + u["r2_b"] / 1e6 * R2_B_USD_PER_M) * FX,
        email=u["email"] / 1_000 * RESEND_OVER_USD_PER_K * FX,
    )


def support_cost(u: dict) -> float:
    return u["ticket_rate"] * (u["ticket_min"] / 60.0) * SUPPORT_HOURLY


def payment_cost(tier: dict, mode: str, ch: dict, dispute_rate: float) -> float:
    """mode: monthly / yearly / mixed；返回元/月"""
    if not tier["price_monthly"]:
        return 0.0
    if mode == "mixed":
        cost_m = tier["price_monthly"] * ch["pct"] + ch["fixed"]
        cost_y = (tier["price_yearly"] * ch["pct"] + ch["fixed"]) / 12
        return (1 - ANNUAL_MIX) * cost_m + ANNUAL_MIX * cost_y + dispute_rate * ch["dispute_fee"]
    p, months = (tier["price_monthly"], 1) if mode == "monthly" else (tier["price_yearly"], 12)
    return (p * ch["pct"] + ch["fixed"]) / months + dispute_rate * ch["dispute_fee"]


def tier_cost(tier: dict, scen: str, mode: str = "mixed", channel: str = PRIMARY) -> dict:
    ch = CHANNELS[channel]
    u = usage_of(tier, scen)
    dr = SCENARIOS[scen]["dispute"]
    dr = ch["dispute_rate"] if dr is None else dr
    inf = infra_cost(u)
    sup = support_cost(u)
    pay = payment_cost(tier, mode, ch, dr)
    return dict(usage=u, infra=inf, infra_total=sum(inf.values()),
                support=sup, payment=pay, total=sum(inf.values()) + sup + pay)


def breakeven_price(tier: dict, channel: str = PRIMARY, alloc: float = 0.0) -> float:
    """盈亏平衡月费：p = 基础设施 + 客服 + 固定分摊 + p × 费率（无单笔固定费时与月/年付无关）"""
    ch = CHANNELS[channel]
    c = tier_cost(tier, "基准", "monthly", channel)
    fixed_part = c["infra_total"] + c["support"] + alloc
    return fixed_part / (1 - ch["pct"])


def fixed_cost(active: int, emails_per_month: float, channel: str = PRIMARY,
               entity: str = "individual") -> dict:
    resend = 0.0 if emails_per_month <= RESEND_FREE_EMAILS else RESEND_PRO_USD * FX
    compliance = ENTITY[entity]["compliance_year"] / 12
    total = WORKERS_BASE_USD * FX + DOMAIN_YEAR / 12 + resend + compliance
    return dict(workers=WORKERS_BASE_USD * FX, domain=DOMAIN_YEAR / 12, resend=resend,
                compliance=compliance, total=total, per_user=total / active)


def ltv_subscription(price_year: float, monthly_cost: float, renewal_rate: float) -> dict:
    """时长卡/订阅制 LTV：几何续费模型，平均付费年限 = 1/(1-r)"""
    years = 1.0 / (1.0 - renewal_rate)
    revenue = price_year * years
    cost = monthly_cost * 12 * years
    return dict(years=years, revenue=revenue, cost=cost, gross=revenue - cost)


def scale_ledger(active: int, conv: float, channel: str = PRIMARY,
                  entity: str = "individual") -> dict:
    """按真实计费规则（含包含额度）算 C 端月度总账。"""
    ch = CHANNELS[channel]
    paying = active * conv
    free_n = active - paying
    w_sync, w_pro = 0.75, 0.25  # 付费用户档位混合（对应文档表5 云同步订阅版 : 高级同步版）
    u_free = usage_of(TIERS[0], "基准")
    u_sync = usage_of(TIERS[1], "基准")
    u_pro = usage_of(TIERS[2], "基准")

    req = free_n * u_free["req"] + paying * (w_sync * u_sync["req"] + w_pro * u_pro["req"])
    cpu = (free_n * u_free["req"] * u_free["cpu_ms"]
           + paying * (w_sync * u_sync["req"] * u_sync["cpu_ms"] + w_pro * u_pro["req"] * u_pro["cpu_ms"]))
    r2_gb = paying * (w_sync * u_sync["r2_gb"] + w_pro * u_pro["r2_gb"])
    r2_a = paying * (w_sync * u_sync["r2_a"] + w_pro * u_pro["r2_a"])
    r2_b = paying * (w_sync * u_sync["r2_b"] + w_pro * u_pro["r2_b"])
    emails = active * 0.25

    extra_req = max(0.0, req - 10e6) / 1e6 * WORKERS_REQ_USD_PER_M * FX
    extra_cpu = max(0.0, cpu - 30e6) / 1e6 * WORKERS_CPU_USD_PER_M_MS * FX
    r2_storage = max(0.0, r2_gb - 10) * R2_STORAGE_USD_PER_GB * FX
    r2_ops = (r2_a / 1e6 * R2_A_USD_PER_M + r2_b / 1e6 * R2_B_USD_PER_M) * FX
    infra_bill = extra_req + extra_cpu + r2_storage + r2_ops  # D1 在 2 万量级远未触顶

    support = (free_n * support_cost(u_free)
               + paying * (w_sync * support_cost(u_sync) + w_pro * support_cost(u_pro)))
    payment = paying * (w_sync * payment_cost(TIERS[1], "mixed", ch, ch["dispute_rate"])
                        + w_pro * payment_cost(TIERS[2], "mixed", ch, ch["dispute_rate"]))
    fix = fixed_cost(active, emails, channel, entity)["total"]
    total = infra_bill + support + payment + fix
    return dict(active=active, paying=paying, free_n=free_n, r2_gb=r2_gb,
                infra_bill=infra_bill, support=support, payment=payment,
                fixed=fix, total=total, emails=emails)


# ---------------------------------------------------------------- 报告渲染
CSS = """
:root{--ink:#1a1b1c;--sub:#6b7280;--line:#e5e7eb;--bg:#f7f8fa;--blue:#3b82f6;
--green:#16a34a;--red:#dc2626;--amber:#d97706;--okbg:#f0fdf4;--warnbg:#fffbeb;--badbg:#fef2f2;}
*{box-sizing:border-box;margin:0;padding:0;}
body{font-family:"Microsoft YaHei","PingFang SC",-apple-system,sans-serif;color:var(--ink);
background:var(--bg);line-height:1.7;font-size:15px;}
.wrap{max-width:1040px;margin:0 auto;padding:32px 20px 60px;}
header{background:#fff;border:1px solid var(--line);border-radius:12px;padding:28px 32px;margin-bottom:20px;}
h1{font-size:22px;font-weight:700;margin-bottom:6px;}
.sub{color:var(--sub);font-size:13px;}
.verdict{display:flex;gap:14px;align-items:flex-start;margin-top:18px;background:var(--okbg);
border:1px solid #86efac;border-radius:10px;padding:14px 18px;}
.verdict .badge{flex-shrink:0;background:var(--green);color:#fff;font-size:13px;font-weight:700;
padding:4px 12px;border-radius:999px;}
.verdict p{font-size:14px;}
section{background:#fff;border:1px solid var(--line);border-radius:12px;padding:24px 28px;margin-bottom:20px;}
h2{font-size:18px;font-weight:700;margin-bottom:14px;padding-bottom:10px;border-bottom:2px solid var(--blue);}
h3{font-size:15px;font-weight:700;margin:18px 0 8px;}
p{margin-bottom:8px;} ul,ol{margin:6px 0 10px 22px;} li{margin-bottom:5px;}
table{width:100%;border-collapse:collapse;margin:12px 0;font-size:13.5px;}
th,td{border:1px solid var(--line);padding:7px 10px;text-align:left;vertical-align:top;}
th{background:#f1f5f9;font-weight:700;}
td.n,th.n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap;}
.ok{background:var(--okbg);} .warn{background:var(--warnbg);} .bad{background:var(--badbg);}
.card{background:var(--bg);border:1px solid var(--line);border-radius:10px;padding:14px 18px;margin:12px 0;}
.small{font-size:12.5px;color:var(--sub);}
.bar{display:flex;height:22px;border-radius:5px;overflow:hidden;margin:4px 0 10px;border:1px solid var(--line);}
.bar span{display:block;height:100%;}
.legend{display:flex;gap:16px;flex-wrap:wrap;font-size:12.5px;color:var(--sub);margin-bottom:6px;}
.legend i{display:inline-block;width:11px;height:11px;border-radius:3px;margin-right:5px;vertical-align:middle;}
footer{color:var(--sub);font-size:12px;text-align:center;margin-top:24px;}
"""

C_INFRA, C_SUPPORT, C_PAY = "#93c5fd", "#fbbf24", "#f87171"


def money(v: float, d: int = 2) -> str:
    return f"{v:,.{d}f}"


def build_html() -> str:
    today = date.today().isoformat()
    ch = CHANNELS[PRIMARY]
    stripe = CHANNELS["stripe"]
    out = []
    a = out.append

    a(f'<!DOCTYPE html><html lang="zh-CN"><head><meta charset="UTF-8">'
      f'<meta name="viewport" content="width=device-width,initial-scale=1.0">'
      f'<title>灵枢订阅分级 · 每用户成本测算报告（微信/支付宝方案）</title><style>{CSS}</style></head><body><div class="wrap">')
    a('<header>')
    a('<h1>灵枢（LingShu）订阅分级 · 每人每月成本测算</h1>')
    a(f'<div class="sub">测算日期：{today}｜收款方案：<b>微信支付 + 支付宝直连</b>（附 Stripe 对比）'
      f'｜方法：自底向上（Cloudflare Workers / D1 / R2 + Resend + 支付通道 + 客服人力 + 主体合规）'
      f'｜汇率 USD/CNY = {FX}｜口径：C 端直接成本，不含研发人力与官方托管模型推理成本</div>')
    a('<div class="verdict"><span class="badge">核心结论</span><p>'
      '<b>改用微信/支付宝后，成本结构整体换了一副骨架。</b>开发者版（15 元/月）每人每月成本从 Stripe 方案的 '
      '<b>¥3.58 降至 ¥0.93</b>，其中支付通道费从 ¥2.74 降到 <b>¥0.09（-97%）</b>——'
      '因为 0.6% 费率没有每笔 ¥2.00 的固定费，也没有 ¥150/笔的拒付罚金。'
      '现在<b>客服人力（¥0.80，占 86%）是绝对第一大成本</b>，云基础设施（¥0.04）与支付通道（¥0.09）几乎可以忽略。'
      '按 15 元定价，单用户贡献毛利率 <b>94%</b>；C 端整体毛利率三年稳定在 <b>93%–95%</b>。</p></div>')
    a('</header>')

    # ---- 1 速览
    a('<section><h2>一、每人每月成本速览（微信 / 支付宝方案）</h2>')
    a('<p class="small">"变动成本"按公开单价全额计（保守上界，不扣免费额度）；"分摊固定" = 平台月固定成本 ÷ 活跃账号数'
      '（2 万活跃量级，含主体与代账摊销 ¥200/月）；付费档按 60% 年付 / 40% 月付 混合。</p>')
    a('<table><tr><th>订阅分级</th><th class="n">基础设施</th><th class="n">客服人力</th>'
      '<th class="n">支付通道</th><th class="n">分摊固定</th><th class="n">合计（基准）</th>'
      '<th class="n">低情景</th><th class="n">高情景</th><th class="n">等价月费</th><th class="n">贡献毛利率</th></tr>')
    for t in TIERS:
        base = tier_cost(t, "基准")
        low = tier_cost(t, "低")["total"]
        high = tier_cost(t, "高")["total"]
        alloc = fixed_cost(20_000, 5_000)["per_user"]
        total = base["total"] + alloc
        price = t["price_monthly"]
        margin = f'{(price - total) / price * 100:.1f}%' if price else "—（净支出）"
        a(f'<tr><td><b>{t["name"]}</b><br><span class="small">{t["alias"]}</span></td>'
          f'<td class="n">{money(base["infra_total"], 3)}</td>'
          f'<td class="n">{money(base["support"])}</td>'
          f'<td class="n">{money(base["payment"], 3)}</td>'
          f'<td class="n">{money(alloc, 3)}</td>'
          f'<td class="n"><b>{money(total)}</b></td>'
          f'<td class="n">{money(low + alloc)}</td>'
          f'<td class="n">{money(high + alloc)}</td>'
          f'<td class="n">{money(price)}</td>'
          f'<td class="n"><b>{margin}</b></td></tr>')
    a('</table>')
    a('<div class="card"><b>读表提示：</b>免费版是唯一的净支出项（约 ¥0.17/人/月），但它是付费转化的唯一漏斗；'
      '付费档成本以「月付」为基准展示，年付等价月费更低（见第七节）。</div>')
    a('</section>')

    # ---- 2 收款渠道对比
    a('<section><h2>二、收款方案切换：成本影响对比</h2>')
    a('<table><tr><th>科目</th><th class="n">微信 + 支付宝</th><th class="n">Stripe（原方案）</th>'
      '<th class="n">差额</th><th>说明</th></tr>')
    cmp_rows = []
    for t in TIERS:
        if not t["price_monthly"]:
            continue
        cn_m = payment_cost(t, "monthly", CHANNELS["cn"], 0.0)
        st_m = payment_cost(t, "monthly", CHANNELS["stripe"], CHANNELS["stripe"]["dispute_rate"])
        cn_y = payment_cost(t, "yearly", CHANNELS["cn"], 0.0)
        st_y = payment_cost(t, "yearly", CHANNELS["stripe"], CHANNELS["stripe"]["dispute_rate"])
        cmp_rows.append((t, cn_m, st_m, cn_y, st_y))
    a(f'<tr><td><b>费率</b></td><td class="n">0.6%</td><td class="n">2.9% / 国际卡 4.4%</td>'
      f'<td class="n">−2.3 pt</td><td>软件与互联网服务类目标准费率</td></tr>')
    a(f'<tr><td><b>每笔固定费</b></td><td class="n">¥0</td><td class="n">¥2.00</td>'
      f'<td class="n ok">−¥2.00</td><td>小额订阅下这一项就是全部差距的来源</td></tr>')
    a(f'<tr><td><b>拒付 / 争议</b></td><td class="n">¥0（无此机制）</td><td class="n">¥150/笔</td>'
      f'<td class="n ok">−¥0.30</td><td>按 0.2% 争议率计提，国内为平台介入调解，最坏结果是退款本金</td></tr>')
    a(f'<tr><td><b>提现 / 到账</b></td><td class="n">免费，T+1</td><td class="n">免费，T+2 滚动</td>'
      f'<td class="n">—</td><td>微信自动提现至对公账户；支付宝实时到余额</td></tr>')
    a(f'<tr><td><b>主体与合规（年）</b></td><td class="n">¥2,400</td><td class="n">¥5,000</td>'
      f'<td class="n ok">−¥2,600</td><td>大陆个体户/公司 + 代账 vs 境外主体注册 + 秘书年费</td></tr>')
    for t, cn_m, st_m, cn_y, st_y in cmp_rows:
        a(f'<tr><td>{t["name"]} 支付成本（月付）</td><td class="n">{money(cn_m, 3)}</td>'
          f'<td class="n">{money(st_m)}</td><td class="n ok">−{money(st_m - cn_m)}</td>'
          f'<td class="small">降幅 {(1 - cn_m / st_m) * 100:.0f}%</td></tr>')
        a(f'<tr><td>{t["name"]} 支付成本（年付摊月）</td><td class="n">{money(cn_y, 3)}</td>'
          f'<td class="n">{money(st_y)}</td><td class="n ok">−{money(st_y - cn_y)}</td>'
          f'<td class="small">降幅 {(1 - cn_y / st_y) * 100:.0f}%</td></tr>')
    a(f'<tr class="warn"><td><b>开发者版总成本（月付，含客服与云）</b></td>'
      f'<td class="n"><b>¥0.93</b></td><td class="n">¥3.58</td>'
      f'<td class="n ok"><b>−¥2.65</b></td><td class="small">毛利率 76% → 94%</td></tr>')
    a('</table>')
    a('<div class="card warn"><b>结论反转：原先"必须强推年付"的建议不再成立。</b>'
      'Stripe 方案下月付与年付的支付成本差 ¥1.91/月，是最大单笔可优化项；'
      '换成微信/支付宝后这个差距只剩 <b>¥0.016/月</b>，年付的价值从"省手续费"变成"锁定留存 + 提前回款"，'
      '折扣力度可以自己定，不必为了省通道费硬推。</div>')
    a('</section>')

    # ---- 3 单价表
    a('<section><h2>三、计价依据（2026-09 公开报价）</h2>')
    a('<table><tr><th>项目</th><th>计费口径</th><th class="n">单价</th><th>免费 / 包含额度</th></tr>')
    for row in [
        ("Cloudflare Workers", "Paid 计划月保底（含 10M 请求 + 30M CPU-ms）", "$5.00/月", "—"),
        ("Workers 超额请求", "超出 10M 部分", "$0.30/百万", "静态资源请求免费且不计次"),
        ("Workers CPU", "超出 30M ms 部分", "$0.02/百万 ms", "子请求等待不计 CPU"),
        ("D1 读取行", "超出 25B 行/月", "$0.001/百万行", "25B 行/月"),
        ("D1 写入行", "超出 50M 行/月", "$1.00/百万行", "50M 行/月"),
        ("D1 存储", "超出 5GB", "$0.75/GB-月", "5 GB"),
        ("R2 标准存储", "GB-月（按日峰值平均）", "$0.015/GB-月", "10 GB"),
        ("R2 Class A（写入/列取）", "次", "$4.50/百万次", "1,000,000 次/月"),
        ("R2 Class B（读取）", "次", "$0.36/百万次", "10,000,000 次/月"),
        ("R2 出网流量", "GB", "<b>免费</b>", "全免（同步下载不计费）"),
        ("微信支付", "交易金额", "0.6%", "无单笔固定费；退款退还手续费；提现免费"),
        ("支付宝（网站/APP/JSAPI）", "交易金额", "0.6%～1.0%", "一般行业 0.6%；退款退费规则因产品而异"),
        ("Resend 邮件", "封（Free 3,000/月，Pro 5 万封）", "$0.90/千封（超量）", "3,000 封/月"),
        ("客服人力", "小时（学生团队兼职口径）", "¥60", "—"),
        ("域名", "年", "¥100", "—"),
        ("主体与代账", "年（个体户/公司 + 代账）", "¥2,400", "—"),
    ]:
        a(f'<tr><td>{row[0]}</td><td>{row[1]}</td><td class="n">{row[2]}</td><td>{row[3]}</td></tr>')
    a('</table>')
    a('</section>')

    # ---- 4 用量假设
    a('<section><h2>四、每用户每月用量假设（基准情景）</h2>')
    a('<p class="small">依据现有代码实测特征推导：access token TTL 900 秒（活跃时约每 15 分钟刷新一次）、'
      'refresh token 30 天、D1 存 users/devices/sessions/login_attempts 四表、R2 桶 MEMORY_BUCKET 尚未启用（随同步开放）。</p>')
    a('<table><tr><th>驱动项</th>' + "".join(f'<th class="n">{t["name"]}</th>' for t in TIERS) + '<th>推导依据</th></tr>')
    fields = [
        ("Worker 请求（次）", "req", "登录/刷新/设备列表/同步上报；刷新按 16–24 活跃天 × 6 次/天"),
        ("单请求 CPU（ms）", "cpu_ms", "JWT 校验 + D1 查询，子请求等待不计入"),
        ("D1 读取行", "d1_read", "按索引查 session/user + 同步游标；扫描行计入读取"),
        ("D1 写入行", "d1_write", "session.last_used 更新（含索引写）+ 同步游标/冲突历史"),
        ("D1 存储（MB）", "d1_mb", "users/devices/sessions/login_attempts 四表 + 索引"),
        ("R2 稳态存储（GB）", "r2_gb", "记忆文本 + 1536 维向量（6 KB/条）压缩后；高级版含 3 份版本历史"),
        ("R2 Class A（次）", "r2_a", "增量对象上传次数"),
        ("R2 Class B（次）", "r2_b", "跨设备增量拉取次数（出网免费）"),
        ("事务邮件（封）", "email", "注册验证摊薄 + 重置密码 + 安全提醒"),
        ("工单率", "ticket_rate", "付费档咨询/退款/同步问题显著高于免费档"),
        ("单次处理（分钟）", "ticket_min", "含排队、排查与回复"),
    ]
    for label, key, basis in fields:
        cells = []
        for t in TIERS:
            v = t["usage"][key]
            cells.append(f'<td class="n">{v:,.2f}</td>' if key in ("r2_gb", "d1_mb", "email", "ticket_rate")
                         else f'<td class="n">{v:,.0f}</td>')
        a(f'<tr><td>{label}</td>' + "".join(cells) + f'<td class="small">{basis}</td></tr>')
    a('<tr class="warn"><td><b>情景乘数</b></td>'
      '<td colspan="5">低 = 用量×0.5 / 存储×0.4 / 工单×0.5；高 = 用量×2.5 / 存储×2.2 / 工单×1.8</td>'
      '<td class="small">敏感性区间</td></tr>')
    a('</table></section>')

    # ---- 5 成本明细
    a('<section><h2>五、成本明细与结构（基准情景，元/人/月）</h2>')
    a(f'<div class="legend"><span><i style="background:{C_INFRA}"></i>基础设施</span>'
      f'<span><i style="background:{C_SUPPORT}"></i>客服人力</span>'
      f'<span><i style="background:{C_PAY}"></i>支付通道</span></div>')
    for t in TIERS:
        c = tier_cost(t, "基准")
        tot = c["total"]
        if tot <= 0:
            continue
        wi, ws, wp = c["infra_total"] / tot * 100, c["support"] / tot * 100, c["payment"] / tot * 100
        a(f'<h3>{t["name"]}　<span class="small">{t["note"]}</span></h3>')
        a(f'<div class="bar"><span style="width:{wi:.2f}%;background:{C_INFRA}"></span>'
          f'<span style="width:{ws:.2f}%;background:{C_SUPPORT}"></span>'
          f'<span style="width:{wp:.2f}%;background:{C_PAY}"></span></div>')
        a('<table><tr><th>科目</th><th class="n">金额</th><th class="n">占比</th><th>说明</th></tr>')
        labels = {"workers_req": "Workers 请求", "workers_cpu": "Workers CPU", "d1_read": "D1 读取行",
                  "d1_write": "D1 写入行", "d1_storage": "D1 存储", "r2_storage": "R2 存储",
                  "r2_ops": "R2 读写操作", "email": "事务邮件"}
        for k, label in labels.items():
            v = c["infra"][k]
            if v <= 0:
                continue
            a(f'<tr><td>{label}</td><td class="n">{money(v, 4)}</td>'
              f'<td class="n">{v / tot * 100:.2f}%</td><td class="small">按公开单价全额计（未扣包含额度，保守）</td></tr>')
        a(f'<tr class="ok"><td>基础设施小计</td><td class="n">{money(c["infra_total"], 4)}</td>'
          f'<td class="n">{wi:.2f}%</td><td class="small">R2 出网免费是最大红利</td></tr>')
        a(f'<tr><td>客服人力</td><td class="n">{money(c["support"])}</td>'
          f'<td class="n">{ws:.2f}%</td><td class="small">工单率 × 单次时长 × ¥60/小时</td></tr>')
        a(f'<tr><td>支付通道</td><td class="n">{money(c["payment"], 3)}</td>'
          f'<td class="n">{wp:.2f}%</td><td class="small">0.6% × 金额，无单笔固定费</td></tr>')
        a(f'<tr class="warn"><td><b>合计（不含固定分摊）</b></td><td class="n"><b>{money(tot)}</b></td>'
          f'<td class="n">100%</td><td class="small">—</td></tr>')
        a('</table>')
    a('</section>')

    # ---- 6 固定成本
    a('<section><h2>六、平台固定成本与分摊</h2>')
    a('<p class="small">全部跑在 Cloudflare 上，没有常驻服务器：Workers Paid 保底 $5/月即覆盖 D1 25B 读取、'
      '50M 写入、5GB 存储与 10M 请求。2 万用户量级下唯一真正超额的只有 R2 存储。</p>')
    a('<table><tr><th>活跃账号</th><th class="n">Workers 保底</th><th class="n">域名</th>'
      '<th class="n">Resend</th><th class="n">主体与代账</th><th class="n">固定成本合计</th><th class="n">每人分摊</th></tr>')
    for label, active, _, entity in SCALES:
        f = fixed_cost(active, active * 0.25, PRIMARY, entity)
        a(f'<tr><td>{label}（{active:,} 活跃）</td><td class="n">{money(f["workers"])}</td>'
          f'<td class="n">{money(f["domain"])}</td><td class="n">{money(f["resend"])}</td>'
          f'<td class="n">{money(f["compliance"])}</td>'
          f'<td class="n"><b>{money(f["total"])}</b></td>'
          f'<td class="n">{money(f["per_user"], 4)}</td></tr>')
    a('</table>')
    a('<div class="card">邮件按 0.25 封/人/月估：2 万活跃 = 5,000 封/月，超过 Resend 免费 3,000 封，需升级 Pro（$20/月 ≈ ¥144）。'
      '<b>注意：微信/支付宝收款必须有大陆经营主体</b>，个体工商户即可（在校大学生可注册），'
      '代账按 ¥200/月计；这笔 ¥2,400/年 是切换方案后新增的固定成本，但仍远小于 Stripe 方案的主体维护费。</div>')
    a('</section>')

    # ---- 7 定价对照
    a('<section><h2>七、与定价对照：单用户贡献毛利</h2>')
    a('<table><tr><th>订阅分级</th><th class="n">月付成本</th><th class="n">年付成本</th>'
      '<th class="n">混合（60% 年付）</th><th class="n">等价月费</th>'
      '<th class="n">月付毛利率</th><th class="n">年付毛利率</th></tr>')
    for t in TIERS:
        if not t["price_monthly"]:
            continue
        cm = tier_cost(t, "基准", "monthly")["total"]
        cy = tier_cost(t, "基准", "yearly")["total"]
        cx = tier_cost(t, "基准", "mixed")["total"] + fixed_cost(20_000, 5_000)["per_user"]
        price = t["price_monthly"]
        a(f'<tr><td>{t["name"]}</td><td class="n">{money(cm)}</td><td class="n">{money(cy)}</td>'
          f'<td class="n"><b>{money(cx)}</b></td><td class="n">{money(price)}</td>'
          f'<td class="n">{(price - cm) / price * 100:.1f}%</td>'
          f'<td class="n"><b>{(price - cy) / price * 100:.1f}%</b></td></tr>')
    a('</table>')
    alloc20 = fixed_cost(20_000, 5_000)["per_user"]
    be_sync = breakeven_price(TIERS[1], PRIMARY, alloc20)
    be_pro = breakeven_price(TIERS[2], PRIMARY, alloc20)
    a('<div class="card ok"><b>定价下限已经低到没有约束力。</b>按直接成本算（含固定分摊 ¥'
      f'{alloc20:.3f}/人/月），开发者版的盈亏平衡月费只有 <b>¥{be_sync:.2f}/月</b>，'
      f'高级同步版 <b>¥{be_pro:.2f}/月</b>——几乎全部由客服人力构成。'
      '因为 0.6% 方案没有单笔固定费，<b>月付与年付的盈亏平衡点完全相同</b>（这本身也是该渠道的一个特性）。'
      '15 元的定价 100% 由价值和竞品决定（Mem0 $19/月、Zep 企业档约 2.7 万/年），'
      '成本侧完全不构成约束——这一条可以直接写进商业计划书 5.5 节"成本导向为底线"的论证。</div>')
    a('</section>')

    # ---- 8 三年总账
    a('<section><h2>八、三年 C 端总成本总账（按真实阶梯计费）</h2>')
    a('<p class="small">付费用户按 开发者版 : 高级同步版 = 75 : 25 加权；C 端收入取商业计划书表 5 口径。</p>')
    a('<table><tr><th>年度</th><th class="n">活跃</th><th class="n">付费</th><th class="n">R2 用量</th>'
      '<th class="n">基础设施</th><th class="n">客服</th><th class="n">支付</th><th class="n">固定</th>'
      '<th class="n">月总成本</th><th class="n">年总成本</th><th class="n">C端收入</th><th class="n">毛利率</th></tr>')
    for label, active, conv, entity in SCALES:
        r = scale_ledger(active, conv, PRIMARY, entity)
        year_cost = r["total"] * 12
        rev = YEAR_C_REVENUE[active]
        gp = (rev - year_cost) / rev * 100
        a(f'<tr><td>{label}</td><td class="n">{active:,}</td><td class="n">{r["paying"]:,.0f}</td>'
          f'<td class="n">{r["r2_gb"]:,.0f} GB</td>'
          f'<td class="n">{money(r["infra_bill"])}</td><td class="n">{money(r["support"])}</td>'
          f'<td class="n">{money(r["payment"])}</td><td class="n">{money(r["fixed"])}</td>'
          f'<td class="n"><b>{money(r["total"])}</b></td><td class="n">{money(year_cost)}</td>'
          f'<td class="n">{money(rev)}</td><td class="n ok"><b>{gp:.1f}%</b></td></tr>')
    a('</table>')
    a('<div class="card"><b>与商业计划书的冲突点：</b>文档第 8.3 节假设整体毛利率 65%–75% / 70%–80% / 75%–82%，'
      '但自底向上测算 <b>C 端订阅业务本身在 93%–95%</b>。65%–82% 只有在把 B 端交付人力与研发摊销并入"整体"口径时才成立。'
      '建议把 C 端与 B 端毛利率分开披露：<b>C 端 90%–95%（全 serverless + R2 免出网费 + 0.6% 支付费率）、'
      'B 端 55%–70%（含交付与运维人力）</b>，整体加权后落在 70%–82%，既自洽也可辩护。</div>')
    a('</section>')

    # ---- 9 门槛与路径
    a('<section><h2>九、自动续费的真实门槛与三条落地路径</h2>')
    a('<p class="small">成本账算下来很漂亮，但"能不能自动续费"决定了产品形态——'
      '这一条比费率重要得多，它决定你卖的是<b>订阅</b>还是<b>时长卡</b>。</p>')
    a('<h3>1. 自动续费（周期扣款）能力门槛</h3>')
    a('<table><tr><th style="width:26%;">能力</th><th>准入条件（2026-09 核）</th></tr>')
    for name, gate in AUTOPAY_GATES:
        a(f'<tr class="bad"><td><b>{name}</b></td><td>{gate}</td></tr>')
    a('</table>')
    a('<div class="card bad"><b>一句话结论：第 1 年不要指望真正的自动续费。</b>'
      '按商业计划书第 1 年仅 100 名付费用户的假设，微信侧会落到 <b>T+7 甚至 T+14</b> 结算周期'
      '（4 周去重付费用户需 ≥300 才能维持 T+1），支付宝商家扣款的「注册资本 2,000 万 + 月付费用户 300 人」完全够不到，'
      '而微信委托代扣已明确把个体工商户排除在准入范围外。</div>')

    a('<h3>2. 三条落地路径对比</h3>')
    a('<table><tr><th style="width:18%;">路径</th><th>做法</th><th>优势</th><th>代价 / 风险</th>'
      '<th class="n">合规成本</th></tr>')
    for p in PATHS:
        cls = ' class="ok"' if p["key"] == "C" else ""
        a(f'<tr{cls}><td><b>{p["name"]}</b></td><td>{p["desc"]}</td>'
          f'<td>{p["pros"]}</td><td>{p["cons"]}</td>'
          f'<td class="n">¥{ENTITY[p["entity"]]["compliance_year"]:,}/年</td></tr>')
    a('</table>')
    a('<div class="card ok"><b>推荐路径 C（分阶段）：</b>第 1 年用个体工商户 + 时长卡把付费闭环跑起来，'
      '<b>同期注册公司</b>——公司主体不只是为了自动续费，它同时是 B 端客户对公转账、签约与开票的前提，'
      '商业计划书第 2 年就要拓展中小企业 API 客户，这一步迟早要走。'
      '把订单表设计成渠道无关（channel / out_trade_no / status），后续切换自动续费时改造代价最低。</div>')

    a('<h3>3. 其他工程与合规事项</h3>')
    a('<table><tr><th style="width:22%;">事项</th><th>门槛 / 代价</th><th>应对建议</th></tr>')
    a('<tr class="warn"><td><b>必须有大陆经营主体</b></td>'
      '<td>微信支付与支付宝的收单产品均要求企业账号或个体工商户，'
      '需营业执照 + 对公/法人银行账户；注册个体工商户门槛低（在校大学生可办），'
      '代账约 ¥200/月。文档 8.4 节写"现阶段尚未完成外部融资、无外部主体"，这一条需要同步修订。</td>'
      '<td>先注册个体工商户（成本 ¥0–500 + 代账 ¥200/月）；'
      '若未来要接受对公转账的 B 端客户，再升级为有限公司。'
      '注意：主体名称最好与品牌"灵枢"关联，避免用户支付时看到无关商户名产生疑虑。</td></tr>')
    a('<tr class="warn"><td><b>billing.ts 需整体重写</b></td>'
      '<td>现有 <code>lingshu-cloud/src/billing.ts</code> 是完整的 Stripe 实现：'
      'Checkout Session、Webhook 验签、Customer Portal、subscriptions 表与权益判断。'
      '换成微信/支付宝要重做：RSA2 签名与验签、预支付订单、异步回调（幂等 + 重复通知）、'
      '订单状态机、退款接口、对账。单人估计 <b>1.5–2.5 周</b>。</td>'
      '<td>保留 <code>plans</code> 抽象层与权益判断逻辑不动，只替换支付provider；'
      '订单表设计成渠道无关（channel / out_trade_no / amount / status / paid_at），'
      '为将来同时挂 Stripe 留好位置。</td></tr>')
    a('<tr><td><b>回调地址的可达性</b></td>'
      '<td>支付回调需要公网 HTTPS 地址，当前服务跑在 Cloudflare Workers 上'
      '（<code>account.lingeringdream.com</code>）。国内网络访问 Cloudflare 存在不稳定与被干扰的风险，'
      '回调丢单 = 用户付了钱没开通，是最高优先级的故障场景。</td>'
      '<td>回调接口必须做<b>主动查单补偿</b>（定时任务对账未回调订单，调微信/支付宝查单接口确认），'
      '不能只依赖回调。同时监控国内访问成功率，必要时把回调路径单独放到境内服务器。</td></tr>')
    a('<tr><td><b>退款与费率细则差异</b></td>'
      '<td>微信支付退款时手续费<b>按比例退还</b>；支付宝不同产品规则不同——'
      'APP 支付 / 手机网站支付明确"退款时服务费不退回"，JSAPI 与周期扣款页面写"手续费一并退还"。'
      '两家的退款有效期均为交易后 12 个月。</td>'
      '<td>按"退款不退手续费"的保守口径建模（本模型即如此，误差 &lt; ¥0.01/人/月）。'
      '签约时向支付宝业务确认实际签约费率与退费规则，写进退款政策页。</td></tr>')
    a('<tr><td><b>外币收款能力归零</b></td>'
      '<td>微信/支付宝只能收人民币，海外用户（GitHub 上的国际用户、留学生）无法付款。</td>'
      '<td>短期：海外用户引导至"本地免费版 + 自备 API Key"的使用路径，或暂不服务；'
      '中期：主体资质达标后并行接入 Stripe 收国际卡，订单表的 channel 字段已为此预留。</td></tr>')
    a('</table></section>')

    # ---- 10 会员体系 vs 买断（决策：不做买断）
    alloc20 = fixed_cost(20_000, 5_000)["per_user"]
    a('<section><h2>十、为什么坚持会员体系（不做买断）：LTV 视角</h2>')
    a('<p class="small">几何续费模型：平均付费年限 = 1 ÷（1 − 年续费率）；'
      '服务成本取各档「混合支付（60% 年付）+ 固定分摊」的每人月成本。'
      '<b>决策（2026-09-05 定）：不做一次性买断，所有付费形态均为可续费的会员时长卡。</b></p>')
    a('<table><tr><th>方案（会员时长卡）</th><th class="n">年续费率</th><th class="n">平均付费年限</th>'
      '<th class="n">LTV 收入</th><th class="n">LTV 服务成本</th><th class="n">LTV 毛利</th></tr>')
    ltv_rows = []
    for t in TIERS:
        if not t["price_yearly"]:
            continue
        cost_m = tier_cost(t, "基准", "mixed")["total"] + alloc20
        for r in (0.30, 0.40, 0.55):
            l = ltv_subscription(t["price_yearly"], cost_m, r)
            ltv_rows.append((t, r, l))
            cls = ' class="ok"' if r == 0.40 else ""
            a(f'<tr{cls}><td>{t["name"]}（¥{t["price_yearly"]:.0f}/年）</td>'
              f'<td class="n">{r:.0%}</td><td class="n">{l["years"]:.2f} 年</td>'
              f'<td class="n">¥{l["revenue"]:,.0f}</td><td class="n">¥{l["cost"]:,.0f}</td>'
              f'<td class="n"><b>¥{l["gross"]:,.0f}</b></td></tr>')
    a('</table>')

    sync_ltv = next(l for t, r, l in ltv_rows if t["key"] == "sync" and r == 0.40)
    sync_ltv55 = next(l for t, r, l in ltv_rows if t["key"] == "sync" and r == 0.55)
    pro_ltv = next(l for t, r, l in ltv_rows if t["key"] == "pro" and r == 0.40)
    a('<div class="card ok"><b>决策依据：放弃买断、只做会员时长卡。</b>'
      f'开发者版（¥150/年）在 40% 年续费率下 LTV 毛利即达 <b>¥{sync_ltv["gross"]:,.0f}</b>，'
      f'续费率若到 55% 更可到 <b>¥{sync_ltv55["gross"]:,.0f}</b>；高级同步版（¥288/年）同口径约 '
      f'¥{pro_ltv["gross"]:,.0f}。更关键的是三条非财务理由：'
      '① 会员体系形成可续费的 <b>ARR</b>，对融资估值的支撑远强于一次性收入；'
      '② 存在升级路径（开发者版 → 高级同步版 → 团队版），买断无法衔接；'
      '③ 时长卡在 0.6% 费率下按不按次付费都不增加成本，手动续费完全可行，'
      '第 1 年即可用个体工商户 + 时长卡跑通付费闭环，不等自动续费资质。</div>')
    a('<div class="card"><b>对收入结构的直接影响：</b>原商业计划书表 5 中「本地永久版」买断收入'
      '（第 1/2/3 年约 2 / 5 / 15 万元）已取消；该部分客户与收入并入会员时长卡，'
      '<b>C 端收入现 100% 为可续费收入</b>。第八章三年账表的毛利率 93%–95% 仍成立，'
      '且现在全部来自可预测的经常性收入。</div>')
    a('</section>')

    # ---- 11 风险与建议
    a('<section><h2>十一、敏感性、风险与建议</h2>')
    a('<h3>1. 成本敏感项（按影响排序，微信/支付宝方案下）</h3>')
    a('<table><tr><th>敏感项</th><th class="n">影响</th><th>说明</th></tr>'
      '<tr class="bad"><td>客服工单率</td><td class="n">±¥0.7/人/月</td>'
      '<td><b>现在是绝对第一大成本（占 86%）</b>。工单率从 4% 降到 2%（靠文档、自助排查、常见问题机器人）'
      '等于把单位成本砍掉 40%</td></tr>'
      '<tr class="warn"><td>客服人力单价</td><td class="n">±¥0.4/人/月</td>'
      '<td>从学生兼职 ¥60/小时 换成专职 ¥150/小时，成本立刻翻倍；早期必须由团队自己扛</td></tr>'
      '<tr class="warn"><td>主体与代账</td><td class="n">±¥0.1/人/月</td>'
      '<td>2,000 活跃量级下分摊 ¥0.10/人/月，规模越大越摊薄；这是切换方案新增的成本项</td></tr>'
      '<tr><td>R2 存储增长</td><td class="n">±¥0.02/人/月</td>'
      '<td>唯一会随用户使用时长自然累积的成本；20 GB/万活跃 ≈ ¥216/月</td></tr>'
      '<tr><td>支付费率</td><td class="n">±¥0.09/人/月</td>'
      '<td>0.6% 已是最低档；即使涨到 1.0% 也只增加 ¥0.06</td></tr>'
      '<tr><td>汇率</td><td class="n">±10%</td><td>云资源以美元计价；人民币贬值 10% 仅使每人成本上升约 ¥0.004</td></tr>'
      '</table>')

    a('<h3>2. 建议动作</h3>')
    a('<ol>')
    a('<li><b>产品形态：做会员体系，但先卖"时长卡"，不要先做自动续费。</b>'
      '会员分级（免费 / 开发者 / 高级 / 团队）保持不变，这决定了 ARR 叙事与升级路径；'
      '但收款形态先用时长卡（1 个月 / 3 个月 / 1 年 + 到期提醒手动续费），'
      '因为自动续费的门槛（微信：企业主体 + 4 周 300 付费用户；支付宝：注册资本 2,000 万）第 1 年够不到。'
      '等公司主体与经营记录达标后再切换，订单表设计成渠道无关可把二次改造代价降到最低。</li>')
    a('<li><b>不做一次性买断，全部付费形态统一为会员时长卡。</b>'
      '买断会反噬 ARR（用户一年半回本就停付），且无升级路径、对融资估值支撑弱。'
      '取消后，原买断客户并入订阅，C 端收入 100% 可续费；'
      '档位只保留免费 / 开发者 / 高级同步 / 团队四档。</li>')
    a('<li><b>成本优化的重心从"支付"移到"客服"。</b>换渠道后支付成本已无优化空间，'
      '而客服占了 86%。优先级：写好同步与冲突解决的文档 → 做自助诊断页 → 站内常见问题，'
      '目标把付费档工单率从 4% 压到 2%，单位成本直接减半。</li>')
    a('<li><b>免费版按"客服与滥用"设配额，不按云成本。</b>免费版云成本仅 ¥0.003/人/月，'
      '真实风险是批量注册刷验证邮件（Resend 3,000 封/月免费额度会被打穿）和工单。'
      '建议：单账号 R2 上限 100 MB、同步 200 次/月、注册必须邮箱验证 + 轻量风控。</li>')
    a('<li><b>个体工商户先跑通付费闭环，公司主体同期启动、不要拖。</b>'
      '微信/支付宝的普通收单接受个体工商户，在校大学生即可办理（¥0–500 + 代账 ¥200/月），'
      '用来最快验证付费意愿；但公司主体是迟早要走的——它不只是自动续费的前提，'
      '更是 B 端客户对公转账、签约与开票的必要条件，而商业计划书第 2 年就要拓展中小企业 API 客户。'
      '注意新《公司法》要求认缴资本 5 年内实缴，注册资本别填过高。</li>')
    a('<li><b>商业计划书的收款与主体章节要同步修订。</b>8.4 节"无外部主体"、'
      '以及所有提及 Stripe 订阅的表述都要改成"微信支付 + 支付宝 + 时长卡"，'
      '并补充主体资质这一项启动前置条件。</li>')
    a('<li><b>把模型推理成本单列，别混进订阅毛利。</b>若未来 30% 用户使用官方托管模型，'
      '按 30 万 input + 6 万 output token/人/月、¥2/百万 input、¥8/百万 output 估，约 ¥1.1/人/月'
      '（仅对使用者计入），需按量单独收费。</li>')
    a('</ol></section>')

    # ---- 12 口径
    a('<section><h2>十二、口径与边界（哪些没算进来）</h2>')
    a('<ul>')
    a('<li><b>不含研发人力。</b>研发计入期间费用，不影响毛利；若按"完全成本"口径，'
      '把 60 万研发/18 个月按活跃用户分摊，第 1 年（2,000 活跃）高达 ¥278/人/月——'
      '这也是为什么早期必须用毛利而非净利看单位经济。</li>')
    a('<li><b>不含官方托管模型推理成本。</b>文档已声明独立核算，用户可用本地模型（Ollama）或自备 API Key。</li>')
    a('<li><b>不含获客成本（CAC）。</b>靠技术社区内容运营，边际获客成本接近 0 但不等于 0；'
      '建议在商业计划书中单列 CAC 与 LTV。</li>')
    a('<li><b>按"退款不退手续费"的保守口径</b>建模（微信实际会退，误差 &lt; ¥0.01/人/月）；'
      '未计入退款本身的收入冲回。</li>')
    a('<li><b>保守取向。</b>变动成本按公开单价全额计，未扣 Workers 10M 请求、D1 25B 读取等包含额度；'
      '实际账单会低于本报告数值。</li>')
    a('</ul></section>')

    a(f'<footer>测算脚本：<code>scripts/unit_economics.py</code>（可复算）｜单价来源：Cloudflare Workers / D1 / R2 官方定价页、'
      f'微信支付商户文档与企业微信商户号说明、支付宝开放平台各支付产品页、Resend 定价页（2026-09 核）｜'
      f'用量假设依据：<code>lingshu-cloud/src/*</code> 与 <code>wrangler.jsonc</code>。</footer>')
    a('</div></body></html>')
    return "".join(out)


def main() -> None:
    print(f"主方案：{CHANNELS[PRIMARY]['name']}\n")
    print(f"{'档位':<16}{'基础设施':>10}{'客服':>8}{'支付':>8}{'合计':>9}")
    for t in TIERS:
        c = tier_cost(t, "基准")
        print(f"{t['name']:<16}{c['infra_total']:>10.4f}{c['support']:>8.2f}{c['payment']:>8.3f}{c['total']:>9.2f}")
    print()
    for label, active, conv, entity in SCALES:
        r = scale_ledger(active, conv, PRIMARY, entity)
        rev = YEAR_C_REVENUE[active]
        print(f"{label}: 活跃 {active:,} 付费 {r['paying']:,.0f} | 月成本 ¥{r['total']:,.0f} "
              f"| 年成本 ¥{r['total'] * 12:,.0f} | 收入 ¥{rev:,} | 毛利率 {(rev - r['total'] * 12) / rev * 100:.1f}%")

    out_path = r"D:\PA\PA\订阅分级每人成本测算报告.html"
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(build_html())
    print(f"\n报告已生成：{out_path}")


if __name__ == "__main__":
    main()
