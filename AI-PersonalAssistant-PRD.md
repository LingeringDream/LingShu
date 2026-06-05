# 灵枢 (LingShu) — macOS 桌面 AI 个人助理

## 项目代号：LingShu
## 版本：v0.4 (macOS 桌面助理与 SoulLedger 修订)
## 日期：2026-06-04
## 作者：PM Office

> **说明**：本文档版本号独立于代码版本（`Cargo.toml` 当前为 `0.1.0`，跟随 Phase 0 脚手架）。PRD 版本号按产品需求迭代次数递增，代码版本号在 Phase 1 首个可运行桌面壳交付时同步至 `0.4.0`。

---

## 目录

- **一、项目概述** — macOS 桌面宠物式个人助理、MVP 范围、核心差异化
- **二、竞品调研与启示** — 桌面 AI、AI 伴侣、桌面代理、记忆系统分析
- **三、核心功能设计** — 桌面宠物、日历智能、权限分级、SoulLedger
- **四、系统架构** — macOS 桌面壳、后端服务、记忆内核、权限桥接
- **五、技术栈选型** — Tauri/macOS、React、Rust 后端、AI/ML、数据存储
- **六、项目里程碑与开发计划** — macOS MVP 优先，后续扩大生态
- **七、数据流与交互时序** — 对话流程、主动监控流程
- **八、安全与隐私** — macOS 权限、记忆隐私、LLM 安全、合规
- **九、错误处理与降级策略** — 12 种故障场景矩阵
- **十、数据备份与灾难恢复** — 备份策略、RTO/RPO、告警阈值
- **十一、商业模式设想** — 开源+增值服务、成本模型
- **十二、风险与应对** — 7 项风险及应对策略
- **十三、核心 API 规范** — 对话、记忆、人格、日历、权限端点
- **十四、测试策略** — 测试金字塔、LLM Mock
- **十五、数据库迁移策略** — sqlx-cli、向前兼容规范
- **十六、前后端类型契约** — OpenAPI 自动生成、Phase 1 契约门禁规划
- **十七、总结**
- **附录 A：高性能架构优化方案** — 逐层性能优化 + 资源预算

---

## 一、项目概述

### 1.1 项目愿景

打造一款运行在 macOS 桌面的 AI 个人助理——**灵枢**。它以可自由悬浮的桌面宠物形态常驻屏幕，具备长期记忆、可控人格、主动建议和分级系统操控能力。第一版 MVP 聚焦 Apple Calendar 日程创建、桌面交互入口、SoulLedger 记忆系统和用户可编辑的记忆/人格中心；后续再扩展到 Shortcuts、Accessibility、第三方日历和更多软件生态。

灵枢不是一个普通网页聊天框，也不是只会模拟点击的桌面代理。它的核心目标是成为一个长期陪伴用户工作的 macOS 个人助理：能记住重要事实，忘掉无价值噪音，保持稳定人格，同时允许用户审计、编辑、回滚它的记忆和性格变化。

### 1.2 命名由来

**灵枢**取自中国古代医学经典《灵枢经》，"灵"意为灵性、智慧，"枢"意为枢纽、核心。寓意这款助理是用户工作与生活节奏中的智慧中枢，连接日程、记忆、想法和本机操作。

### 1.3 核心差异化

与市面上现有的 AI 助理不同，灵枢的新定位如下：

| 维度 | 通用桌面 AI (ChatGPT/Gemini) | AI 伴侣 (Replika/Kindroid/Nomi) | 桌面代理 (Fazm/Jarvis/Noet) | **灵枢 LingShu** |
|------|------------------------------|----------------------------------|-----------------------------|--------------------|
| 定位 | 桌面聊天入口 | 情感陪伴 | 屏幕理解与任务执行 | **有记忆和人格的 macOS 桌面个人助理** |
| 形态 | 菜单栏/快捷面板 | App 内角色 | 浮动条/控制面板 | **可悬浮、可拖拽、可贴边的桌面宠物** |
| 执行 | 通常需用户手动 | 基本不执行外部任务 | 强调自动操控 | **分级权限 + 人在回路确认** |
| 记忆 | 平台级 memory | 关系记忆强 | 工作上下文记忆 | **SoulLedger：分层记忆、遗忘、审计、编辑、回滚** |
| 人格 | 弱人格 | 强人格但偏陪伴 | 弱人格 | **稳定 Identity Core + 可控轻微演化** |
| MVP 场景 | 通用问答 | 陪伴聊天 | 跨 App 自动化 | **Apple Calendar 日程 + 个人记忆连续性** |

### 1.4 MVP 范围

MVP 只做能证明产品差异化的最小闭环：

- macOS 桌面宠物窗口：透明、置顶、可拖拽、可缩放、可贴边，点击展开聊天与控制面板。
- Apple Calendar 接入：解析自然语言日程，创建前展示确认卡片，写入系统日历。
- 权限分级：L0 无权限聊天，L1 日历写入，L2 App/URL/Shortcuts，L3 Accessibility 辅助操控，L4 高风险自主操控仅作为远期规划。
- SoulLedger 记忆系统：分层保存、向量化检索、关键词触发、遗忘机制、人格轻微演化。
- Memory & Personality Center：用户可查看、搜索、编辑、删除、锁定记忆；可开关人格自动适应并回滚人格快照。

---

## 二、竞品调研与启示

> v0.4 修订：原 v0.3 竞品偏向代码执行、第二大脑和模型调度。新定位应优先参考四类产品：桌面 AI 助理、AI 伴侣、桌面代理、桌面宠物。Open Interpreter / Khoj / JARVIS 仍保留为技术参考，但不再代表产品定位。

### 2.0 v0.4 竞品格局

| 类型 | 代表产品 | 强项 | 缺口 | 对灵枢的启示 |
|------|----------|------|------|--------------|
| 通用桌面 AI | ChatGPT macOS、Gemini macOS、Claude Desktop、Copilot | 快捷键、窗口上下文、平台 memory | 人格弱，桌面存在感弱，记忆编辑粒度有限 | 不能只做浮动聊天框，必须有桌面实体和可审计记忆 |
| AI 伴侣 | Replika、Nomi、Kindroid | 关系感、人格、长期陪伴 | 不擅长日程、软件操控和生产力任务 | 借鉴人格连续性，但把能力落到 macOS 执行 |
| 桌面代理 | Fazm、Jarvis、Noet、LacPointer、MemoryAgent | 屏幕感知、连接器、自动操控 | 多数缺少稳定人格和透明记忆治理 | 分级权限和执行审计是产品信任基础 |
| 桌面宠物 | Clawster、Hiora、DeskMochi、Sato 等 | 桌面存在感强，交互轻量 | 记忆、日程和系统操控深度不足 | 桌面宠物是入口，不是全部价值 |

**结论**：灵枢的差异化不是单点能力，而是组合能力：桌面存在感 + macOS 执行 + SoulLedger 记忆连续性 + 用户可控人格演化。

### 2.1 Open Interpreter
- **GitHub**: [OpenInterpreter/open-interpreter](https://github.com/OpenInterpreter/open-interpreter)
- **Stars**: 57k+
- **核心能力**: 让 LLM 在本地执行 Python/JS/Shell 代码，终端 ChatGPT 式交互
- **技术亮点**: 支持多 LLM 后端 (GPT-4, Claude, 本地模型)，FastAPI 服务器模式，Profile 配置系统
- **对灵枢的启示**:
  - 本地执行能力可借鉴——灵枢需要能在用户确认后操作本机文件、App 和自动化脚本
  - Profile 配置机制——不同项目/场景使用不同配置
  - 安全确认机制——执行敏感操作前需用户确认

### 2.2 Khoj — Your AI Second Brain
- **GitHub**: [khoj-ai/khoj](https://github.com/khoj-ai/khoj)
- **Stars**: 25k+
- **核心能力**: 个人 AI 应用，可连接本地/云端 LLM，支持多格式文档搜索与问答
- **技术亮点**:
  - 多平台接入 (Browser, Obsidian, Emacs, Desktop, WhatsApp)
  - 语义搜索引擎，支持 PDF/Markdown/Notion/Word
  - 自定义 Agent 系统 (知识库 + 人设 + 工具)
  - 自动化研究 + 个人 Newsletter
- **对灵枢的启示**:
  - Agent 自定义系统——灵枢可支持用户创建不同角色的子 Agent
  - 多源文档检索——个人资料、日程、文档、会议记录的统一检索
  - 主动推送机制——定时提醒、上下文建议、长期计划跟进

### 2.3 JARVIS (HuggingGPT)
- **GitHub**: [microsoft/JARVIS](https://github.com/microsoft/JARVIS)
- **Stars**: 24k+
- **核心能力**: LLM 作为控制器，调度 HuggingFace 上的专家模型完成复杂任务
- **技术亮点**:
  - 四阶段流水线：任务规划 → 模型选择 → 任务执行 → 响应生成
  - 支持多模态任务 (文本、图像、视频、音频)
  - 本地 + 云端混合推理模式
- **对灵枢的启示**:
  - 任务分解与调度架构——灵枢处理复杂 PM 任务时需要类似的规划能力
  - 多模型协作——不同子任务可调用不同 AI 模型

### 2.4 其他参考项目

| 项目 | 特点 | 可借鉴点 |
|------|------|---------|
| **Live2D + ChatGPT** | 2.5D 虚拟形象驱动 | 虚拟形象的口型同步、表情驱动 |
| **SadTalker** | 音频驱动的说话头像生成 | 面部动画生成技术 |
| **Ready Player Me** | 3D 虚拟形象创建 SDK | 快速创建个性化虚拟形象 |
| **Convai** | 3D AI 角色对话 | 虚拟角色的自然对话能力 |
| **Three.js** | WebGL 3D 渲染库 | 前端 3D 渲染技术栈 |

---

## 三、核心功能设计

### 3.1 macOS 桌面宠物系统 — "灵体"

灵体是灵枢在桌面上的常驻入口。MVP 不追求复杂 VRM 虚拟人，而先实现可用、稳定、低资源占用的桌面宠物壳：它能自由悬浮在桌面，用户点击后展开对话、日程确认、记忆编辑和权限设置。

#### 3.1.1 MVP 形态
- **透明悬浮窗**：始终置顶、背景透明、可拖拽、可缩放、可贴边隐藏。
- **轻量形象**：第一版使用 2D/简化 3D 精灵，保留 Three.js 扩展空间；VRM/Live2D 放到后续。
- **状态反馈**：通过姿态、表情、气泡和边缘提示表现待确认日程、主动建议、权限请求、记忆更新。
- **交互入口**：单击展开 mini chat，右键打开 Memory & Personality Center，拖拽改变位置，长按进入设置。

#### 3.1.2 桌面行为
- **空闲行为**：低频 idle 动画，避免干扰。
- **提醒行为**：日程临近、Thought Queue 有高置信建议时显示轻提示。
- **确认行为**：创建日程、运行 Shortcut、辅助操控前展示确认卡片。
- **降噪行为**：用户专注模式或全屏 App 下自动降低透明度/隐藏。

#### 3.1.3 技术实现路径
```
桌面壳: Tauri 2 + macOS window APIs
前端渲染: React 18 + TypeScript + Vite + Three.js
窗口能力: transparent + always_on_top + frameless + drag region
日历桥接: Swift sidecar 直连 macOS EventKit（Phase 1 搭建 Tauri 壳后由 Tauri command 调用 sidecar）
权限桥接: macOS Calendar entitlement、Shortcuts、Accessibility（通过 Swift sidecar + macOS API，后续 Tauri 壳接入后形成统一桥接层）
形象升级: MVP 2D/简化 3D → v2 VRM/Live2D
```

---

### 3.2 智能引擎 — "灵核"

灵枢的大脑，基于多层 AI 架构构建的个人助理系统。MVP 的智能核心不是项目管理套件，而是围绕用户日常工作节奏：理解对话、调用记忆、创建日程、形成主动建议，并在用户确认后执行 macOS 操作。

> **版本范围说明**：以下功能标注 `[v1.0]` 为首发版本包含，`[v2.0]` 为后续迭代。

#### 3.2.1 对话智能 [v1.0]
- **多轮对话管理**: 维护上下文，支持话题切换与回溯
- **意图识别**: 区分信息查询、任务指派、决策辅助、情感支持等不同意图
- **多模态输入**:
  - 文字：自然语言交互 [v1.0]
  - 语音：实时语音对话 (STT + TTS) [v2.0]
  - 文件：拖拽文档直接分析 [v2.0]
  - 截图/图片：OCR + 视觉理解 [v2.0]

#### 3.2.2 Apple Calendar 日程智能 [MVP]

**这是 MVP 的终极可执行闭环**（落在 Phase 3，第 15–19 周）：用户用自然语言说出安排，灵枢解析为结构化日程，展示确认卡片，经用户确认后写入 macOS 自带 Calendar。

核心能力：
- **自然语言解析**：从“明天下午三点和张三开 30 分钟需求会”提取标题、日期、开始/结束时间、参与人、地点、备注。
- **日历写入**：通过 EventKit 或 Swift sidecar 请求 Calendar 权限并创建事件。
- **确认优先**：所有日程写入默认需要用户确认；未来可对低风险重复规则开启自动执行。
- **冲突提示**：MVP 先做同一时间段冲突提醒；读取全量日历需要用户单独授权。
- **记忆联动**：从用户偏好中自动补全默认会议时长、常用日历、常用地点、工作时间边界。

#### 3.2.3 Thought Queue 主动建议 [MVP 基础 / v2 扩展]

灵枢可以形成自己的“待确认想法”，但不伪装成真正意识，也不绕过用户执行。Thought Queue 是一组可解释、可忽略、可确认的建议。

| 触发条件 | 主动建议 |
|---------|---------|
| 用户多次提到某任务但未排期 | 建议创建 Calendar 时间块 |
| 会议前 15 分钟 | 提醒会议并展示相关记忆 |
| 用户长期偏好被触发 | 自动采用偏好，回复中说明依据 |
| 记忆候选高价值 | 建议“是否记住这条偏好/决策” |
| 日程与工作时间冲突 | 建议调整时间或拆分任务 |
| 长期未完成的计划再次出现 | 提醒历史上下文和下一步 |

Thought Queue 中的每条建议都必须包含 `reason`、`confidence`、`source_memory_ids` 和 `requires_confirmation`，方便用户审计。

---

### 3.3 macOS 权限分级操控 — "灵域"

灵域不再是大型项目管理工作台，而是灵枢和 macOS 的权限边界。所有执行能力按风险分级，用户可以逐级开启，并随时关闭。

| 等级 | 能力 | 权限 | 默认策略 |
|------|------|------|----------|
| L0 | 聊天、桌面宠物、记忆中心、建议展示 | 无系统权限 | 默认开启 |
| L1 | 创建/修改 Apple Calendar 日程 | Calendar write-only / full access | 每次执行前确认 |
| L2 | 打开 App、文件、URL，运行用户预设 Shortcuts/AppleScript | Automation / Shortcuts | 白名单 + 确认 |
| L3 | 代按快捷键、输入文本、读取可访问性树 | Accessibility | 用户显式授权 + 实时可停止 |
| L4 | 屏幕识别 + 自主点击 + 多步任务 | Screen Recording + Accessibility | 远期能力，默认关闭 |

#### 3.3.1 MVP 集成清单

| 类别 | 平台/能力 | 集成方式 | 优先级 |
|------|-----------|---------|--------|
| 日历 | Apple Calendar | EventKit / Swift sidecar | P0 |
| 桌面壳 | macOS floating window | Tauri window APIs | P0 |
| 记忆 | SoulLedger | PostgreSQL + Qdrant + Redis | P0 |
| 快捷操作 | 打开 URL/App | macOS open command / Tauri shell allowlist | P1 |
| 自动化 | Shortcuts | `shortcuts run` / App Intents | P2 |
| 辅助操控 | Accessibility | AX APIs | P3 |

#### 3.3.2 扩展生态

v1.0 之后再按用户需求扩展 Google Calendar、Outlook、Reminders、Notion、Slack、飞书、钉钉、企业微信、GitHub、Linear 等。扩展原则是：先做用户明确使用的生态，不在 MVP 阶段堆连接器。

---

### 3.4 SoulLedger 灵魂账本系统 — "灵性"

SoulLedger 是灵枢的核心差异化：它管理助理记住什么、忘记什么、性格如何轻微变化、变化为什么发生，以及用户如何审计和编辑这些内容。

#### 3.4.1 记忆分层

| 层级 | 名称 | 用途 | 存储 |
|------|------|------|------|
| L0 | Working Memory | 当前会话短期状态，自动过期 | Redis / 内存 |
| L1 | Episodic Memory | 原始对话、操作、日程事件，保留事实来源 | PostgreSQL |
| L2 | Semantic Memory | 压缩后的事实、偏好、项目状态 | PostgreSQL + Qdrant |
| L3 | Procedural Memory | 用户常用流程、软件操作步骤 | PostgreSQL + Qdrant |
| L4 | User Profile | 用户习惯、长期目标、工作方式 | PostgreSQL |
| L5 | Personality State | 当前人格参数和演化历史 | PostgreSQL |
| L6 | Identity Core | 核心人格、边界和价值约束，默认锁定 | PostgreSQL |
| L7 | Thought Queue | 主动建议、风险判断、待确认想法 | PostgreSQL / Redis |

> **Phase 0 实现状态**：当前代码中 SoulLedger 只有一张扁平 `memories` 表（`memory_type VARCHAR(30)` + `content TEXT`），用于 MVP 前的基础 CRUD 占位。上述 8 层架构是 **目标设计**，将在 Phase 2 通过独立迁移脚本逐步落地：`personality_snapshots`、`thought_queue`、`user_profiles`、`identity_core` 等专用表及其对应的向量/缓存存储届时一并建立。

#### 3.4.2 检索算法

每次用户输入后，系统先强制加载 Identity Core、当前 Personality Snapshot、用户开关设置和活跃项目/主题记忆，再做候选召回。

```
retrieval_score =
  0.30 * semantic_similarity
+ 0.20 * entity_match
+ 0.15 * keyword_match
+ 0.15 * task_relevance
+ 0.10 * importance
+ 0.05 * recency
+ 0.05 * user_confirmed
+ layer_boost
- contradiction_penalty
- stale_penalty
```

> **实现说明**：上述权重为设计目标。实际 Phase 2 实现中，`semantic_similarity` 依赖 Qdrant 向量检索，`entity_match` 依赖 NLP 实体提取，`task_relevance`、`importance`、`contradiction_penalty` 等主观维度需通过 LLM-as-judge 打分或启发式规则近似，而非精确数学计算。

召回来源包括关键词、向量语义、最近上下文、项目/任务图谱、操作流程记忆。低置信召回必须拒绝，避免助理把无关记忆硬塞进上下文。

#### 3.4.3 写入与遗忘算法

普通对话不直接写入长期记忆，而是生成 `memory_candidate` 并计算保留分：

```
retention_score =
  0.30 * user_explicit_signal
+ 0.25 * future_usefulness
+ 0.20 * preference_or_identity_signal
+ 0.15 * project_relevance
+ 0.10 * repetition_count
- triviality_penalty
- privacy_sensitivity_penalty
```

> **实现说明**：`future_usefulness`、`triviality_penalty`、`privacy_sensitivity_penalty` 不可直接计算。Phase 2 将通过 LLM 对 `memory_candidate` 做一次性评估（评分 0–1 并附简短理由），结果与用户显式信号加权后得到 `retention_score`。LLM 调用成本需通过频率限制和缓存控制。

规则：
- `< 0.25`：不保存，只在 Working Memory 中短期存在。
- `0.25 - 0.55`：保存为短期 episode，到期压缩或软遗忘。
- `0.55 - 0.80`：晋升为 semantic memory。
- `> 0.80`：长期保存，并保留原始来源。
- 用户说“记住”时进入确认/锁定流程；用户说“别记”时写入 deny rule。

遗忘分三类：软遗忘（不主动召回）、压缩遗忘（多条低价值 episode 合并为摘要）、硬遗忘（用户删除或 TTL 到期彻底移除）。

#### 3.4.4 人格演化机制

人格由可解释参数组成：`directness`、`warmth`、`proactivity`、`risk_tolerance`、`verbosity`、`formality`、`humor`。Identity Core 默认锁定，轻微自动适应只影响 Personality State。

```
trait_delta =
  0.40 * repeated_user_feedback
+ 0.25 * observed_preference
+ 0.20 * successful_interaction_pattern
+ 0.15 * explicit_style_request
```

> **实现说明**：`repeated_user_feedback` 可通过对话中用户对回复风格的显式评价（"太啰嗦了""直接一点"）提取信号；`observed_preference` 则通过用户对不同类型的回复的操作反馈（复制/忽略/追问/点赞）间接推断。这些参数在不同 LLM 模型间不具有可移植性——同一组 `trait` 值在 GPT-4、Claude、Qwen 上的表现可能差异显著。模型切换后建议冻结自动适应 1–2 周，让用户先确认基线感受。

限制：
- 用户可关闭“允许性格自动适应”。
- 单次变化不超过 `0.03`，每日累计不超过 `0.10`。
- 明显人格变化必须用户确认。
- 每次变化写入 `PersonalityChangeEvent`，可查看原因、影响参数和来源记忆。
- 支持回滚到任意 Personality Snapshot。

#### 3.4.5 Memory & Personality Center

用户必须能直接编辑灵枢的“灵魂账本”：
- 查看本次回复使用了哪些记忆。
- 搜索、编辑、删除、锁定、禁用记忆。
- 标记“永久记住”“这个可以忘记”“以后不要记这种内容”。
- 查看人格参数、变化历史和触发原因。
- 开关自动记忆、自动遗忘、性格自动适应。
- 回滚人格快照，或重置为 Identity Core。

---

## 四、系统架构

### 4.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        客户端层 (Client Layer)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ macOS Pet    │  │  Menu Bar    │  │ Memory & Personality │  │
│  │ Tauri+React  │  │  Controller  │  │ Center               │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
│         │                 │                      │              │
│         └─────────────────┼──────────────────────┘              │
│                           │                                     │
│                    ┌──────┴───────┐                              │
│                    │  桌面灵体层   │                              │
│                    │  Three.js    │                              │
│                    │  2D/3D Pet   │                              │
│                    └──────┬───────┘                              │
└───────────────────────────┼─────────────────────────────────────┘
                            │ WebSocket + REST API
┌───────────────────────────┼─────────────────────────────────────┐
│                     服务端层 (Server Layer)                       │
│                           │                                     │
│  ┌────────────────────────┼────────────────────────────────┐    │
│  │              API Gateway (Axum / Traefik)                │    │
│  └────────────────────────┼────────────────────────────────┘    │
│                           │                                     │
│  ┌──────────┐  ┌──────────┴──┐  ┌──────────┐  ┌──────────┐    │
│  │ 对话引擎  │  │ Calendar   │  │ 权限桥接  │  │ 通知引擎  │    │
│  │ Dialog   │  │ Engine     │  │ Permission│  │ Notify   │    │
│  │ Engine   │  │             │  │ Engine   │  │ Engine   │    │
│  └──────────┘  └─────────────┘  └──────────┘  └──────────┘    │
│                                                                │
│  ┌──────────┐  ┌─────────────┐  ┌──────────┐  ┌──────────┐    │
│  │SoulLedger│  │ Thought     │  │人格引擎   │  │ 调度引擎  │    │
│  │ Memory   │  │ Queue       │  │Personality│  │ Scheduler│    │
│  │ Engine   │  │  Graph      │  │ Engine   │  │          │    │
│  └──────────┘  └─────────────┘  └──────────┘  └──────────┘    │
└─────────────────────────────────────────────────────────────────┘
                            │
┌───────────────────────────┼─────────────────────────────────────┐
│                      AI 引擎层 (AI Layer)                        │
│                           │                                     │
│  ┌────────────────────────┼────────────────────────────────┐    │
│  │              LLM Router (自研 Rust 路由器)                 │    │
│  │     根据任务类型和成本自动选择最优模型                        │    │
│  └──┬──────────┬──────────┼──────────┬─────────────────────┘    │
│     │          │          │          │                          │
│  ┌──┴──┐   ┌──┴──┐   ┌──┴──┐   ┌──┴──┐                       │
│  │GPT-4│   │Claude│   │Qwen │   │Local│  (可扩展)              │
│  │     │   │     │   │     │   │LLM  │                        │
│  └─────┘   └─────┘   └─────┘   └─────┘                        │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  专用模型层                                               │   │
│  │  ├── STT (Whisper / Paraformer)                         │   │
│  │  ├── TTS (Edge TTS / VITS)                              │   │
│  │  ├── 情感分析 (Fine-tuned BERT)                          │   │
│  │  ├── 文档理解 (LayoutLM / PaddleOCR)                     │   │
│  │  └── 图表生成 (独立 Python 服务 / ECharts 前端渲染)       │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                            │
┌───────────────────────────┼─────────────────────────────────────┐
│                      数据层 (Data Layer)                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │
│  │PostgreSQL│  │ Redis    │  │ Qdrant   │  │ 文件存储(v2) │   │
│  │ 主数据库  │  │ 缓存/队列 │  │ 向量数据库│  │ 文件存储     │   │
│  │ 主数据库  │  │          │  │          │  │              │   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 核心模块详解

#### 4.2.1 对话引擎 (Dialog Engine)
```
职责: 管理所有用户交互的对话流程
技术栈: Rust (Axum + 自研 NLU 模块) + 外部 LLM API
核心能力:
  - 多轮对话上下文管理 (滑动窗口 + 摘要压缩)
  - 意图分类器 (聊天/记忆编辑/创建日程/权限请求/软件操控)
  - 槽位填充 (从自然语言中提取结构化信息)
  - 多模态输入处理 (文字/语音/文件/图片)
  - 流式响应 (SSE/WebSocket, 零拷贝转发 LLM 流式输出)
性能特征:
  - 对话管理本身内存 < 5MB/会话
  - LLM 响应流式转发: 无中间缓冲，直接 pipe
  - 意图分类: 本地规则引擎 + 轻量分类器 (<1ms)
```

#### 4.2.2 Calendar 智能引擎 (Calendar Intelligence Engine)
```
职责: 解析、确认并写入用户日程
技术栈: Rust 规则引擎 + 外部 LLM + macOS EventKit/Swift sidecar
核心能力:
  - 自然语言日程解析 (标题/时间/时区/地点/参与人/备注)
  - 用户偏好补全 (默认日历、默认会议时长、工作时间)
  - 冲突检测和确认卡片生成
  - EventKit 写入 Apple Calendar
  - 日程创建结果回写 SoulLedger
性能特征:
  - 规则解析: <1ms
  - LLM 解析: 流式异步，不阻塞 UI
  - Calendar 写入: 本地调用，失败需返回可解释权限/冲突原因
```

#### 4.2.3 权限桥接引擎 (Permission Bridge Engine)
```
职责: 管理 macOS 系统权限、执行白名单和用户确认
技术栈: Swift sidecar + macOS APIs（规划通过 Tauri commands 接入桌面壳，当前 Phase 0 仅有后端 API 层）
权限层级:
  L0: 无权限聊天
  L1: Calendar
  L2: open/Shortcuts/AppleScript
  L3: Accessibility
  L4: Screen Recording + Accessibility (远期)
核心能力:
  - 权限状态检测和引导授权
  - 操作风险分级和确认策略
  - 执行审计日志
  - 用户可配置白名单和禁用规则
性能特征:
  - 权限检查本地完成
  - 高风险动作必须在 UI 中可见并可停止
```

#### 4.2.4 SoulLedger 记忆引擎 (SoulLedger Memory Engine)
```
职责: 管理灵枢的长期记忆、遗忘机制、人格状态和主动建议
技术栈: PostgreSQL + Qdrant + Redis
记忆类型:
  - Working Memory → Redis / 内存
  - Episodic Memory → PostgreSQL
  - Semantic / Procedural Memory → PostgreSQL + Qdrant
  - User Profile / Personality State / Identity Core → PostgreSQL
  - Thought Queue → PostgreSQL / Redis
核心能力:
  - 混合召回 (向量 + 关键词 + 实体 + 最近上下文)
  - 记忆晋升、压缩、软遗忘、硬遗忘
  - 人格轻微自动演化和快照回滚
  - 本次回复引用记忆审计
性能目标:
  - 向量检索: MVP 先以 100K 向量 P95 < 20ms 为优化目标；<5ms/百万级向量属于后续目标
  - PoC 状态 (2026-06-04): 100K 向量无过滤 P95 14.49ms ✅ 已达 MVP 目标 (< 20ms)；filtered P95 29.82ms ⚠️ 超出 20ms，需在 Phase 2 中优化过滤路径或放宽过滤检索时效目标。原 <5ms / filtered <10ms 的严格阈值推迟到百万级向量阶段重评。
  - 低置信召回拒绝，避免无关记忆污染上下文
```

---

## 五、技术栈选型

### 5.1 macOS 桌面端与前端

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| 桌面壳 | **Tauri 2 + macOS window APIs** | 复用现有 React/Vite，资源占用低于 Electron，适合透明悬浮窗 |
| 框架 | React 18 + TypeScript | 当前仓库已采用，生态成熟，类型安全 |
| 形象渲染 | MVP：2D/简化 3D；v2：Three.js + React Three Fiber | MVP 先保证桌面体验稳定，再升级 VRM/Live2D |
| 虚拟形象 | MVP：轻量 sprite/glTF；v2：VRM/Live2D | 避免第一版被复杂形象制作拖慢 |
| 状态管理 | Zustand | 轻量，适合实时应用 |
| UI 组件 | 当前：自定义 CSS；规划：Radix UI | 适合 Memory & Personality Center 的可访问性与复杂控件 |
| 实时通信 | 当前：Fetch + SSE；规划：原生 WebSocket | 与 Axum 原生能力匹配，避免引入 Socket.IO 协议栈 |
| 语音 | v2.0：Web Speech API + Whisper.js / 独立 STT 服务 | 浏览器原生 + 离线能力，Phase 2 以后接入 |
| 构建工具 | Vite | 快速 HMR，原生 ESM |
| macOS 日历 | EventKit / Swift sidecar | Tauri Rust 侧不直接提供完整 EventKit，优先用 Swift 桥接 |
| macOS 自动化 | Shortcuts / AppleScript / Accessibility APIs | 支持分级权限和用户确认 |

### 5.2 后端 (Rust)

> **核心决策：后端全面采用 Rust**。相比 Python，Rust 在内存占用上降低 5-10 倍，CPU 利用率提升 3-5 倍，且无 GC 停顿。AI/ML 推理通过 HTTP 调用外部服务或本地独立进程，后端本身保持轻量。

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| Web 框架 | **Axum 0.7** | Tokio 生态，零开销抽象，类型安全的路由和中间件 |
| 异步运行时 | **Tokio 1.x** | Rust 异步标准，百万级并发，极低内存开销 |
| 数据库 ORM | **sqlx** | 编译期 SQL 检查，async 原生，零运行时反射 |
| HTTP 客户端 | **reqwest** | 调用 LLM API、第三方服务，支持连接池和流式响应 |
| WebSocket | **Axum ws + tokio-tungstenite** | 高性能 WebSocket，当前已有处理器骨架，路由接入待 Phase 1 完成 |
| 任务队列 | **自研轻量队列** (Redis Streams + Tokio workers) | 替代 Celery，内存占用从 ~200MB 降到 ~5MB |
| 定时任务 | 规划：**tokio-cron-scheduler** 或 Tokio interval workers | 轻量级，无额外进程，待主动智能阶段引入 |
| 序列化 | **serde + serde_json** | 零拷贝序列化，编译期代码生成 |
| 日志 | **tracing** | 结构化日志，async-aware，性能远超 Python logging |
| 错误处理 | **anyhow + thiserror** | 生产级错误处理 |
| API 文档 | **utoipa** (OpenAPI 自动生成) | 编译期生成，零运行时开销 |
| 配置管理 | **figment** | 多源配置 (文件/环境变量/CLI) |
| 指标采集 | **prometheus-client** | 原生 Rust Prometheus 指标 |

**为什么不用 Python FastAPI？**

| 指标 | Python FastAPI | Rust Axum | 提升 |
|------|---------------|-----------|------|
| 空闲内存 | ~150MB | ~15MB | **10x** |
| 并发连接 (1K) | ~300MB | ~30MB | **10x** |
| JSON 序列化 (10KB) | ~50μs | ~3μs | **16x** |
| 数据库查询开销 | ORM 反射 + 连接池 | 编译期检查 + 零拷贝 | **3-5x** |
| 冷启动时间 | ~2s | ~50ms | **40x** |
| GC 停顿 | 有 (gc.collect) | 无 | — |

**Rust AI 集成架构：**

后端本身不运行 LLM，而是通过 HTTP 调用外部 AI 服务，保持自身极度轻量：

```
┌─────────────────────────────────────────────────┐
│              Rust 后端 (Axum)                    │
│              内存: ~30MB, CPU: <5%               │
│                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │ 对话引擎  │  │日历引擎   │  │权限桥接  │      │
│  │ (Rust)   │  │ (Rust)   │  │ (Rust)   │      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
│       │              │              │            │
│       └──────────────┼──────────────┘            │
│                      │ HTTP (reqwest)            │
└──────────────────────┼───────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
   ┌────┴────┐   ┌────┴────┐   ┌────┴────┐
   │LLM 服务  │   │STT 服务  │   │TTS 服务  │
   │(外部API/ │   │(Whisper  │   │(Edge TTS│
   │ Ollama)  │   │ server)  │   │ server) │
   └─────────┘   └─────────┘   └─────────┘
```

### 5.3 AI/ML (独立服务层)

> AI 模型作为独立微服务运行，与 Rust 后端通过 HTTP/gRPC 通信。这样后端崩溃不影响模型服务，模型负载不影响后端响应。

| 组件 | 技术选型 | 部署方式 | 理由 |
|------|---------|---------|------|
| 主 LLM | OpenAI / Claude / Qwen 等云端主模型 | 云端 API | 按需调用，零本地资源；具体型号按成本、延迟和能力实时配置 |
| 本地 LLM | Qwen2.5-7B / Llama 3.1-8B | **Ollama** 独立进程 | CPU 推理，与后端进程隔离 |
| STT | v2.0：Whisper Large-v3 | **faster-whisper** 独立服务 | GPU 加速，HTTP API |
| TTS | v2.0：Edge TTS / CosyVoice | 独立微服务 | 流式合成 |
| 向量模型 | v1.0：外部/本地 embedding；v2.0：BGE-M3 | **TEI** (Text Embeddings Inference) | HuggingFace 官方 Rust 推理服务 |
| OCR | v2.0：Surya / PaddleOCR | 独立微服务 | 按需启动 |
| 情感分析 | v2.0：Fine-tuned RoBERTa | TEI 共享服务 | 多模型共用推理服务 |
| LLM 路由 | **自研 Rust 路由器** | 内嵌后端 | 基于意图和复杂度自动选择模型 |

### 5.4 数据存储

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| 主数据库 | PostgreSQL 16 | 可靠，JSON 支持好 |
| 缓存/队列 | Redis 7 | 高速缓存 + Redis Streams 任务队列 |
| 向量数据库 | **Qdrant** | Rust 实现，性能最优，与后端技术栈一致 |
| 图数据库 | 非 MVP；候选 Apache AGE 或 Neo4j | 2026-06-04 AGE PoC 失败；MVP 不 provision 图数据库，后续按需求重评 |
| 文件存储 | v2.0：MinIO (兼容 S3) | 本地化对象存储，当前 Phase 0 尚未接入 |
| 全文搜索 | v2.0：Meilisearch 或 PostgreSQL FTS | 当前记忆搜索先用 PostgreSQL `ILIKE`/全文索引兜底 |

> **图数据库状态**：Apache AGE 只是后续图谱能力候选。2026-06-04 实测标准 `postgres:16-bookworm` 镜像不包含 AGE extension，PoC 未通过；MVP 不依赖图数据库，SoulLedger 先使用 PostgreSQL + Qdrant。

### 5.5 部署

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| 容器化 | Docker + Docker Compose | 一键部署 |
| 后端镜像 | **scratch / distroless** | Rust 静态编译，镜像 < 20MB |
| 编排 | Kubernetes (可选) | 生产级容器编排 |
| CI/CD | GitHub Actions | 自动化构建部署 |
| 监控 | Prometheus + Grafana | 指标监控 + 可视化 |
| 日志 | Loki + Grafana | 轻量级日志聚合 |

---

## 六、项目里程碑与开发计划

> v0.4 修订：路线图改为 **macOS MVP 优先**。第一阶段不做 Jira/飞书/大型项目管理工作台，先证明桌面宠物入口、Apple Calendar 执行、SoulLedger 记忆连续性和用户可控人格。

### Phase 0: 基础设施 + 技术验证 (第 1-3 周)
- [x] 项目脚手架搭建 (Rust Axum + React + Three.js)
- [~] Docker 开发环境配置 (docker-compose 一键启动；本地需安装 Docker 后验证)
- [x] 基础 CI/CD 流水线搭建 (GitHub Actions: Rust lint/test、frontend type-check/build、docker build；frontend ESLint 与严格 OpenAPI contract 仍待 Phase 1 补齐)
- [x] 数据库 schema 设计 (PostgreSQL + sqlx 迁移脚本)
- [~] API 规范定义 (utoipa 已接入，OpenAPI 当前只登记部分已实现端点)
- [~] **技术验证 PoC**：
  - [!] Apache AGE 图查询 PoC（2026-06-04 实测失败：标准 Postgres 镜像无 AGE extension）
  - [!] Rust LLM 调用 PoC（代码已建立；当前环境缺少 Ollama，实测 blocked）
  - [~] Qdrant 向量检索 PoC（2026-06-04：无过滤 P95 14.49ms ✅ 达 MVP <20ms 目标；filtered P95 29.82ms ⚠️ 待 Phase 2 优化）
  - [ ] 如 PoC 不通过，及时调整技术选型

> 状态标记：`[x]` 已落地到仓库；`[~]` 有实现或材料但缺少完整接入；`[!]` 已验证但未通过或被环境阻塞；`[ ]` 未开始。

### Phase 1: macOS MVP 壳与对话 (第 4-8 周)
- [ ] Tauri 桌面壳接入现有 React/Vite 前端
- [ ] 透明置顶悬浮窗、拖拽、缩放、贴边、菜单栏入口
- [ ] 桌面宠物 MVP 形象与状态气泡
- [ ] LLM 接入层 (reqwest 调用 GPT-4/Claude/Ollama，流式 SSE 转发)
- [ ] 对话界面改造为 mini chat + 展开面板
- [ ] 前后端类型契约 (utoipa → openapi-typescript-codegen 自动生成)

### Phase 2: SoulLedger MVP (第 9-14 周)
- [ ] 记忆数据模型：episode、semantic memory、user profile、identity core、personality snapshot
- [ ] 混合检索：关键词 + 向量 + 最近上下文 + 层级 boost
- [ ] 记忆候选写入、保留评分、软遗忘、压缩遗忘、硬删除
- [ ] Memory & Personality Center：查看、搜索、编辑、删除、锁定、禁记
- [ ] 人格参数、自动适应开关、PersonalityChangeEvent、快照回滚
- [ ] 本次回复使用记忆的审计展示

### Phase 3: Apple Calendar 与权限分级 (第 15-19 周)
- [ ] EventKit / Swift sidecar 接入 Apple Calendar
- [ ] Calendar 权限状态检测、授权引导、写入失败解释
- [ ] 自然语言日程解析与确认卡片
- [ ] 创建日程后回写 SoulLedger
- [ ] 权限分级设置页：L0/L1/L2/L3/L4
- [ ] 操作审计日志和用户确认策略

### Phase 4: 主动建议与轻量自动化 (第 20-24 周)
- [ ] Thought Queue 数据模型和主动建议展示
- [ ] 日程临近提醒、未排期任务建议、冲突提醒
- [ ] L2 白名单能力：打开 App、URL、文件
- [ ] Shortcuts / AppleScript 预设流程入口
- [ ] 通知引擎：macOS 原生通知 + 桌面宠物提醒

### Phase 5: 打磨与发布 (第 25-32 周)
- [ ] 性能优化 (悬浮窗资源占用、API 响应、记忆检索延迟)
- [ ] 安全审计 (Prompt 注入、权限滥用、隐私数据外发)
- [ ] 集成测试 + E2E 测试
- [ ] 用户测试与反馈迭代
- [ ] 文档编写 (用户手册 + API 文档 + 权限说明)
- [ ] macOS MVP 发布

### v2.0 规划 (MVP 发布后)
- VRM/Live2D 精细形象与语音交互
- Google Calendar / Outlook / Reminders
- Accessibility 辅助操控和可视化执行回放
- 更多第三方生态：Notion、Slack、飞书、钉钉、GitHub、Linear
- 知识图谱自动构建和跨项目长期规划
- 多语言支持 (英文/日文)

---

## 七、数据流与交互时序

### 7.1 用户对话流程

```
用户输入 (文字/语音/文件)
    │
    ▼
┌─────────────┐     ┌─────────────┐
│ 输入预处理   │────▶│ 意图识别     │
│ (STT/OCR/  │     │ + 槽位提取   │
│  文件解析)  │     └──────┬──────┘
└─────────────┘            │
                           ▼
                    ┌──────────────┐
                    │ 记忆检索      │
                    │ (相关上下文)  │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ 灵核路由器     │
                    │ (对话/日历/权限)│
                    └──────┬───────┘
                           │
                    ┌──────┴───────┐
                    │              │
                    ▼              ▼
            ┌──────────┐   ┌──────────┐
            │ 工具调用  │   │ 直接回复  │
            │ (Calendar/│   │          │
            │  权限桥接)│   │          │
            └────┬─────┘   └────┬─────┘
                 │              │
                 └──────┬───────┘
                        │
                        ▼
                 ┌──────────────┐
                 │ 响应生成      │
                 │ (文字+语音+  │
                 │  虚拟形象动画)│
                 └──────┬───────┘
                        │
                        ▼
                 ┌──────────────┐
                 │ SoulLedger更新│
                 │ (候选/遗忘/审计)│
                 └──────────────┘
```

### 7.2 主动监控流程

```
┌──────────────┐
│ 定时触发器    │ (每 N 分钟)
│ 事件触发器    │ (日历/记忆/用户行为)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ 数据采集      │ (日程/会话/记忆候选)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ 建议计算      │ (Thought Queue)
└──────┬───────┘
       │
       ▼
┌──────────────┐     ┌──────────────┐
│ 置信度判断    │────▶│ 触发通知      │ (桌面宠物/macOS通知)
│ + 记忆依据    │     │ + 用户确认    │ (日程/建议/权限请求)
└──────────────┘     └──────────────┘
```

---

## 八、安全与隐私

### 8.1 数据安全
- **当前 Phase 0**：通过环境变量配置数据库、Redis、Qdrant、LLM 和安全密钥；迁移脚本已包含 integration token 加密字段、审计日志表和软删除字段。
- **v1.0 目标**：敏感数据加密存储 (AES-256-GCM)，OAuth Token 和 LLM API Key 不以明文落库。
- **macOS 本地密钥**：桌面端优先使用 Keychain 保存本机密钥、API Key 和授权状态。
- **部署目标**：生产 API 通信使用 TLS 1.3；数据库连接启用 SSL；本地开发环境可使用明文内网连接。
- **LLM 调用目标**：支持 PII 自动检测、替换和审计记录。
- **文件安全目标**：文件上传做病毒扫描、格式校验和大小限制；当前 Phase 0 尚未开放文件上传。
- **记忆隐私目标**：每条长期记忆必须有来源、作用域、置信度和删除方式；用户可关闭自动记忆和人格自动适应。

### 8.2 LLM 安全 (关键新增)
- **Prompt 注入防护**：
  - 用户输入与系统 Prompt 严格分离，用户内容包裹在特定标记中
  - 输出过滤器：检测 LLM 是否泄露了系统 Prompt 或执行了非预期操作
  - 输入长度限制 + 频率限制，防止资源耗尽攻击
- **API Key 管理**：
  - 当前 Phase 0：从环境变量读取 LLM API Key，`.env.example` 仅提供占位值
  - v1.0 目标：支持加密配置、Vault/Secrets Manager 或等价密钥管理，不写入代码或普通配置文件
  - v1.0 目标：支持 Key 轮换、异常调用量告警和泄露应急禁用
- **LLM 输出安全**：
  - 代码执行（如果支持）在沙箱中运行，限制文件系统和网络访问
  - 生成的链接/文件路径做白名单校验，防止路径遍历攻击

### 8.3 macOS 权限控制
- L0 无系统权限，默认可用。
- L1 Calendar 权限优先申请 write-only；如需冲突检测或读取事件，再单独请求 full access。
- L2 App/URL/Shortcuts/AppleScript 必须使用 allowlist，用户可编辑白名单。
- L3 Accessibility 权限必须在设置页明确说明风险，所有代输入/快捷键行为可被用户实时停止。
- L4 Screen Recording + 自主点击属于远期高风险能力，默认关闭，不进入 MVP。
- 所有写操作记录 who/what/when/where/source_memory_ids/confirmation_id。
- API 限流：每用户 60 请求/分钟，LLM 调用 20 请求/分钟。

### 8.4 第三方与本机自动化安全
- Apple Calendar：只保存必要的 event id、标题、时间、用户确认记录；默认不读取备注和无关日历内容。
- Shortcuts：只运行用户预设或白名单中的快捷指令。
- AppleScript：只允许签名/白名单脚本；禁止 LLM 直接生成并无确认执行任意脚本。
- Accessibility：先展示计划，再执行；破坏性动作必须二次确认。
- 第三方 OAuth：后续扩展时 Token 加密存储，支持自动刷新和手动撤销。

### 8.5 合规
- 符合 GDPR 数据保护要求
- 支持数据导出 (JSON/CSV) 与删除 (被遗忘权)
- AI 决策可解释性：提供推理过程和数据来源
- 中国用户：符合《个人信息保护法》，数据存储可选境内节点

---

## 九、错误处理与降级策略

### 故障场景矩阵

| 故障场景 | 影响 | 降级策略 | 用户提示 |
|---------|------|---------|---------|
| **LLM API 超时 (>30s)** | 对话无响应 | 自动重试 1 次 → 切换备用模型 → 返回缓存相似回答 | "AI 思考中...已切换到备用模型" |
| **LLM API 返回错误 (429/500)** | 对话不可用 | 队列排队 + 指数退避重试 → 降级为本地小模型 | "云端 AI 暂时繁忙，已切换到本地模式" |
| **第三方平台 API 限流** | 数据同步延迟 | 队列缓存 + 批量合并请求 + 指数退避 | "数据同步延迟，预计 X 分钟后恢复" |
| **第三方平台 Token 过期** | 集成功能中断 | 自动刷新 Token → 失败则降级为只读 → 通知用户 | "第三方连接已过期，请重新授权" |
| **Qdrant 不可用** | 记忆检索失败 | 降级为 PostgreSQL 全文搜索 (精度降低) | "记忆搜索暂不可用，已切换到基础搜索" |
| **PostgreSQL 主从切换** | 短暂不可写 (~5s) | 连接池自动重连 + 写操作排队 | 用户无感知 (自动恢复) |
| **Redis 不可用** | 缓存失效 | 直接查数据库 (延迟增加 ~50ms) + 本地 L1 缓存兜底 | 用户无感知 (性能略降) |
| **WebSocket 断连** | 实时通信中断 | 自动重连 (指数退避 1s→2s→4s→...→30s) + 消息补发 | "连接已恢复" (重连后提示) |
| **STT 服务不可用** | 语音输入不可用 | 降级为纯文字输入 | "语音识别暂不可用，请使用文字输入" |
| **TTS 服务不可用** | 语音播放不可用 | 降级为纯文字输出 | 用户无感知 (无语音) |
| **3D 渲染异常** | 虚拟形象不显示 | 自动降级为 2D 精灵图 → 纯文字模式 | "已切换到简约模式" |
| **磁盘空间不足** | 写入失败 | 清理过期缓存/日志 → 告警管理员 | "系统存储空间不足，请联系管理员" |

---

## 十、数据备份与灾难恢复

### 备份策略

| 数据组件 | 备份方式 | 频率 | 保留周期 | 存储位置 |
|---------|---------|------|---------|---------|
| PostgreSQL | pg_basebackup 全量 + WAL 归档 | 每日全量 + 持续 WAL | 30 天 | 异地 S3/MinIO（v2.0 可选） |
| Qdrant | Collection Snapshot API | 每 6 小时 | 7 天 | 异地 S3/MinIO（v2.0 可选） |
| Redis | RDB 快照 + AOF 持久化 | RDB 每小时 + AOF 持续 | 7 天 | 本地 + 异地 |
| 文件存储 (v2.0 MinIO) | 跨节点复制 / 异地同步 | 实时 | 永久 | 异地 MinIO |

### 恢复目标

| 指标 | 目标值 | 说明 |
|------|--------|------|
| **RPO (恢复点目标)** | < 5 分钟 | 最多丢失 5 分钟数据（WAL 归档间隔） |
| **RTO (恢复时间目标)** | < 1 小时 | 从备份恢复完整服务的时间 |

### 恢复流程

```
1. 检测故障 → 告警通知管理员
2. 评估影响范围 (单组件 / 全系统)
3. PostgreSQL: pg_basebackup 恢复 + WAL 回放到故障前一刻
4. Qdrant: 从最近 Snapshot 恢复 Collection
5. Redis: 加载 RDB + AOF 重放 (自动)
6. 文件存储 (v2.0 MinIO): 从异地副本同步
7. 验证数据完整性 → 恢复服务 → 通知用户
```

### 监控告警阈值

| 指标 | 警告阈值 | 严重阈值 | 通知方式 |
|------|---------|---------|---------|
| API P95 延迟 | > 500ms 持续 5 分钟 | > 2s 持续 1 分钟 | 警告: macOS 通知/Slack, 严重: 电话 |
| 后端内存 (纯云端) | > 100MB | > 200MB | macOS 通知/Slack |
| LLM 调用错误率 | > 10% 持续 5 分钟 | > 30% 持续 1 分钟 | 严重: 电话 |
| 数据库连接池使用率 | > 80% | > 95% | macOS 通知/Slack |
| 磁盘使用率 | > 85% | > 95% | 警告: macOS 通知, 严重: 电话 |
| 备份失败 | 连续 2 次失败 | 连续 4 次失败 | macOS 通知/Slack |
| WebSocket 连接数 | > 500/实例 | > 1000/实例 | macOS 通知/Slack |

---

## 十一、商业模式设想

### 11.1 开源 + 增值服务

| 层级 | 内容 | 价格 |
|------|------|------|
| **社区版** | 核心功能开源，支持本地部署 | 免费 |
| **专业版** | 云托管 + 高级 AI 模型 + 更多集成 | ¥199/月 (详见 11.3 成本模型) |
| **企业版** | 私有部署 + 定制开发 + SLA 保障 | 按需报价 |

### 11.2 增值点
- 高级虚拟形象资源 (服装/场景/特效)
- 高级个人助理模板 (会议准备/复盘/专注计划/日程编排)
- AI 模型算力包
- 培训与咨询服务

### 11.3 成本模型 (v1.0 单用户)

| 成本项 | 月度成本 | 说明 |
|--------|---------|------|
| LLM API 调用 | ¥100-300 | 取决于所选云端模型、缓存命中率和小模型路由比例 |
| 云端 TTS (Edge TTS) | ¥0 | 免费额度充足 |
| 云端 STT (Whisper API) | ¥20-50 | 按分钟计费 |
| 向量模型 API (可选) | ¥0-30 | 可用本地 TEI 替代 |
| 服务器 (云托管模式) | ¥50-100 | 轻量云服务器 |
| **合计** | **¥170-480/月** | 专业版 ¥199/月，重度用户需小模型路由控制成本 |

> **盈亏平衡点**：专业版定价 ¥199/月，LLM 成本控制在 ¥100/用户/月以内（通过缓存+小模型路由），每用户毛利 ~¥50。需 200+ 付费用户覆盖基本运营成本。

---

## 十二、风险与应对

| 风险 | 影响 | 概率 | 应对策略 |
|------|------|------|---------|
| LLM 成本过高 | 运营成本不可控 | 高 | 本地模型兜底 + 智能路由 + 缓存 |
| 3D 渲染性能问题 | 低端设备体验差 | 中 | LOD 分级 + 降级为 2D 形象 |
| 第三方 API 变动 | 集成功能失效 | 中 | 适配层抽象 + 多平台备份 |
| 数据安全事件 | 信任危机 | 低 | 端到端加密 + 安全审计 + 应急预案 |
| 用户接受度低 | 推广困难 | 中 | 渐进式引导 + 自定义程度高 |
| Rust 生态 AI 工具不成熟 | 开发效率低 | 高 | LLM 编排自研 + 参考社区方案 + 预留额外工期 |
| 图数据库候选不可用 | 远期关系推理受限 | 低 | MVP 不依赖图数据库；AGE PoC 已失败，后续如需要图谱能力再评估 Neo4j/自建 AGE 镜像 |

---

## 十三、核心 API 规范

> API 路径统一使用 `/api/v1/*`。当前 OpenAPI 由 `utoipa` 生成，但 Phase 0 只登记了部分已实现端点；后续每新增路由必须同步 OpenAPI path/schema。

### 当前已实现端点

#### 系统 API

```
GET    /api/v1/system/health                 — 健康检查
GET    /api/v1/system/metrics                — Prometheus 指标占位
```

#### 本地会话与用户 API

```
POST   /api/v1/auth/local-session            — 获取本地默认用户会话 token
GET    /api/v1/users/me                      — 当前本地用户信息
PATCH  /api/v1/users/me                      — 更新当前用户信息
```

#### 对话 API

```
POST   /api/v1/chat                          — 发送消息 (SSE 流式响应，占位 echo)
GET    /api/v1/conversations                 — 获取会话列表
POST   /api/v1/conversations                 — 创建会话
GET    /api/v1/conversations/:id             — 获取会话详情
DELETE /api/v1/conversations/:id             — 删除会话
```

> `chat/sessions` 路由文件已存在，但尚未在 `main.rs` 合并进应用路由；接入前不计入可用 API。

#### 当前遗留项目/任务 API

```
GET    /api/v1/projects                      — 获取项目列表
POST   /api/v1/projects                      — 创建项目
GET    /api/v1/projects/:id                  — 获取项目详情
PATCH  /api/v1/projects/:id                  — 更新项目
DELETE /api/v1/projects/:id                  — 删除项目
GET    /api/v1/projects/:id/health           — 获取项目健康度
GET    /api/v1/projects/:pid/tasks           — 获取任务列表
POST   /api/v1/projects/:pid/tasks           — 创建任务
GET    /api/v1/projects/:pid/tasks/:tid      — 获取任务详情
PATCH  /api/v1/projects/:pid/tasks/:tid      — 更新任务
DELETE /api/v1/projects/:pid/tasks/:tid      — 删除任务
```

> v0.4 新定位下，项目/任务 API 暂作为历史实现和未来扩展保留，不进入 macOS MVP 的核心验收范围。

#### 记忆 API

```
GET    /api/v1/memories                      — 浏览记忆列表
GET    /api/v1/memories/search?q=...         — 搜索记忆（当前为 PostgreSQL `ILIKE` 文本搜索占位；混合向量+关键词+语义检索待 Phase 2 SoulLedger 实现）
```

### 规划端点

#### WebSocket API

```
WS     /ws/chat                              — WebSocket 实时对话与虚拟形象状态同步
```

当前 WebSocket handler 已有骨架，但尚未接入应用路由。

#### SoulLedger API

```
GET    /api/v1/soul-ledger/memories          — 分层浏览记忆
POST   /api/v1/soul-ledger/memories          — 手动创建/锁定记忆
PATCH  /api/v1/soul-ledger/memories/:id      — 编辑记忆内容、权重、标签、TTL
DELETE /api/v1/soul-ledger/memories/:id      — 删除记忆
POST   /api/v1/soul-ledger/memories/:id/forget — 标记软遗忘/硬遗忘
GET    /api/v1/soul-ledger/used-in-response/:message_id — 查看某次回复引用的记忆
```

#### Personality API

```
GET    /api/v1/personality/current           — 获取当前人格参数
GET    /api/v1/personality/snapshots         — 获取人格快照列表
POST   /api/v1/personality/snapshots/:id/restore — 回滚人格快照
PATCH  /api/v1/personality/settings          — 开关自动适应、调整 drift budget
GET    /api/v1/personality/events            — 查看人格变化事件
```

#### Calendar API

```
POST   /api/v1/calendar/parse                — 自然语言解析为日程草案
POST   /api/v1/calendar/events               — 用户确认后创建 Apple Calendar 事件
GET    /api/v1/calendar/permission           — 查询 Calendar 权限状态
POST   /api/v1/calendar/permission/request   — 触发 macOS Calendar 授权流程
GET    /api/v1/calendar/conflicts            — 检测拟创建日程的时间冲突
```

#### Permission API

```
GET    /api/v1/permissions                   — 查看 L0-L4 权限状态
PATCH  /api/v1/permissions/settings          — 修改权限开关和执行确认策略
GET    /api/v1/actions/audit-log             — 查看系统操作审计日志
POST   /api/v1/actions/confirm               — 确认待执行动作
POST   /api/v1/actions/cancel                — 取消待执行动作
```

### 统一错误响应

所有 JSON API 错误响应使用统一 envelope：

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "请求参数不合法"
  }
}
```

---

## 十四、测试策略

### 测试金字塔

```
          ┌───────────┐
          │  E2E 测试  │  ← 5-10 个核心用户流程
          │  (Playwright)│     对话→日程创建→记忆编辑
          ├───────────┤
          │  集成测试   │  ← API 接口 + 数据库 + 外部 Mock
          │  (cargo test)│     覆盖所有 API 端点
          ├───────────┤
          │  单元测试   │  ← 业务逻辑 + 工具函数
          │  (cargo test)│     覆盖率 ≥ 80%
          └───────────┘
```

### 各层测试策略

| 层级 | 工具 | 覆盖率目标 | 说明 |
|------|------|-----------|------|
| 单元测试 | cargo test | ≥ 80% | 业务逻辑、工具函数、数据结构 |
| 集成测试 | cargo test + testcontainers | 所有 API 端点 | 真实 PostgreSQL/Redis/Qdrant 容器 |
| LLM Mock | wiremock / mockito | 所有 LLM 调用路径 | 模拟 LLM 响应，避免测试依赖外部 API |
| 前端测试 | Vitest + React Testing Library | ≥ 70% | 组件渲染、用户交互、状态管理 |
| E2E 测试 | Playwright | 5-10 核心流程 | 对话→日程创建→记忆编辑→人格回滚完整链路 |
| 性能基准 | criterion.rs | 关键路径 | 每次 PR 自动运行，检测性能回退 |
| 安全测试 | cargo-audit + OWASP ZAP | — | 依赖漏洞扫描 + API 安全扫描 |

### LLM 测试特别策略

```rust
// 使用 wiremock 模拟 LLM API，确保测试不依赖外部服务
#[tokio::test]
async fn test_chat_with_mock_llm() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"content": "测试回复"}}]
        })))
        .mount(&mock_server).await;

    let app = create_test_app(mock_server.uri()).await;
    let response = send_chat_message(&app, "你好").await;
    assert_eq!(response.text, "测试回复");
}
```

---

## 十五、数据库迁移策略

### 工具选型：sqlx-cli

```bash
# 创建迁移
sqlx migrate add create_users_table

# 执行迁移 (自动按时间戳排序)
sqlx migrate run

# 回滚最近一次迁移
sqlx migrate revert

# 验证迁移状态
sqlx migrate info
```

### 迁移规范

1. **向前兼容**：每次迁移必须支持零停机部署
   - 新增列使用 `DEFAULT` 值，避免锁表
   - 删除列分两步：v1 停止读取 → v2 删除列
   - 重命名列分三步：新增 → 迁移数据 → 删除旧列

2. **迁移脚本命名**：`{timestamp}_{description}.sql`
   ```
   20260601_000001_create_users_table.sql
   20260601_000002_create_projects_table.sql
   20260602_000001_add_project_health_index.sql
   ```

3. **测试环境**：当前 CI 自动执行迁移；回滚测试待 Phase 1 补齐

4. **生产环境**：迁移前自动备份，支持一键回滚

---

## 十六、前后端类型契约

### 方案：OpenAPI 自动生成

```
Rust 后端 (utoipa)                    前端 (TypeScript)
  │                                      │
  │  编译期生成 openapi.json              │
  ├─────────────────────────────────────▶│
  │                                      │  openapi-typescript-codegen
  │                                      │  自动生成 API 客户端 + 类型定义
  │                                      │
  │  Phase 1 契约门禁:                    │
  │  生成 committed spec 并验证前端类型同步 │
  ◀─────────────────────────────────────┤
```

```rust
// Rust 端：utoipa 编译期生成 OpenAPI schema
#[derive(Deserialize, Serialize, ToSchema)]
pub struct ChatRequest {
    /// 用户消息内容
    pub message: String,
    /// 会话 ID (可选，不传则创建新会话)
    pub session_id: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/chat",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "流式响应", body = ChatResponse)
    )
)]
async fn chat_handler(/* ... */) { /* ... */ }
```

```typescript
// 前端：自动生成的类型 (CI 中运行 openapi-typescript-codegen)
// 不需要手动维护，每次后端 API 变更后自动重新生成
import { ChatService, ChatRequest, ChatResponse } from './generated/api';

const response = await ChatService.chatPost({ message: "你好" });
// 完整的类型提示和编译期检查
```

### Phase 1 契约门禁规划

当前 Phase 0 只在运行时暴露 Swagger/OpenAPI，并在 CI 中保留 placeholder job；还没有生成 `docs/api/openapi.json`、前端 generated client，也没有严格 diff gate。Phase 1 接入桌面壳和核心 API 后再启用以下门禁：

```yaml
# GitHub Actions: Phase 1 后每次 PR 验证前后端契约一致
- name: Generate OpenAPI spec
  run: cargo test generate_openapi -- --ignored
- name: Regenerate TypeScript client
  run: npx openapi-typescript-codegen -i openapi.json -o frontend/src/generated
- name: Check for breaking changes
  run: npx openapi-diff openapi.json frontend/src/generated/openapi.json
```

---

## 十七、总结

灵枢 (LingShu) 不是又一个“套壳 ChatGPT”，也不是只会点击屏幕的自动化代理，而是一个**有记忆、有性格、会遗忘、可审计、能分级操控 macOS 的桌面个人助理**。

它从四个维度重新定义桌面 AI 助理：

1. **有存在感** — 以桌面宠物形态常驻 macOS，而不是藏在浏览器标签页里。
2. **有连续性** — SoulLedger 分层记忆保证长期上下文和人格稳定，不被长上下文漂移冲掉。
3. **有边界** — 权限按 L0-L4 分级，Calendar/Shortcuts/Accessibility 等能力由用户逐级开启。
4. **有控制权** — 用户可以编辑记忆、删除记忆、禁记内容、开关性格自动适应并回滚人格快照。

MVP 的胜负不取决于形象是否足够精细，而取决于它能否让用户感到：这个助理真的记得我、理解我的工作节奏、会在恰当时机提出建议，并且每一次记忆和行动都在我的控制之内。

---

## 附录 A：高性能架构优化方案 (Performance Annex)

> **说明**：本附录中的 Rust/TypeScript/SQL 代码示例为 **设计参考与目标架构示意**，并非当前代码库中已存在的实现。其中 `moka` 三级缓存、Qdrant 集合配置、`ModelRouter` 意图路由、WebSocket 批处理、增量同步等模块在 Phase 0 代码中对应着简化版或尚未实现。实施前请以 `crates/` 下的实际代码为准。

> 本附录针对"保持原设计、重点做性能优化"的需求，逐层给出具体优化策略与资源预算。

### A.1 性能目标总览

| 指标 | 目标值 | 测量方式 |
|------|--------|---------|
| 首屏加载 (含 3D 形象) | < 2s (LCP) | Lighthouse |
| 3D 渲染帧率 | ≥ 30fps (空闲), ≥ 24fps (动画) | rAF 计数器 |
| 前端内存占用 | < 300MB (含 3D) | Chrome DevTools |
| API P95 响应时间 | **< 100ms** (非 LLM, Rust) | Prometheus |
| LLM 首 Token 延迟 | < 800ms (流式) | 自定义埋点 |
| 后端单实例内存 | **< 50MB** (Rust, 不含 LLM) | cAdvisor |
| 数据库查询 P99 | < 50ms | pg_stat_statements |
| WebSocket 消息延迟 | < 50ms (Rust) | 自定义埋点 |
| Docker Compose 总内存 (纯云端) | **< 600MB** (单用户, 不含本地模型) | docker stats |
| Docker Compose 总内存 (本地推理) | **< 8GB** (单用户, 含 Ollama/Whisper/TEI) | docker stats |
| Rust 后端冷启动 | < 50ms | 启动计时 |
| Rust 后端 Docker 镜像 | < 50MB | 镜像大小 |

---

### A.2 前端性能优化

#### A.2.1 3D 虚拟形象 — 分级渲染策略

3D 形象是最大的性能消耗点。采用**三级渲染策略**，根据设备能力和用户交互状态动态调整：

```
┌─────────────────────────────────────────────────────────┐
│                   渲染分级决策树                          │
│                                                         │
│  设备检测 (GPU/内存/核心数)                               │
│       │                                                 │
│       ├── 高性能设备 ──────────────────────────────────▶ │
│       │   ┌──────────────────────────────────────┐      │
│       │   │ Level 1: 全质量渲染                    │      │
│       │   │ - 完整 VRM 模型 (含物理模拟)           │      │
│       │   │ - 实时阴影 + 环境光遮蔽                │      │
│       │   │ - 60fps 目标                           │      │
│       │   │ - 面部 BlendShape 全精度               │      │
│       │   └──────────────────────────────────────┘      │
│       │                                                 │
│       ├── 中等设备 ──────────────────────────────────▶   │
│       │   ┌──────────────────────────────────────┐      │
│       │   │ Level 2: 标准渲染                     │      │
│       │   │ - 简化 VRM 模型 (LOD1)                │      │
│       │   │ - 烘焙光照贴图替代实时光影             │      │
│       │   │ - 30fps 目标                          │      │
│       │   │ - 面部核心 BlendShape (嘴型+眨眼)     │      │
│       │   └──────────────────────────────────────┘      │
│       │                                                 │
│       └── 低端设备/省电模式 ────────────────────────▶    │
│           ┌──────────────────────────────────────┐      │
│           │ Level 3: 轻量渲染                     │      │
│           │ - 2D 精灵图序列帧 (预渲染)             │      │
│           │ - CSS 动画替代 3D 变换                 │      │
│           │ - 15fps (仅表情切换时刷新)             │      │
│           │ - Canvas 2D 或纯 CSS 实现             │      │
│           └──────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
```

**关键优化技术：**

```typescript
// 模型优化
const avatarOptimizations = {
  // 1. GLTF 压缩 — 使用 Draco/KTX2 减少 70-80% 模型体积
  compression: 'Draco + KTX2 Basis Universal',

  // 2. 几何体优化 — 合并网格，减少 draw call
  meshMerging: '同一材质的子网格合并为单个 BufferGeometry',

  // 3. 纹理优化 — ASTC 压缩 + Mipmap + 按需加载
  textures: 'KTX2 压缩, 1024x1024 最大, 按部位懒加载',

  // 4. 骨骼简化 — 非可见骨骼直接移除
  skeleton: '移除衣服内部骨骼, 合并末端骨骼链',

  // 5. 动画压缩 — 关键帧抽稀 + 量化
  animation: '关键帧间隔 ≥ 33ms, 使用 AnimationObjectPool 复用'
};
```

**空闲状态优化：**
```typescript
// 当用户 30 秒无交互时，自动降级渲染
class AvatarRenderer {
  private idleTimer: number = 0;
  private currentLevel: 1 | 2 | 3 = 1;

  onUserInactive(seconds: number) {
    if (seconds > 30 && this.currentLevel === 1) {
      this.downgradeTo(2); // 停止物理模拟，降低帧率
    }
    if (seconds > 120) {
      this.downgradeTo(3); // 切换为静态图 + 微动画
    }
  }

  onUserActive() {
    this.upgradeTo(this.preferredLevel); // 恢复渲染
  }

  // 后台标签页时完全暂停渲染
  onVisibilityChange(hidden: boolean) {
    if (hidden) {
      this.pauseRenderLoop(); // requestAnimationFrame 自动暂停
      // 仅维持 WebSocket 心跳
    }
  }
}
```

#### A.2.2 资源加载策略

```
首屏加载流水线 (目标 < 2s):

0ms    ──▶ 加载 Shell HTML + 关键 CSS (内联)
50ms   ──▶ 渲染聊天 UI 骨架 (无 3D)
100ms  ──▶ 异步加载 JS Bundle (Code Splitting)
200ms  ──▶ 首次有意义的交互 (聊天输入框可用)
300ms  ──▶ 开始加载 3D 形象资源 (低优先级)
800ms  ──▶ 3D 形象基础模型就绪 (LOD0 最简版本)
1200ms ──▶ 3D 形象完整渲染 + 口型同步就绪
```

```typescript
// Code Splitting 策略
const routes = {
  // 核心 — 立即加载 (~80KB gzipped)
  '/':            lazy(() => import('./ChatCore')),

  // 3D 形象 — 空闲时预加载 (~200KB gzipped)
  '/avatar':      lazy(() => import('./AvatarScene')),

  // Memory & Personality Center — 路由触发时加载
  '/memory':      lazy(() => import('./MemoryCenter')),

  // Calendar 确认面板 — 按需加载
  '/calendar':    lazy(() => import('./CalendarPanel')),

  // 设置页面 — 按需加载
  '/settings':    lazy(() => import('./Settings')),
};
```

#### A.2.3 虚拟形象资源管理

```typescript
// 资源预算
const RESOURCE_BUDGET = {
  model: {
    maxVertices: 30_000,     // LOD0 最精细
    lod1Vertices: 15_000,    // 中等距离
    lod2Vertices: 5_000,     // 远距离/低性能
    maxTextures: 4,          // 贴图数量上限
    maxTextureSize: 1024,    // 单张贴图最大尺寸
  },
  animations: {
    maxConcurrent: 3,        // 同时播放的动画数
    blendShapeCount: 52,     // VRM 标准 BlendShape
    keyframeBudget: 200,     // 单动画最大关键帧数
  },
  memory: {
    modelBudget: '15MB',     // 模型内存预算
    textureBudget: '20MB',   // 纹理内存预算
    totalBudget: '50MB',     // 3D 总内存预算
  }
};
```

---

### A.3 后端性能优化 (Rust)

#### A.3.1 异步架构设计 (Tokio)

Rust 后端基于 Tokio 异步运行时，天然具备零开销并发能力，无需像 Python 那样精心管理连接池和 GIL 问题。

```rust
// Axum 服务配置 — 零开销并发
use axum::{Router, routing::get};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // 1. 数据库连接池 — 编译期 SQL 检查，零 ORM 开销
    let db_pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .connect(&database_url)
        .await
        .unwrap();

    // 2. Redis 连接 — 使用 fred 库，原生异步
    let redis = fred::prelude::RedisClient::new(
        fred::prelude::RedisConfig::from_url("redis://localhost").unwrap(),
        None, None, None,
    );
    redis.connect();
    redis.wait_for_connect().await.unwrap();

    // 3. HTTP 客户端 — reqwest 连接池，复用于所有外部 API 调用
    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(20)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    // 4. 路由 — 类型安全，编译期验证
    let app = Router::new()
        .route("/api/v1/chat", post(chat_handler))
        .route("/api/v1/projects", get(list_projects))
        .with_state(AppState { db_pool, redis, http_client });

    // 5. 启动 — 单二进制，冷启动 <50ms
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**对比 Python 的关键优势：**
- 无 GIL：多线程真正并行，CPU 密集计算 (健康度/图算法) 不阻塞 IO
- 零拷贝：serde 直接从网络 buffer 反序列化，无中间对象
- 编译期优化：所有 SQL 在编译期检查，运行时零反射开销
- 内存安全：无 GC 停顿，无内存泄漏，无 use-after-free

#### A.3.2 多级缓存架构 (Rust)

```
请求 → L1 缓存 (进程内 moka) → L2 缓存 (Redis) → L3 数据库
         │                        │                   │
         │ <0.1ms                 │ <1ms              │ <5ms
         │                        │                   │
         ├─ 热点数据               ├─ 会话数据          ├─ 持久化数据
         ├─ LRU 1000 项           ├─ TTL 可配          ├─ 索引优化
         └─ 容量: ~5MB            └─ 容量: 256MB       └─ 容量: 不限
```

```rust
// 三级缓存实现 — 使用 moka (Rust 高性能并发缓存)
use moka::future::Cache;
use std::time::Duration;

pub struct MultiLevelCache {
    // L1: 进程内缓存 (moka — 无锁并发，比 Python LRU 快 10x)
    l1: Cache<String, serde_json::Value>,
    // L2: Redis 连接
    redis: fred::prelude::RedisClient,
}

impl MultiLevelCache {
    pub fn new(redis: fred::prelude::RedisClient) -> Self {
        Self {
            l1: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(Duration::from_secs(60))
                .build(),
            redis,
        }
    }

    pub async fn get<F, Fut>(&self, key: &str, loader: F) -> Result<serde_json::Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value>>,
    {
        // L1 命中 — <0.1ms，无锁读取
        if let Some(val) = self.l1.get(key) {
            return Ok(val);
        }

        // L2 命中 — <1ms
        if let Ok(Some(cached)) = self.redis.get::<Option<String>, _>(key).await {
            let val: serde_json::Value = serde_json::from_str(&cached)?;
            self.l1.insert(key.to_string(), val.clone()).await;
            return Ok(val);
        }

        // L3: 数据库加载
        let val = loader().await?;
        self.set(key, &val, 300).await?;
        Ok(val)
    }

    pub async fn set(&self, key: &str, val: &serde_json::Value, ttl: u64) -> Result<()> {
        self.l1.insert(key.to_string(), val.clone()).await;
        let serialized = serde_json::to_string(val)?;
        self.redis.set::<(), _, _>(key, serialized, Some(Expiration::EX(ttl as i64)), None, false).await?;
        Ok(())
    }
}
```

**缓存策略明细：**

| 数据类型 | 缓存层 | TTL | 失效策略 |
|---------|--------|-----|---------|
| 用户配置 | L1+L2 | 1h | 主动失效 |
| 项目元数据 | L1+L2 | 5min | Webhook 触发 |
| 对话上下文 | L2 | 会话期 | 会话结束清除 |
| 向量检索结果 | L1 | 10min | moka 自然淘汰 |
| LLM 响应缓存 | L2 | 24h | 内容哈希匹配 |
| 知识图谱查询 | L1+L2 | 30min | 图谱更新触发 |
| 仪表盘数据 | L1+L2 | 2min | 定时刷新 |
| 第三方 API 响应 | L2 | 按 API | Webhook/定时 |

#### A.3.3 LLM 调用优化 (Rust)

LLM 调用是最昂贵的操作（延迟和成本），必须重点优化：

```rust
// 1. Prompt 精确缓存 — SHA-256 哈希匹配
use sha2::{Sha256, Digest};

pub struct PromptCache {
    cache: Cache<String, String>,  // hash → response
}

impl PromptCache {
    pub async fn get_or_fetch(&self, prompt: &str, fetch: impl FnOnce() -> impl Future<Output = String>) -> String {
        let hash = {
            let mut hasher = Sha256::new();
            hasher.update(prompt.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        if let Some(cached) = self.cache.get(&hash) {
            return cached;  // 命中，跳过 LLM 调用
        }

        let response = fetch().await;
        self.cache.insert(hash, response.clone()).await;
        response
    }
}

// 2. 智能模型路由 — 根据意图和复杂度选择最优模型
pub struct ModelRouter;

impl ModelRouter {
    pub fn route(&self, intent: &Intent, complexity: f32) -> ModelConfig {
        match complexity {
            c if c < 0.3 => ModelConfig::new("qwen2.5-7b", 512),     // 简单查询用小模型
            c if c < 0.7 => self.route_by_intent(intent),             // 中等复杂度按意图分
            _ => ModelConfig::new("gpt-4o", 4096),                    // 高复杂度用大模型
        }
    }

    fn route_by_intent(&self, intent: &Intent) -> ModelConfig {
        match intent {
            Intent::SimpleQuery | Intent::Chitchat => ModelConfig::new("qwen2.5-7b", 256),
            Intent::TaskPlanning => ModelConfig::new("gpt-4o", 2048),
            Intent::RiskAnalysis => ModelConfig::new("cloud-reasoning-large", 4096),
            Intent::ReportGeneration => ModelConfig::new("gpt-4o", 4096),
            _ => ModelConfig::new("gpt-4o", 2048),
        }
    }
}

// 3. 流式响应 — 零拷贝转发，首 Token 优先
pub async fn stream_llm_response(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event>>> {
    let stream = state.http_client
        .post(&state.llm_endpoint)
        .json(&req)
        .send()
        .await
        .unwrap()
        .bytes_stream();  // 直接转发字节流，无中间缓冲

    Sse::new(stream.map(|chunk| {
        Ok(Event::default().data(String::from_utf8_lossy(&chunk.unwrap())))
    }))
}
```

#### A.3.4 后台任务优化 (Rust)

Rust 后端无需 Celery 这样的重量级任务队列，直接用 Tokio 任务实现：

```rust
// 轻量级后台任务 — 直接用 Tokio spawn，无需额外进程
use tokio::sync::mpsc;

pub struct TaskScheduler {
    // 多级优先级通道
    high_priority: mpsc::Sender<BackgroundTask>,
    normal_priority: mpsc::Sender<BackgroundTask>,
    low_priority: mpsc::Sender<BackgroundTask>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        let (hp_tx, hp_rx) = mpsc::channel(100);    // P0: 紧急
        let (np_tx, np_rx) = mpsc::channel(500);    // P1-P2: 正常
        let (lp_tx, lp_rx) = mpsc::channel(1000);   // P3: 低优先级

        // 每个优先级独立的 worker，高优先级可抢占
        tokio::spawn(Self::worker_loop(hp_rx, "high"));
        tokio::spawn(Self::worker_loop(np_rx, "normal"));
        tokio::spawn(Self::worker_loop(lp_rx, "low"));

        Self { high_priority: hp_tx, normal_priority: np_tx, low_priority: lp_tx }
    }

    pub async fn submit(&self, task: BackgroundTask) {
        let tx = match task.priority {
            Priority::Critical | Priority::High => &self.high_priority,
            Priority::Normal => &self.normal_priority,
            Priority::Low => &self.low_priority,
        };
        let _ = tx.send(task).await;
    }

    async fn worker_loop(mut rx: mpsc::Receiver<BackgroundTask>, name: &str) {
        while let Some(task) = rx.recv().await {
            // 带超时执行
            match tokio::time::timeout(
                Duration::from_secs(task.timeout_secs),
                task.execute(),
            ).await {
                Ok(result) => { /* 处理结果 */ }
                Err(_) => { tracing::warn!(task = %task.id, "task timeout"); }
            }
        }
    }
}
```

**对比 Python Celery：**
- 无独立 Worker 进程：节省 ~200MB/Worker
- 无消息代理开销：直接 mpsc channel，零序列化
- 超时控制：tokio::time::timeout，精确到毫秒
- 内存占用：单个 Tokio 任务 ~几百字节 vs Celery Worker ~200MB

**任务优先级队列：**
```
┌─────────────────────────────────────────────────────┐
│  优先级队列 (Tokio mpsc channels)                    │
│                                                     │
│  P0 (紧急) ──▶ 用户实时交互 / 关键路径通知           │
│  P1 (高)   ──▶ 日程确认 / 权限请求 / 关键提醒         │
│  P2 (中)   ──▶ 记忆压缩 / 向量索引更新 / 建议计算     │
│  P3 (低)   ──▶ 数据同步 / 索引重建 / 日志归档        │
│                                                     │
│  每级独立 worker，高优先级通道容量小但 worker 优先消费  │
└─────────────────────────────────────────────────────┘
```

---

### A.4 数据库性能优化

#### A.4.1 PostgreSQL 调优

```sql
-- postgresql.conf 关键参数 (针对 8GB 内存服务器)

-- 内存分配
shared_buffers = '2GB'              -- 总内存的 25%
effective_cache_size = '6GB'        -- 总内存的 75%
work_mem = '64MB'                   -- 排序/哈希操作内存
maintenance_work_mem = '512MB'      -- VACUUM/CREATE INDEX 内存

-- WAL 配置
wal_buffers = '64MB'
checkpoint_completion_target = 0.9
max_wal_size = '2GB'

-- 连接
max_connections = 100               -- 配合连接池使用

-- 查询优化
random_page_cost = 1.1              -- SSD 存储
effective_io_concurrency = 200      -- SSD 并行 IO
```

**核心索引策略：**
```sql
-- 对话记录表 — 按时间和用户查询最频繁
CREATE INDEX CONCURRENTLY idx_messages_user_time
ON messages (user_id, created_at DESC);

-- 任务表 — 按项目和状态查询
CREATE INDEX CONCURRENTLY idx_tasks_project_status
ON tasks (project_id, status, priority DESC);

-- 全文搜索索引
CREATE INDEX CONCURRENTLY idx_documents_fts
ON documents USING gin(to_tsvector('chinese', title || ' ' || content));

-- 部分索引 — 只索引未完成的任务 (热点数据)
CREATE INDEX CONCURRENTLY idx_tasks_active
ON tasks (assignee_id, due_date)
WHERE status NOT IN ('done', 'cancelled');
```

#### A.4.2 向量数据库优化 (Qdrant)

Qdrant 是 Rust 实现的向量数据库，与后端技术栈一致，通过 HTTP API 调用。

```rust
// Qdrant 集合配置 — 通过 HTTP API 创建
use serde_json::json;

async fn setup_vector_collection(client: &reqwest::Client, base_url: &str) -> Result<()> {
    let config = json!({
        "vectors": {
            "size": 1024,               // BGE-M3 向量维度
            "distance": "Cosine"        // 余弦相似度
        },
        "optimizers_config": {
            "indexing_threshold": 20000, // 延迟索引构建，批量写入时减少开销
            "memmap_threshold": 20000    // 超过 2 万向量自动切换为内存映射模式
        },
        "on_disk_payload": true,         // payload 存磁盘，减少内存占用
        "quantization_config": {
            "scalar": {
                "type": "int8",          // 标量量化，减少 75% 内存
                "always_ram": true       // 量化索引常驻内存
            }
        }
    });

    client.put(&format!("{}/collections/project_memories", base_url))
        .json(&config)
        .send()
        .await?;
    Ok(())
}

// 向量检索 — 按项目分区，减少扫描范围
async fn search_memories(
    client: &reqwest::Client,
    base_url: &str,
    project_id: &str,
    query_vector: &[f32],
    limit: u32,
) -> Result<Vec<SearchResult>> {
    let body = json!({
        "vector": query_vector,
        "filter": {
            "must": [{ "key": "project_id", "match": { "value": project_id } }]
        },
        "limit": limit,
        "params": { "ef": 128 }  // 搜索宽度 (精度-速度权衡)
    });

    let resp = client.post(&format!("{}/collections/project_memories/points/search", base_url))
        .json(&body)
        .send()
        .await?
        .json::<SearchResponse>()
        .await?;

    Ok(resp.result)
}
```

#### A.4.3 关系图谱候选研究 (非 MVP)

> **当前状态**：2026-06-04 的 Apache AGE PoC 在标准 `postgres:16-bookworm` 镜像上失败，原因是镜像不包含 `age.control`。MVP 不依赖图数据库，下面内容仅作为远期关系图谱研究方向保留；进入实施前必须先完成新的 AGE 自建镜像 PoC 或 Neo4j 对比 PoC。

```sql
-- 候选方案示例：Apache AGE 图查询 (PostgreSQL 扩展)
-- 需要先加载扩展: LOAD 'age';
-- 设置搜索路径: SET search_path = ag_catalog, "$user", public;

-- 1. 创建图
SELECT create_graph('personal_context');

-- 2. 创建实体和关系 (通过 Rust sqlx 执行)
-- INSERT INTO personal_context."Person" (name, properties) ...
-- INSERT INTO personal_context."Event" (title, starts_at, properties) ...
-- INSERT INTO personal_context."Memory" (content, importance, properties) ...

-- 3. 查询：某人相关的日程与记忆 (1-2 跳)
SELECT * FROM cypher('personal_context', $$
    MATCH (p:Person {name: $name})-[:MENTIONED_IN]->(m:Memory)-[:RELATED_TO]->(e:Event)
    RETURN m.content, e.title, e.starts_at
$$) AS (memory agtype, event_title agtype, starts_at agtype);

-- 4. 查询：偏好影响的待确认建议 (限制深度)
SELECT * FROM cypher('personal_context', $$
    MATCH (pref:Memory {type: 'preference'})-[:SUPPORTS*1..2]->(suggestion:Thought)
    RETURN pref.content, suggestion.title, suggestion.confidence
    LIMIT 20
$$) AS (preference agtype, title agtype, confidence agtype);

-- 5. 性能优化：为高频查询创建物化视图
CREATE MATERIALIZED VIEW mv_memory_event_graph AS
SELECT * FROM cypher('personal_context', $$
    MATCH (m:Memory)-[:RELATED_TO]->(e:Event)
    RETURN m.id, m.content, e.id, e.title
$$) AS (memory_id agtype, memory_content agtype, event_id agtype, event_title agtype);

-- 定期刷新 (每 30 分钟)
-- REFRESH MATERIALIZED VIEW CONCURRENTLY mv_memory_event_graph;
```

---

### A.5 网络与通信优化 (Rust)

#### A.5.1 WebSocket 管理

```rust
// WebSocket 连接管理 — Axum + tokio-tungstenite
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut interval = tokio::time::interval(Duration::from_secs(20)); // 心跳 20s
    let mut msg_buffer: Vec<String> = Vec::new();
    let batch_interval = Duration::from_millis(50); // 50ms 批量合并

    loop {
        tokio::select! {
            // 心跳检测
            _ = interval.tick() => {
                if sender.send(Message::Ping(vec![])).await.is_err() {
                    break; // 连接已断开
                }
            }
            // 接收消息
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        msg_buffer.push(text);
                        // 50ms 内的消息批量处理
                        tokio::time::sleep(batch_interval).await;
                        if !msg_buffer.is_empty() {
                            let batch = msg_buffer.drain(..).collect::<Vec<_>>();
                            tokio::spawn(process_message_batch(batch, state.clone()));
                        }
                    }
                    Some(Ok(Message::Pong(_))) => { /* 心跳响应 */ }
                    Some(Err(_)) | None => break,
                    _ => {}
                }
            }
        }
    }
}
```

#### A.5.2 API 响应优化 (Rust)

```rust
// 1. Brotli 压缩中间件 — Axum tower-http
use tower_http::compression::CompressionLayer;

let app = Router::new()
    .route("/api/v1/chat", post(chat_handler))
    .layer(CompressionLayer::new().br(true).gzip(true)); // Brotli + gzip

// 2. 增量同步 — 只返回变更部分
pub async fn delta_sync(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(params): Query<SyncParams>,
) -> Json<DeltaResponse> {
    let changes = sqlx::query_as!(
        ChangeSet,
        "SELECT id, data, updated_at FROM project_data
         WHERE project_id = $1 AND updated_at > $2
         ORDER BY updated_at",
        project_id,
        params.last_sync
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap();

    Json(DeltaResponse {
        added: changes.iter().filter(|c| c.created_at > params.last_sync).collect(),
        modified: changes.iter().filter(|c| c.created_at <= params.last_sync).collect(),
        deleted_ids: get_deleted_ids(&state.db_pool, &project_id, params.last_sync).await,
        sync_token: Utc::now().to_rfc3339(),
    })
}
```

---

### A.6 AI/ML 推理优化

> AI 模型作为独立微服务运行，以下配置通过 HTTP API 传递给对应服务。Rust 后端本身不运行模型推理。

#### A.6.1 本地模型优化 (Ollama 配置)

```yaml
# Ollama Modelfile — 本地 LLM 配置
# 通过 Rust 后端 HTTP 调用: POST http://localhost:11434/api/chat

FROM qwen2.5:7b-instruct-q4_K_M   # 4-bit 量化，精度-体积最佳平衡
PARAMETER num_ctx 4096              # 上下文窗口，4K tokens 足够日程/记忆类对话
PARAMETER num_predict 1024          # 最大生成 token 数
PARAMETER num_thread 4              # CPU 线程数
PARAMETER num_gpu 0                 # 0 = 纯 CPU，节省显存给前端 3D 渲染
PARAMETER use_mmap true             # 内存映射加载
PARAMETER temperature 0.7           # 生成温度
```

```rust
// Rust 后端调用 Ollama API
pub async fn call_local_llm(client: &reqwest::Client, prompt: &str) -> Result<String> {
    let body = json!({
        "model": "qwen2.5:7b",
        "messages": [{"role": "user", "content": prompt}],
        "stream": false,
        "options": { "num_ctx": 4096, "temperature": 0.7 }
    });
    let resp = client.post("http://localhost:11434/api/chat")
        .json(&body).send().await?
        .json::<OllamaResponse>().await?;
    Ok(resp.message.content)
}
```

#### A.6.2 STT/TTS 优化 (独立服务配置)

```yaml
# STT 服务配置 (faster-whisper 独立进程)
stt:
  model: "small"              # small 模型 (~461MB) 足够个人助理语音指令场景
  language: "zh"              # 固定语言，跳过语言检测
  beam_size: 3                # 减小 beam search 宽度
  condition_on_previous: false # 禁用前文条件，减少幻觉
  vad_filter: true            # 静音检测，跳过无语音段
  device: "cpu"               # CPU 推理，节省 GPU 给前端 3D

# TTS 服务配置
tts:
  engine: "edge-tts"          # 云端 TTS，零本地资源占用
  cache_enabled: true          # 缓存常用短语的音频
  cache_size: 100              # 缓存条目数
  streaming: true              # 流式合成，边生成边播放
  max_text_length: 500         # 单次合成最大文本长度
```

```rust
// Rust 后端调用 STT 服务 — 流式转发音频
pub async fn transcribe_audio(client: &reqwest::Client, audio: Bytes) -> Result<String> {
    let form = reqwest::multipart::Form::new()
        .part("audio", reqwest::multipart::Part::bytes(audio.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")?);
    let resp = client.post("http://localhost:8081/transcribe")
        .multipart(form).send().await?
        .json::<TranscriptionResponse>().await?;
    Ok(resp.text)
}
```

---

### A.7 资源监控与自适应

#### A.7.1 资源监控仪表盘

```rust
// Prometheus 指标采集 (prometheus-client)
use prometheus_client::metrics::{counter::Counter, gauge::Gauge, histogram::Histogram};

pub struct Metrics {
    // 应用指标
    pub http_request_duration: Histogram,
    pub websocket_connections: Gauge,
    pub llm_request_duration: Histogram,
    pub llm_tokens_used: Counter,
    pub cache_hit_ratio: Gauge,

    // 系统指标 — Rust 进程天然轻量，内存 <50MB
    pub process_memory_bytes: Gauge,

    // 业务指标
    pub active_projects: Gauge,
    pub messages_processed: Counter,
    pub risks_detected: Counter,
}
```

#### A.7.2 自适应降级策略 (Rust)

```rust
// 当系统资源紧张时自动降级
use std::sync::atomic::{AtomicU8, Ordering};

pub struct AdaptiveDegradation {
    current_level: AtomicU8,  // 0-3, 无锁原子操作
}

impl AdaptiveDegradation {
    const CPU_THRESHOLD: f64 = 80.0;
    const MEMORY_THRESHOLD: f64 = 85.0;
    const LATENCY_THRESHOLD_MS: u64 = 2000;

    pub fn evaluate(&self, metrics: &SystemMetrics) -> DegradationLevel {
        let mut level: u8 = 0;
        if metrics.cpu_percent > Self::CPU_THRESHOLD { level = level.max(1); }
        if metrics.memory_percent > Self::MEMORY_THRESHOLD { level = level.max(2); }
        if metrics.p95_latency_ms > Self::LATENCY_THRESHOLD_MS { level = level.max(1); }
        self.current_level.store(level.min(3), Ordering::Relaxed);
        DegradationLevel::from(level.min(3))
    }

    pub fn actions(&self) -> &'static str {
        match self.current_level.load(Ordering::Relaxed) {
            0 => "全功能运行",
            1 => "关闭 3D 物理模拟，降低渲染帧率到 24fps，LLM 降级为小模型",
            2 => "3D 形象切换为 2D 精灵图，暂停知识图谱更新，缓存积极预热",
            3 => "仅核心对话功能，暂停所有后台任务，通知用户系统繁忙",
            _ => unreachable!(),
        }
    }
}
```

---

### A.8 资源预算明细

> 资源预算分为两种部署模式，核心区别在于 AI 模型是否在本地运行。

#### 模式 A：纯云端 LLM（推荐入门）

LLM 调用走云端 API（OpenAI/Claude/Qwen 等），本地不部署任何 AI 模型。适合个人开发者和小团队快速上手。

**单用户部署：**

| 组件 | CPU | 内存 | 磁盘 | 说明 |
|------|-----|------|------|------|
| PostgreSQL | 0.25 核 | 256MB | 2GB | 主数据库；AGE 不进入 MVP |
| Redis | 0.1 核 | 64MB | 128MB | 缓存 + Streams 队列 |
| Qdrant | 0.15 核 | 128MB | 512MB | 内存映射模式 |
| **Axum 后端** | **0.15 核** | **30MB** | **20MB** | **静态编译, 单二进制** |
| 前端 (Nginx) | 0.05 核 | 32MB | 50MB | 静态资源 |
| **合计** | **~0.7 核** | **~510MB** | **~3.2GB** | **Docker Compose** |

**团队部署 (10-50 人)：**

| 组件 | CPU | 内存 | 磁盘 | 说明 |
|------|-----|------|------|------|
| PostgreSQL | 2 核 | 2GB | 50GB | 主从复制；AGE 不进入 MVP |
| Redis | 0.5 核 | 512MB | 2GB | Cluster 模式 |
| Qdrant | 1 核 | 1GB | 10GB | 分布式, 内存映射 |
| Axum 后端 (x3) | 1.5 核 | 90MB | 60MB | 多实例, 每实例 ~30MB |
| Meilisearch (v2.0 可选) | 0.5 核 | 256MB | 2GB | 全文搜索 |
| **合计** | **~5.5 核** | **~4GB** | **~64GB** | **Docker Compose / K8s** |

> **优势**：内存极低（单用户 510MB），无需 GPU，LLM 能力随云端模型升级自动提升。
> **成本**：LLM API 调用费随模型供应商和上下文长度变化，重度用户约 ¥100-300/月；需要通过缓存、小模型路由和调用预算控制成本。

---

#### 模式 B：本地推理（隐私敏感 / 离线场景）

本地部署 LLM（Ollama）+ STT（faster-whisper）+ 向量模型（TEI）。数据不出内网，但需要更多硬件资源。

**单用户部署：**

| 组件 | CPU | 内存 | 磁盘 | GPU | 说明 |
|------|-----|------|------|-----|------|
| PostgreSQL | 0.25 核 | 256MB | 2GB | - | 主数据库；AGE 不进入 MVP |
| Redis | 0.1 核 | 64MB | 128MB | - | 缓存 |
| Qdrant | 0.15 核 | 128MB | 512MB | - | 内存映射 |
| Axum 后端 | 0.15 核 | 30MB | 20MB | - | 单二进制 |
| 前端 (Nginx) | 0.05 核 | 32MB | 50MB | - | 静态资源 |
| **Ollama (Qwen2.5-7B Q4)** | **2 核** | **5GB** | **4GB** | **可选** | **本地 LLM** |
| **faster-whisper (small)** | **1 核** | **1.5GB** | **1.5GB** | **可选** | **本地 STT** |
| **TEI (BGE-M3)** | **0.5 核** | **800MB** | **1GB** | **可选** | **向量模型** |
| **合计** | **~4.2 核** | **~7.8GB** | **~9.4GB** | **可选** | **Docker Compose** |

**团队部署 (10-50 人)：**

| 组件 | CPU | 内存 | 磁盘 | 说明 |
|------|-----|------|------|------|
| PostgreSQL | 2 核 | 2GB | 50GB | 主从复制；AGE 不进入 MVP |
| Redis | 0.5 核 | 512MB | 2GB | Cluster |
| Qdrant | 1 核 | 1GB | 10GB | 分布式 |
| Axum 后端 (x3) | 1.5 核 | 90MB | 60MB | 多实例 |
| Meilisearch (v2.0 可选) | 0.5 核 | 256MB | 2GB | 全文搜索 |
| Ollama (7B Q4, x2) | 4 核 | 10GB | 8GB | 2 实例负载均衡 |
| faster-whisper | 2 核 | 3GB | 3GB | STT 服务 |
| TEI | 1 核 | 1.6GB | 2GB | 向量模型 |
| **合计** | **~12.5 核** | **~18.5GB** | **~77GB** | **K8s 部署** |

> **优势**：数据完全私有，零 API 调用费用，离线可用。
> **硬件要求**：建议 16GB+ 内存，有 GPU 可显著加速推理（但非必须，CPU 也能跑 7B Q4 模型）。

---

#### 资源对比总结

| 场景 | 单用户内存 | 团队内存 | GPU 需求 | 月度 API 成本 |
|------|-----------|---------|---------|-------------|
| **模式 A: 纯云端** | **510MB** | **4GB** | 不需要 | ¥100-300 |
| **模式 B: 本地推理** | **7.8GB** | **18.5GB** | 可选 (推荐) | ¥0 |

用户可根据自身硬件条件和隐私需求自由选择。

---

### A.9 关键优化 Checklist

开发过程中每个 PR 必须通过以下性能检查：

```
前端:
  □ Lighthouse Performance Score ≥ 90
  □ Bundle Size < 500KB (gzipped, 不含 3D 模型)
  □ 3D 形象内存 < 50MB
  □ 无内存泄漏 (Chrome DevTools Memory Snapshot 连续 3 次无增长)

后端 (Rust):
  □ cargo clippy 零 warning
  □ cargo test 全通过
  □ API 非 LLM 接口 P95 < 100ms (Rust 比 Python 更快)
  □ 后端进程内存 < 50MB (Rust 无 GC)
  □ 数据库查询无全表扫描 (EXPLAIN ANALYZE 验证)
  □ 新增缓存策略有 TTL 和失效机制
  □ 后台任务有超时和重试机制
  □ 无 unsafe 代码 (或 unsafe 经过严格审计)

AI:
  □ LLM 调用有缓存命中率统计
  □ 简单任务不调用大模型
  □ 流式响应首 Token < 800ms
  □ Token 消耗有计量和告警

基础设施:
  □ Rust 后端 Docker 镜像 < 50MB (distroless 基础镜像)
  □ 全栈内存 < 600MB (单用户, Rust 后端)
  □ 无资源泄漏 (24 小时稳定性测试)
  □ 编译时间 < 3 分钟 (增量编译 < 30s)
```

---

*本附录与主 PRD 同版本迭代，所有性能指标将随实际测试数据持续更新。*
