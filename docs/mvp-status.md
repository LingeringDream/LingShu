# LingShu MVP 完成度对照 · MVP Status vs PRD

> 对照 `CLAUDE.md` 的 MVP 范围与 `AI-PersonalAssistant-PRD.md`,记录截至当前的实现状态、
> 已知局限与剩余可选项。日期:2026-06。
> 图例:✅ 已实现并测试 · ◐ 已实现但有验证缺口/局限 · ⏳ 未做(可选/非 MVP)

## 一、MVP 四大件(CLAUDE.md 范围)

| 能力 | 状态 | 说明 |
|------|------|------|
| macOS 桌面壳 + 桌面宠物 | ✅ | Tauri 2,main 控制面板窗口 + pet 透明置顶可拖拽浮窗;浏览器模式优雅降级 |
| Apple Calendar | ◐ | 自然语言解析 → 落 PG → L1 逐次确认流 → EventKit 写入系统日历 + 回写 `external_event_id`。**端到端"事件出现在日历 App"待真机验证** |
| SoulLedger | ✅ | 见第二节,六项机制全部落地 |
| 权限分级 L0–L4 | ◐ | 数据模型 + API + 日历处 L1 强制 + 审计日志;**权限/LLM 设置目前仅存内存,重启回默认**(已知局限) |

## 二、SoulLedger 机制(对照设计决策文档六项)

| 机制 | 状态 | 实现要点 |
|------|------|---------|
| 埋点 telemetry | ✅ | append-only `signal_events`,服务端自动采集 + `/api/v1/signals` 客户端入口,白名单校验 |
| 记忆留存 | ✅ | 决策树:显式"记住"→高权;dedup 命中→抬权;否则默认 0.5 交给衰减 |
| 遗忘衰减 + provenance 护栏 | ✅ | 指数半衰减纯逻辑 + 后台冷却扫描软删除;被活跃人格快照 / derived 记忆引用的源豁免遗忘;同步从 Qdrant 移除 |
| 人格 | ✅ | 用户滑块(主)+ 显式反馈样本(赞/踩、风格 chip → few-shot 注入)+ 自动演化(降级、24h 冷却 + 保守 clamp) |
| Thought Queue | ✅ | 状态机 pending→shown→accepted/dismissed/snoozed + expired;防打扰抑制(近 14 天 dismissed)+ 活跃上限 + 每日维护扫描 |
| LLM-as-judge 离线整合 | ✅ | 24h 冷却的离线语义合并,生成 `tier='derived'` 记忆、软降级源、保留来源链 |
| 向量检索 | ✅ | Ollama 嵌入 + Qdrant,带 SQL 回退;保存即建索引,遗忘即删点 |

## 三、安全与质量

| 项 | 状态 | 说明 |
|----|------|------|
| 网络绑定 | ✅ | 默认 `127.0.0.1`(本地单用户、无密码 owner 令牌,不可暴露公网) |
| 集成令牌加密 | ✅ | AES-256-GCM 静态加密(`TokenCipher` 启动派生一次缓存),响应不回传任何 token |
| SQL 注入 | ✅ | 全部参数化、按 `user_id` 作用域 |
| 路由参数 | ✅ | 已修 axum 0.7 `:param` 语法(此前 `{id}` 致 by-id 路由全 404)+ router 级回归测试 |
| 测试 | ✅ | 215 单元 + DB 集成 + 路由回归;clippy `--all-targets --all-features -D warnings` 零警告 |
| CI | ✅ | Rust lint/test、前端 check、Docker(GHCR 小写)、sqlx-cli 锁 0.8.x、actions v5 |

## 四、待办 / 验证缺口 / 可选增强

1. **EventKit 真机端到端验证(建议优先)**:在 macOS 真机 `cargo tauri dev`,授予日历权限,
   走"确认事件 → 打开日历 App 看到它 → `external_event_id` 回写"完整一遍。目前仅验证到后端 PATCH。
2. **推送并确认 CI 全绿**:把工作流/文档相关改动提交推送,确认 Actions 里 CI 与 Docker Build 两个 workflow 均绿。
3. **WebSocket 回声桩替换**(可选,非 MVP):桌面宠物状态 / 主动建议实时推送,目前 `ws/handler.rs` 仍是回声。
4. **Thought 跨会话实体计数触发器**(设计文档 §3 follow-up,可选):让主动建议更精准,纯后端。
5. **持久化运行时设置**(可选):权限分级 / LLM 设置目前内存存储,重启丢失;如需持久化可落库或 Redis。

## 五、结论

CLAUDE.md 定义的 MVP 范围(桌面壳 + Apple Calendar + SoulLedger + 权限分级)在代码层面**已全部落地**,
后端经单元/集成/回归测试与一次端到端冒烟。唯一尚未"亲眼验证"的是 EventKit 在真机日历 App 的写入效果;
其余均为可选增强,不属于 MVP 必需项。
