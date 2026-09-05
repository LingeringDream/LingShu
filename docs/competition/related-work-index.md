# 相关研究与前沿技术索引（Related Work Index）

> 用途：本文件索引灵枢 · Memory Evolution Lab（中兴 1529 命题）相关的赛题解读、国际前沿技术与本会话产出的研究报告，供竞赛文档写作、实验设计与评审答辩时快速取用。
>
> 维护约定：只收录有可追溯来源（arXiv / 官网 / 官方文档）的条目；论文数据均为作者自报，跨论文横向比较仅供参考，引用时需标注口径。

## 0. 研究报告（会话产出，正文内联交付）

| 编号 | 主题 | 核心内容 | 状态 |
|---|---|---|---|
| R1 | WikiSkill 关联研究（arXiv 2608.27454） | 三层架构（Raw/Wiki/Skill）与灵枢三层记忆同构；持久知识消融 +15.0pt（48.7→63.7）；知识永不回滚/规则可回滚；跨模型负迁移案例；被拒修改是可复用知识；执行 Agent 访问边界影响诊断价值 | 正文已在会话内交付；未落盘，可按需再生成 |
| R2 | 赛题本体解读 + 国际前沿技术图谱 | 赛题三大方向、五项量化指标、加分项；MemGPT/Mem0/Zep/A-MEM/MemoryBank/FadeMem/Collaborative Memory；LoCoMo/LongMemEval 评测；中兴「连接+算力」战略契合 | 已被 R3 扩展覆盖，保留备查 |
| R3 | 评测方法与基准参考（24 来源完整版） | 在 R2 基础上补充：十一基准矩阵（LoCoMo/LongMemEval 主测；MSC/DMR、BEAM、MemoryAgentBench、PerLTQA、PersonaMem 互补）；MemDelta 受控评测协议警示；五项指标→基准映射表；评分器与对照实验设计；「记忆治理」转型洞察 | 正文已在会话内交付；未落盘，可按需再生成 |
| R4 | [正式相关工作](related-work.md) | 分层记忆、访问控制、遗忘、冲突治理和经验演化；区分论文思想、灵枢实现与未完成边界 | 已落盘并随功能同步维护 |

## 1. 赛题本体（来源：全国大学生创业服务网）

- 命题：[产业赛道 · 中兴通讯 1529「面向智能体自主进化的动态记忆沉淀与自适应检索系统」](https://cy.ncss.cn/mtcontest/detail?id=2c93f4c6a00f063201a03d78307316e2)
- 三大研究内容：① 分层异构情景记忆（episode/fact/experience，时序索引+场景标签）；② 多 Agent 记忆网关与私有/公共分级权限隔离；③ 融合时间衰减、来源可信度、交互频次的冲突识别与更新（过期淘汰、矛盾修正、高价值优先召回）。
- 硬性要求：可运行原型（禁纯理论仿真）；50 轮以上长时序连续交互测试；可视化、参数可配置、与传统 RAG 可对比；交付含源代码、部署手册、架构图、创新点说明、标准化测试数据集、对比实验报告与 3–5 分钟演示视频。
- 五项量化指标（详见 R2/R3）：长时序记忆准确率 ≥86% 且较静态 RAG 提升 15pt+；Top-K 检索精准率 ≥83%；冗余/过期记忆过滤率 ≥45%；开放域任务完成度提升 ≥12%；Token 消耗降低 ≥35%。加分项：端侧部署，单次检索时延 ≤600ms、内存占用降低 ≥25%。

## 2. 技术条目索引（按赛题方向分组）

### 2.1 方向①：分层异构记忆

| ID | 技术 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T1 | MemGPT | [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) | 虚拟上下文管理、操作系统式分层内存；灵枢分层+回指链已覆盖 | 引用（related work） |
| T2 | Zep / Graphiti | [arXiv:2501.13956](https://arxiv.org/abs/2501.13956) | 时序感知知识图谱，直接呼应「时序索引、解决时序混乱」；⚠️ 项目约束不引入图数据库，仅借鉴时序思路 | 可选增强（受限） |
| T3 | A-MEM | [arXiv:2502.12110](https://arxiv.org/abs/2502.12110)（NeurIPS 2025，[代码](https://github.com/agiresearch/a-mem)） | Zettelkasten 动态索引，「新记忆触发历史记忆上下文更新」可补进 experience 合并路径 | 可选增强 |
| T4 | 《Memory in the Age of AI Agents》综述 | [arXiv:2512.13564](https://arxiv.org/abs/2512.13564) | 记忆形式/功能/动态统一建模；明确多 Agent 记忆、记忆自动化是前沿方向 | 引用 |

### 2.2 方向②：多智能体记忆共享与权限隔离

| ID | 技术 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T5 | Collaborative Memory | [arXiv:2505.18279](https://arxiv.org/abs/2505.18279) | 二分图动态访问控制、私有/共享两层、不可变来源属性、可证明动态权限；与 Memory Gateway 几乎一一对应，灵枢「数据库后置校验」是更强的安全设计 | 对标（可写进对比） |
| T6 | 工业多 Agent 记忆方案横评 | [Mem0 vs Letta vs Zep](https://plur.ai/blog/mem0-vs-letta-vs-zep/) · [2026 记忆方案横评](https://www.developersdigest.tech/blog/best-ai-agent-memory-providers-2026) | 生产化多 Agent 共享/隔离设计模式参考 | 参考 |

### 2.3 方向③：冲突消解与遗忘机制

| ID | 技术 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T7 | MemoryBank | [arXiv:2305.10250](https://arxiv.org/abs/2305.10250)（AAAI） | 首次将艾宾浩斯遗忘曲线引入 LLM 长期记忆；灵枢多因子衰减+来源护栏更细 | 引用 |
| T8 | FadeMem | [arXiv:2601.18642](https://arxiv.org/html/2601.18642v1) | 双层衰减 + LLM 四类关系冲突消解（相容/矛盾/包含/被包含）；LoCoMo 多跳 F1 29.43、冲突宏准确率 68.9%、存储削减 45%；与灵枢 challenger/incumbent/disputed 同题 | 对标（消融对照首选） |

### 2.4 评测基准（对应赛题指标验证）

| ID | 基准 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T9 | LoCoMo | [arXiv:2402.17753](https://arxiv.org/abs/2402.17753) | 平均 300 轮超长多会话；补「公开长记忆数据集」证据边界①；指标(1)长时序准确率首选载体 | 落地候选（主测） |
| T10 | LongMemEval | [arXiv:2410.10813](https://arxiv.org/abs/2410.10813) | 500 问、含时序推理/知识更新两维，对应赛题指标①②测量口径 | 落地候选（主测） |
| T14 | MSC / DMR | [MemGPT 论文](https://arxiv.org/abs/2310.08560) · [DMR 复现管道](https://deepwiki.com/getzep/zep-papers/3.2-dmr-evaluation-pipeline) | 三阶段管道（上下文检索→答案生成→LLM 评分）；MemGPT 92.5%–93.4% 对固定上下文 32%–39%；可作指标(2) Top-K 参照协议 | 落地候选（参照协议） |
| T15 | BEAM / LIGHT | [arXiv:2510.27246](https://arxiv.org/abs/2510.27246) | 百万 token 级、100 对话+2000 探测；LIGHT 框架较最强基线 +3.5%–12.69%；规模增大时所有方案均退化 | 参考（扩展性验证） |
| T16 | MemoryAgentBench | [arXiv:2507.05257](https://arxiv.org/abs/2507.05257) | 四种核心能力（准确检索/测试时学习/长程理解/选择性遗忘）；「选择性遗忘」子任务对应指标(3)过滤率口径 | 落地候选（指标③互补） |
| T17 | PerLTQA | [arXiv:2402.16288](https://arxiv.org/abs/2402.16288) | 个人长期记忆，语义/情景二分（8593 问、30 角色）呼应灵枢 fact/experience 分层设计 | 引用/参考 |
| T18 | PersonaMem | [Cognee 基准指南](https://www.cognee.ai/ai-memory-benchmarks) | 全合成个性化记忆；v2 专测隐式个性化推断；⚠️ 新基准未同行评议 | 参考（待复核） |

### 2.5 技能/知识演化线（赛题「经验沉淀复用技能」主干，需重点补强）

| ID | 技术 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T11 | WikiSkill | [arXiv:2608.27454](https://arxiv.org/pdf/2608.27454) | 三层架构与灵枢同构；知识永不回滚/规则可回滚；被拒修改是可复用知识（skill-impact）；执行 Agent 访问边界影响诊断价值；无 pruning（灵枢遗忘衰减可反超） | 对标+演进参考（见 R1 详细分析） |

### 2.6 评测方法学（如何测，而非测什么）

| ID | 技术 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T19 | MemDelta | [arXiv:2606.29914](https://arxiv.org/abs/2606.29914) | 受控评测协议：仅换嵌入即偏移 +6.2pp（p=0.004）；RAG 47.2% vs 全上下文 49.8%（p=0.34）且跨模型排名反转；智能体自我记忆 42% < 基础检索 47%；三原则=固定嵌入/按模型家族分层/先报写路径成本 | 对标（评测协议必读） |
| T20 | From Recall to Forgetting | [arXiv:2604.20006](https://arxiv.org/html/2604.20006v1) | 把「遗忘与更新」纳入正式评测维度，呼应赛题「动态筛选淘汰」核心功能；⚠️ 新文献未同行评议沉淀 | 参考（待复核） |

### 2.7 生产工程 / 其他

| ID | 技术 | 来源 | 一句话关联与落点 | 状态 |
|---|---|---|---|---|
| T12 | Mem0 | [arXiv:2504.19413](https://arxiv.org/abs/2504.19413) | 生产级记忆层（抽取/巩固/检索/图记忆）；自报 Token 省 90%+、p95 延迟降 91%，说明指标(5) 35% 门槛保守可达；⚠️ Apache-2.0，注意自主知识产权 | 参考 |
| T13 | LLM-based Agents 记忆机制综述（ACM） | [ACM 3748302](https://dl.acm.org/doi/10.1145/3748302) | 写入-管理-读取全流程系统综述 | 引用 |

## 3. 与灵枢现有模块的对应关系（速查）

| 灵枢模块 | 直接对标/参考技术 |
|---|---|
| 三层异构记忆（episode/fact/experience） | MemGPT、A-MEM、WikiSkill（Raw/Wiki/Skill）；PerLTQA（语义-情景二分） |
| 时序索引与自适应检索 | Zep/Graphiti（时序图谱，受限） |
| Memory Gateway（私有/定向/全局 + 审计） | Collaborative Memory |
| 冲突治理（challenger/incumbent/disputed） | FadeMem（四类关系） |
| 遗忘衰减 + 来源护栏 | MemoryBank（艾宾浩斯）、FadeMem（双层衰减）；WikiSkill 无 pruning 为差异化点 |
| 自进化（Evolution Engine / evolution_runs） | WikiSkill（技能/知识演化线） |
| 评测（长时序/检索/过滤指标） | LoCoMo、LongMemEval（主测）；MSC/DMR（Top-K 协议）；MemoryAgentBench（选择性遗忘） |
| 评测协议 | MemDelta（受控协议三原则，避免「虚假提升」） |

## 4. 优先级与落地清单

1. **P0 · 评测补强（不改架构）**：接入 LoCoMo / LongMemEval 为主测，MSC/DMR 作 Top-K 参照协议、MemoryAgentBench「选择性遗忘」补指标③口径，把自建 60 轮回归集升级为公开基准对照，补齐实验报告「证据边界①」。对应 T9、T10、T14、T16。
2. **P0 · 评测协议自检**：按 MemDelta 三原则执行——对比间固定嵌入模型、按模型家族分层报告、先报写入路径成本再谈架构收益；相对指标同时报绝对值与显著性检验。对应 T19。
3. **P1 · 冲突治理对标**：以 FadeMem 为消融/对比对象，统一基准与评测脚本（不自报数对自报数）。对应 T8。
4. **P1 · 权限隔离论证**：引用 Collaborative Memory，突出灵枢「向量不作权限边界、数据库后置校验」的差异点。对应 T5。
5. **P2 · 技能/知识演化线**：按 R1 分析，落地「裁决记录读回 + 经验来源标注」两项补强，推进自进化设计。对应 T11。
6. **P2 · 可选增强**：A-MEM 的「新记忆触发旧记忆更新」补进 experience 合并（T3）；Zep 时序思路仅轻量借鉴、不引入图数据库（T2）。
7. **引用层**：MemGPT / MemoryBank / 两篇综述 / PerLTQA / 赛题本体，用于 related work 与创新点文献支撑（T1、T4、T7、T13、T17、命题页）。

## 5. 引用与口径说明

- 论文指标均为各文自报，评测设置不完全一致（如 DMR 评分器版本、LoCoMo 题量与过滤方式），横向比较仅供参考；正式对照需在同一评测脚本下重跑。
- 评测结果必须遵守受控协议（固定嵌入/分层报告/显著性检验），避免 MemDelta 指出的「换嵌入即 +6.2pp 虚假提升」类混杂（T19）。
- PersonaMem（T18）、From Recall to Forgetting（T20）为较新文献，尚未经同行评议沉淀，采用时建议复核；MemDelta 部分题型样本量小（n=88），其结论不宜外推（T19）。
- 开源组件（A-MEM、Graphiti、Mem0 等）使用时核对许可协议，满足赛题「自主知识产权」要求。
- 赛题报名状态、截止时间以全国大学生创业服务网实时公告为准。
