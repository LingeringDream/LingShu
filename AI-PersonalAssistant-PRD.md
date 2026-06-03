# 灵枢 (LingShu) — AI 项目经理个人助理

## 项目代号：LingShu
## 版本：v0.3 (经三轮审查修订)
## 日期：2026-05-27
## 作者：PM Office

---

## 目录

- **一、项目概述** — 愿景、命名、核心差异化
- **二、竞品调研与启示** — Open Interpreter / Khoj / JARVIS 分析
- **三、核心功能设计** — 虚拟形象、智能引擎、工作台、个性化
- **四、系统架构** — 整体架构图、核心模块详解
- **五、技术栈选型** — 前端(Rust后端)、AI/ML、数据存储、部署
- **六、项目里程碑与开发计划** — 36 周 7 Phase + v2.0 规划
- **七、数据流与交互时序** — 对话流程、主动监控流程
- **八、安全与隐私** — 数据安全、LLM安全、权限、合规
- **九、错误处理与降级策略** — 12 种故障场景矩阵
- **十、数据备份与灾难恢复** — 备份策略、RTO/RPO、告警阈值
- **十一、商业模式设想** — 开源+增值服务、成本模型
- **十二、风险与应对** — 7 项风险及应对策略
- **十三、核心 API 规范** — 7 大类 ~25 个端点
- **十四、测试策略** — 测试金字塔、LLM Mock
- **十五、数据库迁移策略** — sqlx-cli、向前兼容规范
- **十六、前后端类型契约** — OpenAPI 自动生成、CI 契约测试
- **十七、总结**
- **附录 A：高性能架构优化方案** — 逐层性能优化 + 资源预算

---

## 一、项目概述

### 1.1 项目愿景

打造一款面向项目经理的 AI 个人助理——**灵枢**，它拥有独特的 3D 虚拟形象，具备深度项目管理智能，能够主动感知、分析并协助完成日常办公中的各项任务。它不仅是一个工具，更是一个"数字同事"——有自己的性格、记忆和成长轨迹。

### 1.2 命名由来

**灵枢**取自中国古代医学经典《灵枢经》，"灵"意为灵性、智慧，"枢"意为枢纽、核心。寓意这款助理是项目经理工作中的智慧中枢，连接人、事、物的关键节点。

### 1.3 核心差异化

与市面上现有的 AI 助理不同，灵枢具备以下独特定位：

| 维度 | 通用 AI 助理 (ChatGPT/Khoj) | 代码执行型 (Open Interpreter) | 模型调度型 (JARVIS) | **灵枢 LingShu** |
|------|---------------------------|----------------------------|--------------------|--------------------|
| 定位 | 通用问答 | 本地代码执行 | 多模型协作 | **PM 专属智能助理** |
| 形象 | 无 | 终端文字 | Web 文字 | **3D 活体虚拟形象** |
| 交互 | 被动响应 | 被动响应 | 被动响应 | **主动感知 + 被动响应** |
| 记忆 | 会话级 | 无 | 无 | **长期记忆 + 知识图谱** |
| 领域 | 通用 | 编程 | 多模态任务 | **项目管理全链路** |

---

## 二、竞品调研与启示

### 2.1 Open Interpreter
- **GitHub**: [OpenInterpreter/open-interpreter](https://github.com/OpenInterpreter/open-interpreter)
- **Stars**: 57k+
- **核心能力**: 让 LLM 在本地执行 Python/JS/Shell 代码，终端 ChatGPT 式交互
- **技术亮点**: 支持多 LLM 后端 (GPT-4, Claude, 本地模型)，FastAPI 服务器模式，Profile 配置系统
- **对灵枢的启示**:
  - 本地执行能力可借鉴——灵枢需要能直接操作用户的文件系统、生成报表
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
  - 多源文档检索——项目文档、会议纪要、邮件的统一检索
  - 主动推送机制——定时报告、风险预警

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

### 3.1 虚拟形象系统 — "灵体"

灵枢的灵魂载体，一个可以感知情绪、表达状态的 3D 虚拟形象。

#### 3.1.1 形象设计
- **基础形象**: 半写实风格的 3D 角色，面部细节丰富，支持口型同步
- **自定义系统**: 用户可选择不同风格 (写实/卡通/科幻)，调整发型、服装、配饰
- **场景适配**: 根据工作场景切换形象状态——开会时穿正装，创意讨论时穿休闲装

#### 3.1.2 情绪与状态引擎
- **情绪映射**: 根据对话内容和项目状态，虚拟形象会展现不同情绪
  - 项目进度正常 → 微笑、自信
  - 发现风险 → 关注、提醒的表情
  - 截止日期临近 → 紧迫、专注
  - 完成里程碑 → 庆祝动画
- **状态指示**: 通过形象的视觉元素传达信息
  - 头顶光环颜色 = 项目健康度 (绿/黄/红)
  - 手中道具 = 当前工作模式 (笔记本=文档、望远镜=分析、盾牌=风控)

#### 3.1.3 技术实现路径
```
前端渲染层: Three.js + React Three Fiber
模型格式: VRM (VRoid 标准) 或 glTF 2.0
口型同步: Web Speech API → 音素映射 → BlendShape 驱动
表情系统: 基于情感分析结果的表情状态机
动画系统: 混合动画 (Idle + 情绪动画 + 手势动画)
```

---

### 3.2 智能引擎 — "灵核"

灵枢的大脑，基于多层 AI 架构构建的项目管理智能系统。

> **版本范围说明**：以下功能标注 `[v1.0]` 为首发版本包含，`[v2.0]` 为后续迭代。

#### 3.2.1 对话智能 [v1.0]
- **多轮对话管理**: 维护上下文，支持话题切换与回溯
- **意图识别**: 区分信息查询、任务指派、决策辅助、情感支持等不同意图
- **多模态输入**:
  - 文字：自然语言交互
  - 语音：实时语音对话 (STT + TTS)
  - 文件：拖拽文档直接分析
  - 截图/图片：OCR + 视觉理解

#### 3.2.2 项目管理智能（核心差异化）

**这是灵枢最独特的部分**——它不是一个通用聊天机器人加上 PM 皮肤，而是一个真正理解项目管理的 AI 系统。

**a) 项目态势感知 [v1.0]**
- 自动连接项目数据源 (v1.0: Jira, 飞书；v2.0: Asana, Linear, 钉钉)
- 实时计算项目健康指标：
  - 进度偏差 (Schedule Variance)
  - 成本偏差 (Cost Variance)
  - 风险暴露值 (Risk Exposure)
  - 团队负载均衡度
  - 依赖关系阻塞指数
- 生成"项目心电图"——用可视化方式呈现项目脉搏

**b) 智能会议助手 [v1.0]**
- 会议前：自动生成议程 (基于待办事项和上次纪要)
- 会议中：实时转录 + 关键信息提取 + 行动项识别
- 会议后：自动生成结构化纪要，分配 action items，设置提醒

**c) 风险预警系统 [v2.0]**
- 基于历史数据和当前指标，预测潜在风险
- 风险分类：进度风险、资源风险、技术风险、范围风险
- 自动生成应对方案建议
- 通过虚拟形象的表情变化 + 桌面通知 + 邮件多渠道预警

**d) 知识图谱 [v2.0]**
- 自动构建项目知识图谱：人员 → 任务 → 文档 → 决策 的关系网络
- 支持自然语言查询："上次关于 API 性能优化的讨论结论是什么？"
- 新成员 onboarding 时可快速了解项目全貌

**e) 报表与汇报自动生成 [v1.0 基础 / v2.0 高级]**
- [v1.0] 自动生成周报/月报/项目状态报告 (Markdown/纯文本)
- [v2.0] 支持多种格式：PPT、Word、PDF、飞书文档
- [v2.0] 可根据汇报对象调整详略程度 (给 CEO 的 vs 给团队的)

#### 3.2.3 主动智能（不只是等你来问）

灵枢最与众不同的是它的**主动性**——它不只是一个被动回答问题的工具：

| 触发条件 | 主动行为 |
|---------|---------|
| 每日早晨 | 推送今日优先事项和日程概览 |
| 会议前 15 分钟 | 提醒会议 + 推送相关资料 |
| 任务即将逾期 | 预警 + 建议应对措施 |
| 检测到依赖阻塞 | 通知相关方 + 建议替代路径 |
| 周五下午 | 自动生成本周总结 + 下周计划 |
| 项目里程碑完成 | 祝贺 + 回顾分析 |
| 收到重要邮件/消息 | 摘要 + 分类 + 优先级判断 |

---

### 3.3 工作台集成 — "灵域"

灵枢的工作空间，将散落各处的工具统一在一个界面中。

#### 3.3.1 核心集成

```
┌─────────────────────────────────────────────────────────┐
│                    灵枢工作台 (LingShu Workspace)         │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │ 项目看板  │  │ 日程管理  │  │ 文档中心  │  │ 通讯聚合 │ │
│  │ Kanban   │  │ Calendar │  │ Documents│  │ Comms   │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │ 风险面板  │  │ 团队状态  │  │ 知识库    │  │ 数据面板 │ │
│  │ Risks    │  │ Team     │  │ Wiki     │  │ Metrics │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │            灵枢对话窗口 (含 3D 虚拟形象)           │    │
│  │         ┌─────┐                                  │    │
│  │         │ 🤖  │  "今天的站会纪要已生成，            │    │
│  │         │     │   有 2 个阻塞项需要你关注。"       │    │
│  │         └─────┘                                  │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

#### 3.3.2 第三方集成清单

**v1.0 首发集成：**

| 类别 | 平台 | 集成方式 | 优先级 |
|------|------|---------|--------|
| 项目管理 | Jira | API + Webhook | P0 |
| 项目管理 | 飞书项目 | API | P0 |
| 文档协作 | 飞书文档 | API | P1 |
| 日历 | 飞书日历 | API | P1 |
| 代码托管 | GitHub | Webhook + API | P1 |
| 通讯协作 | 飞书 Bot | Bot API | P1 |

**v2.0 扩展集成：**

| 类别 | 平台 |
|------|------|
| 项目管理 | Asana, Linear, Trello, Teambition |
| 通讯协作 | Slack, Teams, 钉钉, 企业微信 |
| 文档协作 | Notion, Confluence, Google Docs |
| 日历 | Google Calendar, Outlook |
| 代码托管 | GitLab, Bitbucket |
| 设计工具 | Figma, Sketch |
| 数据分析 | Metabase, Grafana, Tableau |

---

### 3.4 个性化与成长系统 — "灵性"

灵枢不是一个静态工具，它会随着使用而"成长"。

#### 3.4.1 长期记忆
- **项目记忆**: 记住每个项目的关键决策、历史背景、人员关系
- **个人偏好**: 学习用户的沟通风格、工作习惯、关注重点
- **模式识别**: 发现用户的工作模式 (比如总在周三下午处理审批)

#### 3.4.2 技能成长
- 初始阶段：基础问答 + 简单任务
- 成长阶段：主动建议 + 模式识别
- 成熟阶段：预测性洞察 + 自动化流程
- 用户可通过反馈 (👍/👎) 和纠正来训练灵枢

#### 3.4.3 性格可定制
- **严肃型**: 简洁、高效、数据驱动
- **亲和型**: 温暖、鼓励、善于共情
- **幽默型**: 轻松、风趣、善用比喻
- 用户也可以自定义性格参数

---

## 四、系统架构

### 4.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        客户端层 (Client Layer)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Web App     │  │  Desktop App │  │  移动端 (PWA/Native)  │  │
│  │  (React+R3F) │  │  (Electron)  │  │  (React Native)      │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
│         │                 │                      │              │
│         └─────────────────┼──────────────────────┘              │
│                           │                                     │
│                    ┌──────┴───────┐                              │
│                    │  3D 渲染引擎  │                              │
│                    │  Three.js    │                              │
│                    │  VRM Avatar  │                              │
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
│  │ 对话引擎  │  │  PM 智能引擎 │  │ 集成引擎  │  │ 通知引擎  │    │
│  │ Dialog   │  │  PM Engine  │  │ Integr.  │  │ Notify   │    │
│  │ Engine   │  │             │  │ Engine   │  │ Engine   │    │
│  └──────────┘  └─────────────┘  └──────────┘  └──────────┘    │
│                                                                │
│  ┌──────────┐  ┌─────────────┐  ┌──────────┐  ┌──────────┐    │
│  │ 记忆引擎  │  │  知识图谱引擎 │  │ 报表引擎  │  │ 调度引擎  │    │
│  │ Memory   │  │  Knowledge  │  │ Report   │  │ Scheduler│    │
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
│  │PostgreSQL│  │ Redis    │  │ Qdrant   │  │ MinIO/S3     │   │
│  │ + AGE    │  │ 缓存/队列 │  │ 向量数据库│  │ 文件存储     │   │
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
  - 意图分类器 (PM 专属意图: 创建任务/查询进度/生成报告/...)
  - 槽位填充 (从自然语言中提取结构化信息)
  - 多模态输入处理 (文字/语音/文件/图片)
  - 流式响应 (SSE/WebSocket, 零拷贝转发 LLM 流式输出)
性能特征:
  - 对话管理本身内存 < 5MB/会话
  - LLM 响应流式转发: 无中间缓冲，直接 pipe
  - 意图分类: 本地规则引擎 + 轻量分类器 (<1ms)
```

#### 4.2.2 PM 智能引擎 (PM Intelligence Engine)
```
职责: 项目管理领域的核心智能
技术栈: Rust (自研规则引擎 + 图算法库 petgraph) + 外部 LLM
核心能力:
  - 项目健康度计算 (多维指标加权, Rust 原生计算)
  - 风险预测模型 (调用外部 ML 服务)
  - 资源优化建议 (Rust 线性规划库 good_lp)
  - 依赖关系分析 (petgraph: 关键路径/拓扑排序, <1ms 万级节点)
  - 会议智能 (议程生成/纪要提取/Action Item 分配 → LLM)
性能特征:
  - 健康度计算: <0.1ms (纯 Rust 数值计算)
  - 依赖分析: <1ms (10K 节点图)
  - LLM 调用: 流式异步, 不阻塞其他请求
```

#### 4.2.3 知识图谱引擎 (Knowledge Graph Engine)
```
职责: 构建和维护项目知识网络
技术栈: PostgreSQL + Apache AGE 扩展 + LLM 实体提取
数据模型:
  (人员)-[:负责]->(任务)
  (任务)-[:依赖]->(任务)
  (任务)-[:属于]->(里程碑)
  (决策)-[:影响]->(任务)
  (文档)-[:记录]->(会议)
  (风险)-[:关联]->(任务)
核心能力:
  - 自动从对话/文档中提取实体和关系 (LLM)
  - Cypher 查询通过 sqlx 直接执行 (无需额外驱动)
  - 路径分析 ("这个需求变更会影响哪些下游任务？")
性能特征:
  - 图查询通过 PostgreSQL 统一连接池, 无额外连接开销
  - 省去 Neo4j 进程: 节省 ~300MB 内存
```

#### 4.2.4 记忆引擎 (Memory Engine)
```
职责: 管理灵枢的长期记忆
技术栈: Qdrant (Rust 向量数据库) + PostgreSQL
记忆类型:
  - 工作记忆 (当前会话上下文) → Redis
  - 短期记忆 (最近 7 天的交互摘要) → PostgreSQL
  - 长期记忆 (关键决策/模式/偏好) → Qdrant
  - 情景记忆 (具体事件的详细记录) → Qdrant
核心能力:
  - 记忆存储/检索/遗忘 (基于重要性和时效性)
  - 相似记忆关联 (HNSW 向量搜索, <5ms)
  - 记忆压缩 (将多条记忆合并为高层认知)
性能特征:
  - 向量检索: <5ms (百万级向量, HNSW)
  - Qdrant 内存映射模式: 磁盘索引 + 热点缓存
```

---

## 五、技术栈选型

### 5.1 前端

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| 框架 | React 18 + TypeScript | 生态成熟，类型安全 |
| 3D 渲染 | Three.js + React Three Fiber | WebGL 标准，社区活跃 |
| 虚拟形象 | VRM (VRoid Standard) | 开放标准，丰富的形象资源 |
| 状态管理 | Zustand | 轻量，适合实时应用 |
| UI 组件 | Ant Design 5 / Radix UI | 企业级 UI，可定制性强 |
| 实时通信 | Socket.IO Client | 双向实时通信 |
| 语音 | Web Speech API + Whisper.js | 浏览器原生 + 离线能力 |
| 构建工具 | Vite | 快速 HMR，原生 ESM |

### 5.2 后端 (Rust)

> **核心决策：后端全面采用 Rust**。相比 Python，Rust 在内存占用上降低 5-10 倍，CPU 利用率提升 3-5 倍，且无 GC 停顿。AI/ML 推理通过 HTTP 调用外部服务或本地独立进程，后端本身保持轻量。

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| Web 框架 | **Axum 0.7** | Tokio 生态，零开销抽象，类型安全的路由和中间件 |
| 异步运行时 | **Tokio 1.x** | Rust 异步标准，百万级并发，极低内存开销 |
| 数据库 ORM | **sqlx** | 编译期 SQL 检查，async 原生，零运行时反射 |
| HTTP 客户端 | **reqwest** | 调用 LLM API、第三方服务，支持连接池和流式响应 |
| WebSocket | **tokio-tungstenite** | 高性能 WebSocket，配合 Axum 使用 |
| 任务队列 | **自研轻量队列** (Redis Streams + Tokio workers) | 替代 Celery，内存占用从 ~200MB 降到 ~5MB |
| 定时任务 | **tokio-cron-scheduler** | 轻量级，无额外进程 |
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
│  │ 对话引擎  │  │ PM 引擎   │  │ 集成引擎  │      │
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
| 主 LLM | GPT-4o / Claude 3.5 / Qwen-Max | 云端 API | 按需调用，零本地资源 |
| 本地 LLM | Qwen2.5-7B / Llama 3.1-8B | **Ollama** 独立进程 | CPU 推理，与后端进程隔离 |
| STT | Whisper Large-v3 | **faster-whisper** 独立服务 | GPU 加速，HTTP API |
| TTS | Edge TTS / CosyVoice | 独立微服务 | 流式合成 |
| 向量模型 | BGE-M3 | **TEI** (Text Embeddings Inference) | HuggingFace 官方 Rust 推理服务 |
| OCR | Surya / PaddleOCR | 独立微服务 | 按需启动 |
| 情感分析 | Fine-tuned RoBERTa | TEI 共享服务 | 多模型共用推理服务 |
| LLM 路由 | **自研 Rust 路由器** | 内嵌后端 | 基于意图和复杂度自动选择模型 |

### 5.4 数据存储

| 组件 | 技术选型 | 理由 |
|------|---------|------|
| 主数据库 | PostgreSQL 16 | 可靠，JSON 支持好 |
| 缓存/队列 | Redis 7 | 高速缓存 + Redis Streams 任务队列 |
| 向量数据库 | **Qdrant** | Rust 实现，性能最优，与后端技术栈一致 |
| 图数据库 | **Apache AGE** (PostgreSQL 扩展) | 图查询能力内嵌 PostgreSQL，免部署 Neo4j |
| 文件存储 | MinIO (兼容 S3) | 本地化对象存储 |
| 全文搜索 | **Meilisearch** | Rust 实现，轻量高性能，中文分词支持好 |

> **关键优化：用 Apache AGE 替代 Neo4j**。AGE 是 PostgreSQL 的图数据库扩展，知识图谱数据直接存在 PostgreSQL 中，省掉一个独立数据库进程，内存节省 ~300MB。

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

> **总工期：36 周 (约 9 个月)**。每 Phase 预留 20% 缓冲时间。v1.0 聚焦核心能力，知识图谱、风险预测、报表自动生成放到 v2.0。

### Phase 0: 基础设施 + 技术验证 (第 1-3 周)
- [ ] 项目脚手架搭建 (Rust Axum + React + Three.js)
- [ ] Docker 开发环境配置 (docker-compose 一键启动)
- [ ] CI/CD 流水线搭建 (GitHub Actions: build → test → lint → docker)
- [ ] 数据库 schema 设计 (PostgreSQL + sqlx 迁移脚本)
- [ ] API 规范定义 (utoipa 自动生成 OpenAPI)
- [ ] **技术验证 PoC**：
  - [ ] Apache AGE 图查询 PoC（验证 3 个核心查询的性能和语法支持）
  - [ ] Rust LLM 调用 PoC（验证 reqwest 流式转发 + Ollama 集成）
  - [ ] Qdrant 向量检索 PoC（验证 HNSW 检索延迟 < 5ms）
  - [ ] 如 PoC 不通过，及时调整技术选型

### Phase 1: 核心对话 (第 4-8 周)
- [ ] LLM 接入层 (reqwest 调用 GPT-4/Claude/Ollama，流式 SSE 转发)
- [ ] Rust 端 LLM 编排 (Prompt 模板管理 + 工具调用 + 上下文窗口管理)
- [ ] 多轮对话管理 (滑动窗口 + 摘要压缩)
- [ ] 对话界面 (React Chat UI + 流式渲染)
- [ ] 用户认证与权限系统 (JWT + RBAC)
- [ ] 基础记忆系统 (Redis 会话级)
- [ ] 前后端类型契约 (utoipa → openapi-typescript-codegen 自动生成)

### Phase 2: 虚拟形象 (第 9-14 周)
- [ ] 3D 虚拟形象渲染引擎 (Three.js + React Three Fiber + VRM)
- [ ] 口型同步系统 (音素映射 → BlendShape 驱动，需 3-4 周打磨)
- [ ] 基础表情状态机 (情绪 → 表情映射)
- [ ] TTS 语音合成集成 (Edge TTS 流式合成 → 音频驱动口型)
- [ ] 形象自定义系统 (发型/服装/配饰)
- [ ] 三级渲染策略实现 (Level 1/2/3 自动切换)
- [ ] 性能优化 (LOD、纹理压缩、空闲降级)

### Phase 3a: PM 核心集成 (第 15-19 周)
- [ ] Jira 集成 (OAuth + 任务同步 + Webhook 实时更新)
- [ ] 飞书集成 (OAuth + 项目/任务/日历同步)
- [ ] 项目健康度计算引擎 (进度偏差/成本偏差/负载均衡度)
- [ ] 通知引擎 (桌面通知 + 邮件)

### Phase 3b: PM 高级能力 (第 20-24 周)
- [ ] 会议助手功能 (录音转录 + 纪要生成 + Action Item 提取)
- [ ] 调度引擎 (定时任务 + 事件触发 + 主动推送)
- [ ] Asana / Linear 集成 (可选，视用户需求优先级)
- [ ] IM Bot 集成 (飞书/钉钉 Bot 消息推送)

### Phase 4: 记忆与学习 (第 25-29 周)
- [ ] 长期记忆系统 (Qdrant 向量存储 + 记忆检索/遗忘机制)
- [ ] 基础知识图谱 (Apache AGE 或 Neo4j，Phase 0 PoC 结果决定)
- [ ] 主动智能 (晨会推送/会议提醒/逾期预警/周五周报)
- [ ] 报表生成 (基础模板: 周报/月报/状态报告)
- [ ] 用户反馈系统 (👍/👎 + 纠正 → 记忆权重调整)

### Phase 5: 打磨与发布 (第 30-36 周)
- [ ] 性能优化 (3D 渲染 + API 响应 + 缓存策略调优)
- [ ] 安全审计 (Prompt 注入防护 + API Key 管理 + 数据脱敏)
- [ ] 集成测试 + E2E 测试
- [ ] 用户测试与反馈迭代 (邀请 5-10 位 PM 内测)
- [ ] 文档编写 (用户手册 + API 文档 + 部署指南)
- [ ] v1.0 发布

### v2.0 规划 (v1.0 发布后)
- 知识图谱自动构建 (LLM 实体提取 + 人工确认)
- 风险预测模型 (基于历史数据的时序分析)
- 高级报表 (PPT/Word 自动生成)
- 更多第三方集成 (Teams, Linear, Confluence...)
- 移动端 App (React Native / PWA)
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
                    │ PM 智能引擎   │
                    │ (分析+推理)   │
                    └──────┬───────┘
                           │
                    ┌──────┴───────┐
                    │              │
                    ▼              ▼
            ┌──────────┐   ┌──────────┐
            │ 工具调用  │   │ 直接回复  │
            │ (API/DB/ │   │          │
            │  文件系统)│   │          │
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
                 │ 记忆更新      │
                 │ (存储新知识)  │
                 └──────────────┘
```

### 7.2 主动监控流程

```
┌──────────────┐
│ 定时触发器    │ (每 N 分钟)
│ 事件触发器    │ (Webhook/消息队列)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ 数据采集      │ (拉取最新项目数据)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ 指标计算      │ (健康度/风险值/偏差)
└──────┬───────┘
       │
       ▼
┌──────────────┐     ┌──────────────┐
│ 阈值判断      │────▶│ 触发通知      │ (桌面/邮件/IM)
│ + LLM 分析   │     │ + 虚拟形象    │ (表情变化+语音提醒)
└──────────────┘     └──────────────┘
```

---

## 八、安全与隐私

### 8.1 数据安全
- 所有敏感数据加密存储 (AES-256-GCM)
- API 通信全程 TLS 1.3
- 支持数据本地化部署，敏感数据不出内网
- LLM 调用支持脱敏处理 (PII 自动检测与替换)
- 数据库连接使用 SSL，禁止明文传输
- 文件上传做病毒扫描和格式校验

### 8.2 LLM 安全 (关键新增)
- **Prompt 注入防护**：
  - 用户输入与系统 Prompt 严格分离，用户内容包裹在特定标记中
  - 输出过滤器：检测 LLM 是否泄露了系统 Prompt 或执行了非预期操作
  - 输入长度限制 + 频率限制，防止资源耗尽攻击
- **API Key 管理**：
  - 所有 LLM API Key 存储在加密的环境变量或 Vault 中，不写入代码或配置文件
  - 支持 Key 轮换：旧 Key 立即失效，无需重启服务
  - 泄露应急：检测到异常调用量时自动禁用 Key 并告警
- **LLM 输出安全**：
  - 代码执行（如果支持）在沙箱中运行，限制文件系统和网络访问
  - 生成的链接/文件路径做白名单校验，防止路径遍历攻击

### 8.3 权限控制
- 基于 RBAC 的细粒度权限管理 (Admin / PM / Member / Viewer)
- 项目级数据隔离：项目 A 的管理员无法访问项目 B 的数据
- 操作审计日志：所有写操作记录 who/what/when/where
- 敏感操作二次确认（删除项目、修改权限、导出数据）
- API 限流：每用户 60 请求/分钟，LLM 调用 20 请求/分钟

### 8.4 第三方集成安全
- OAuth Token 加密存储，支持自动刷新和手动撤销
- Webhook 签名验证（HMAC-SHA256），防止伪造请求
- 集成权限最小化：只申请必要的 API Scope
- Token 过期自动降级为只读模式，通知用户重新授权

### 8.5 合规
- 符合 GDPR 数据保护要求
- 支持数据导出 (JSON/CSV) 与删除 (被遗忘权)
- AI 决策可解释性：提供推理过程和数据来源
- 中国用户：符合《个人信息保护法》，数据存储可选境内节点

---

## 八-2、错误处理与降级策略

### 故障场景矩阵

| 故障场景 | 影响 | 降级策略 | 用户提示 |
|---------|------|---------|---------|
| **LLM API 超时 (>30s)** | 对话无响应 | 自动重试 1 次 → 切换备用模型 → 返回缓存相似回答 | "AI 思考中...已切换到备用模型" |
| **LLM API 返回错误 (429/500)** | 对话不可用 | 队列排队 + 指数退避重试 → 降级为本地小模型 | "云端 AI 暂时繁忙，已切换到本地模式" |
| **第三方平台 API 限流** | 数据同步延迟 | 队列缓存 + 批量合并请求 + 指数退避 | "数据同步延迟，预计 X 分钟后恢复" |
| **第三方平台 Token 过期** | 集成功能中断 | 自动刷新 Token → 失败则降级为只读 → 通知用户 | "飞书连接已过期，请重新授权" |
| **Qdrant 不可用** | 记忆检索失败 | 降级为 PostgreSQL 全文搜索 (精度降低) | "记忆搜索暂不可用，已切换到基础搜索" |
| **PostgreSQL 主从切换** | 短暂不可写 (~5s) | 连接池自动重连 + 写操作排队 | 用户无感知 (自动恢复) |
| **Redis 不可用** | 缓存失效 | 直接查数据库 (延迟增加 ~50ms) + 本地 L1 缓存兜底 | 用户无感知 (性能略降) |
| **WebSocket 断连** | 实时通信中断 | 自动重连 (指数退避 1s→2s→4s→...→30s) + 消息补发 | "连接已恢复" (重连后提示) |
| **STT 服务不可用** | 语音输入不可用 | 降级为纯文字输入 | "语音识别暂不可用，请使用文字输入" |
| **TTS 服务不可用** | 语音播放不可用 | 降级为纯文字输出 | 用户无感知 (无语音) |
| **3D 渲染异常** | 虚拟形象不显示 | 自动降级为 2D 精灵图 → 纯文字模式 | "已切换到简约模式" |
| **磁盘空间不足** | 写入失败 | 清理过期缓存/日志 → 告警管理员 | "系统存储空间不足，请联系管理员" |

---

## 八-3、数据备份与灾难恢复

### 备份策略

| 数据组件 | 备份方式 | 频率 | 保留周期 | 存储位置 |
|---------|---------|------|---------|---------|
| PostgreSQL | pg_basebackup 全量 + WAL 归档 | 每日全量 + 持续 WAL | 30 天 | 异地 S3/MinIO |
| Qdrant | Collection Snapshot API | 每 6 小时 | 7 天 | 异地 S3/MinIO |
| Redis | RDB 快照 + AOF 持久化 | RDB 每小时 + AOF 持续 | 7 天 | 本地 + 异地 |
| 文件存储 (MinIO) | 跨节点复制 / 异地同步 | 实时 | 永久 | 异地 MinIO |

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
6. MinIO: 从异地副本同步
7. 验证数据完整性 → 恢复服务 → 通知用户
```

### 监控告警阈值

| 指标 | 警告阈值 | 严重阈值 | 通知方式 |
|------|---------|---------|---------|
| API P95 延迟 | > 500ms 持续 5 分钟 | > 2s 持续 1 分钟 | 警告: 飞书/Slack, 严重: 电话 |
| 后端内存 (纯云端) | > 100MB | > 200MB | 飞书/Slack |
| LLM 调用错误率 | > 10% 持续 5 分钟 | > 30% 持续 1 分钟 | 严重: 电话 |
| 数据库连接池使用率 | > 80% | > 95% | 飞书/Slack |
| 磁盘使用率 | > 85% | > 95% | 警告: 飞书, 严重: 电话 |
| 备份失败 | 连续 2 次失败 | 连续 4 次失败 | 飞书/Slack |
| WebSocket 连接数 | > 500/实例 | > 1000/实例 | 飞书/Slack |

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
- 行业专属 PM 模板 (IT/建筑/制造)
- AI 模型算力包
- 培训与咨询服务

### 11.3 成本模型 (v1.0 单用户)

| 成本项 | 月度成本 | 说明 |
|--------|---------|------|
| LLM API 调用 (GPT-4o) | ¥100-300 | 轻度用户 ~¥100，重度 ~¥300 |
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
| Apache AGE 功能不足 | 图查询受限 | 中 | Phase 0 PoC 验证，不通过则回退 Neo4j Community |

---

## 十三、核心 API 规范

> 完整 API 由 utoipa 自动生成 OpenAPI 文档，以下为核心端点概览。

### 对话 API

```
POST   /api/chat                    — 发送消息 (SSE 流式响应)
POST   /api/chat/sessions           — 创建新会话
GET    /api/chat/sessions            — 获取会话列表
GET    /api/chat/sessions/:id        — 获取会话历史
DELETE /api/chat/sessions/:id        — 删除会话
WS     /ws/chat                      — WebSocket 实时对话
```

### 项目管理 API

```
GET    /api/projects                  — 获取项目列表
POST   /api/projects                  — 创建项目
GET    /api/projects/:id              — 获取项目详情
GET    /api/projects/:id/health       — 获取项目健康度
GET    /api/projects/:id/tasks        — 获取任务列表
POST   /api/projects/:id/tasks        — 创建任务
PATCH  /api/projects/:id/tasks/:tid   — 更新任务
```

### 会议 API

```
POST   /api/meetings/transcribe       — 上传会议录音 (返回转录结果)
GET    /api/meetings                   — 获取会议列表
GET    /api/meetings/:id/minutes      — 获取会议纪要
POST   /api/meetings/:id/actions      — 创建 Action Item
```

### 记忆与知识 API

```
GET    /api/memories/search?q=...     — 搜索记忆 (语义检索)
GET    /api/memories                   — 浏览记忆列表
DELETE /api/memories/:id              — 删除记忆
POST   /api/memories/feedback         — 提交反馈 (👍/👎 + 纠正)
```

### 集成 API

```
GET    /api/integrations              — 获取已连接的平台列表
POST   /api/integrations/:platform/connect  — OAuth 授权连接
DELETE /api/integrations/:platform    — 断开连接
POST   /api/integrations/:platform/sync     — 手动触发同步
```

### 报表 API

```
POST   /api/reports/generate          — 生成报告 (周报/月报/自定义)
GET    /api/reports                    — 获取报告列表
GET    /api/reports/:id               — 获取报告详情
GET    /api/reports/:id/download      — 下载报告 (PDF/Word)
```

### 系统 API

```
GET    /api/system/health             — 健康检查
GET    /api/system/metrics            — Prometheus 指标
GET    /api/system/config             — 获取系统配置 (管理员)
PATCH  /api/system/config             — 更新系统配置 (管理员)
```

---

## 十四、测试策略

### 测试金字塔

```
          ┌───────────┐
          │  E2E 测试  │  ← 5-10 个核心用户流程
          │  (Playwright)│     对话→任务创建→报表生成
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
| E2E 测试 | Playwright | 5-10 核心流程 | 对话→任务创建→报表生成完整链路 |
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

3. **测试环境**：每次 CI 自动执行全部迁移 + 回滚测试

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
  │  CI 契约测试:                         │
  │  验证实际响应符合 schema              │
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
    path = "/api/chat",
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

### CI 契约测试

```yaml
# GitHub Actions: 每次 PR 验证前后端契约一致
- name: Generate OpenAPI spec
  run: cargo test generate_openapi  # utoipa 生成 openapi.json
- name: Regenerate TypeScript client
  run: npx openapi-typescript-codegen -i openapi.json -o frontend/src/generated
- name: Check for breaking changes
  run: npx openapi-diff openapi.json frontend/src/generated/openapi.json
```

---

## 十七、总结

灵枢 (LingShu) 不是又一个"套壳 ChatGPT"，而是一个**以项目管理为核心场景、以虚拟形象为情感载体、以主动智能为差异化**的全新品类产品。

它从三个维度重新定义 PM 助手：

1. **有温度** — 3D 虚拟形象让 AI 不再冰冷，情绪表达让交互更自然
2. **有深度** — 不是简单的问答，而是真正理解 PM 方法论和项目上下文
3. **有主动性** — 不等你问，主动发现风险、推送洞察、提醒关注

正如 Open Interpreter 让 LLM 接管了代码执行，Khoj 让 AI 成为第二大脑，JARVIS 让 LLM 调度专家模型——灵枢要让 AI 成为项目经理真正信赖的"数字搭档"。

---

## 附录 A：高性能架构优化方案 (Performance Annex)

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

  // 项目看板 — 路由触发时加载
  '/dashboard':   lazy(() => import('./Dashboard')),

  // 报表生成 — 按需加载
  '/reports':     lazy(() => import('./ReportBuilder')),

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
        .route("/api/chat", post(chat_handler))
        .route("/api/projects", get(list_projects))
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
            Intent::RiskAnalysis => ModelConfig::new("claude-3.5", 4096),
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
│  P1 (高)   ──▶ 会议纪要生成 / 风险预警               │
│  P2 (中)   ──▶ 报表生成 / 知识图谱更新               │
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

#### A.4.3 知识图谱优化 (PostgreSQL + Apache AGE)

> **风险提示**：Apache AGE 是 PostgreSQL 的图数据库扩展，相比 Neo4j 有以下局限：Cypher 语法不完整（不支持 EXISTS 子查询、部分聚合函数）、复杂多跳查询（3+ 跳）性能不如 Neo4j、社区文档较少。建议在 Phase 0 中做 PoC 验证，如不满足则回退到 Neo4j Community Edition。

```sql
-- Apache AGE 图查询 (PostgreSQL 扩展)
-- 需要先加载扩展: LOAD 'age';
-- 设置搜索路径: SET search_path = ag_catalog, "$user", public;

-- 1. 创建图
SELECT create_graph('project_knowledge');

-- 2. 创建实体和关系 (通过 Rust sqlx 执行)
-- INSERT INTO project_knowledge."Person" (name, properties) ...
-- INSERT INTO project_knowledge."Task" (title, status, properties) ...

-- 3. 查询：某人的所有任务 (1 跳)
SELECT * FROM cypher('project_knowledge', $$
    MATCH (p:Person {name: $name})-[:负责]->(t:Task)
    RETURN t.title, t.status
$$) AS (title agtype, status agtype);

-- 4. 查询：依赖链分析 (2 跳，限制深度)
SELECT * FROM cypher('project_knowledge', $$
    MATCH (t:Task {id: $task_id})-[:依赖*1..2]->(dep:Task)
    RETURN dep.title, dep.status, dep.assignee
    LIMIT 20
$$) AS (title agtype, status agtype, assignee agtype);

-- 5. 性能优化：为高频查询创建物化视图
CREATE MATERIALIZED VIEW mv_task_dependency_graph AS
SELECT * FROM cypher('project_knowledge', $$
    MATCH (t:Task)-[:依赖]->(dep:Task)
    RETURN t.id, t.title, dep.id, dep.title
$$) AS (t_id agtype, t_title agtype, dep_id agtype, dep_title agtype);

-- 定期刷新 (每 30 分钟)
-- REFRESH MATERIALIZED VIEW CONCURRENTLY mv_task_dependency_graph;
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
    .route("/api/chat", post(chat_handler))
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
PARAMETER num_ctx 4096              # 上下文窗口，4K tokens 足够 PM 对话
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
  model: "small"              # small 模型 (~461MB) 足够 PM 场景
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

LLM 调用走云端 API（GPT-4o/Claude/Qwen-Max），本地不部署任何 AI 模型。适合个人开发者和小团队快速上手。

**单用户部署：**

| 组件 | CPU | 内存 | 磁盘 | 说明 |
|------|-----|------|------|------|
| PostgreSQL + AGE | 0.25 核 | 256MB | 2GB | 图查询内嵌 |
| Redis | 0.1 核 | 64MB | 128MB | 缓存 + Streams 队列 |
| Qdrant | 0.15 核 | 128MB | 512MB | 内存映射模式 |
| **Axum 后端** | **0.15 核** | **30MB** | **20MB** | **静态编译, 单二进制** |
| 前端 (Nginx) | 0.05 核 | 32MB | 50MB | 静态资源 |
| **合计** | **~0.7 核** | **~510MB** | **~3.2GB** | **Docker Compose** |

**团队部署 (10-50 人)：**

| 组件 | CPU | 内存 | 磁盘 | 说明 |
|------|-----|------|------|------|
| PostgreSQL + AGE | 2 核 | 2GB | 50GB | 主从复制 |
| Redis | 0.5 核 | 512MB | 2GB | Cluster 模式 |
| Qdrant | 1 核 | 1GB | 10GB | 分布式, 内存映射 |
| Axum 后端 (x3) | 1.5 核 | 90MB | 60MB | 多实例, 每实例 ~30MB |
| Meilisearch | 0.5 核 | 256MB | 2GB | 全文搜索 |
| **合计** | **~5.5 核** | **~4GB** | **~64GB** | **Docker Compose / K8s** |

> **优势**：内存极低（单用户 510MB），无需 GPU，LLM 能力随云端模型升级自动提升。
> **成本**：LLM API 调用费（GPT-4o 约 ¥0.07/1K tokens，重度用户约 ¥100-300/月）。

---

#### 模式 B：本地推理（隐私敏感 / 离线场景）

本地部署 LLM（Ollama）+ STT（faster-whisper）+ 向量模型（TEI）。数据不出内网，但需要更多硬件资源。

**单用户部署：**

| 组件 | CPU | 内存 | 磁盘 | GPU | 说明 |
|------|-----|------|------|-----|------|
| PostgreSQL + AGE | 0.25 核 | 256MB | 2GB | - | 图查询内嵌 |
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
| PostgreSQL + AGE | 2 核 | 2GB | 50GB | 主从复制 |
| Redis | 0.5 核 | 512MB | 2GB | Cluster |
| Qdrant | 1 核 | 1GB | 10GB | 分布式 |
| Axum 后端 (x3) | 1.5 核 | 90MB | 60MB | 多实例 |
| Meilisearch | 0.5 核 | 256MB | 2GB | 全文搜索 |
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
