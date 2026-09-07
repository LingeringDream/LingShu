# LingShu Cloud

独立的 LingShu 云服务 Worker。它负责云账户、设备会话、订阅权益、端到端加密同步和团队工作区，
但不替换现有 `lingshu-server` 的 PostgreSQL 用户表或 `local-session`，因此桌面端离线能力保持不变。

## 远端环境

- 生产服务首页：<https://account.lingeringdream.com>
- 生产健康检查：<https://account.lingeringdream.com/health>
- 兼容入口：<https://lingshu-cloud.mingmofushenglu.workers.dev>
- D1：`lingshu-cloud-dev`（APAC）
- R2：`lingshu-memory-dev`，绑定名为 `MEMORY_BUCKET`

远端已设置独立 `JWT_SECRET` 和 `RESEND_API_KEY`；Secret 值不得写入仓库。

## 网页模块

账户网页采用独立路径，而不是在一个长页面中滚动切换：

- `/account`：账户概览
- `/billing`：订阅与账单
- `/devices`：设备与会话
- `/docs`：使用文档
- `/download`：下载中心
- `/team`：团队工作区（仅有效 Team 权益或已有团队成员可进入）

下载中心只提供网盘分发入口。版本、文件名、校验值、网盘链接和提取码集中维护在
`public/downloads.json`；只有有效的 HTTPS 网盘地址才会开放对应入口。未配置网盘链接时页面
明确显示“网盘链接整理中”。公开下载一律通过网盘链接提供，不得在 Worker 或网页中暴露任何
私有仓库凭据。

## 首批接口

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/health` | 服务健康检查 |
| POST | `/api/v1/auth/register` | 创建待验证注册记录并发送验证邮件 |
| GET/POST | `/api/v1/auth/verify-email` | 消费一次性邮箱验证 Token |
| POST | `/api/v1/auth/login` | 登录并创建设备会话 |
| POST | `/api/v1/auth/refresh` | 轮换 Refresh Token |
| POST | `/api/v1/auth/logout` | 注销当前 Access Token 会话或 Refresh Token 家族 |
| POST | `/api/v1/auth/browser-session` | 根据 HttpOnly 浏览器会话获取短期 Access Token（仅同源） |
| POST | `/api/v1/auth/desktop/authorize` | 网页当前账户签发一次性软件授权码（仅同源） |
| POST | `/api/v1/auth/desktop/exchange` | 软件用 PKCE 校验密钥兑换独立设备会话 |
| GET | `/api/v1/account` | 查询当前云账户 |
| GET | `/api/v1/auth/me` | `/api/v1/account` 的兼容别名 |
| GET | `/api/v1/account/devices` | 查询当前账户的设备会话 |
| DELETE | `/api/v1/account/devices/:device_id` | 撤销设备及其会话 |
| GET | `/api/v1/account/subscription` | 查询套餐、订阅权益与账单能力 |
| POST | `/api/v1/billing/checkout` | 创建支付下单（通道为可替换实现） |
| POST | `/api/v1/billing/portal` | 创建账单管理会话（通道为可替换实现） |
| POST | `/api/v1/billing/webhook` | 接收支付渠道回调并同步权益 |
| GET | `/api/v1/sync` | 读取当前加密快照与修订元数据（Pro / Team） |
| PUT | `/api/v1/sync` | 以修订号并发控制写入加密快照（Pro / Team） |
| GET | `/api/v1/sync/history` | 查询最近 50 次同步修订（Pro / Team） |
| GET/POST | `/api/v1/teams` | 列出团队或创建团队工作区 |
| POST | `/api/v1/teams/invites/:invite_id/accept` | 接受发送到当前验证邮箱的邀请 |
| GET/PATCH/DELETE | `/api/v1/teams/:team_id/members...` | 查询及管理团队成员与角色 |
| GET/POST | `/api/v1/teams/:team_id/memories...` | 按作用域读取、写入、搜索或共享团队记忆 |

Access Token 通过 `Authorization: Bearer <token>` 传递。Refresh Token 只在请求体中
传递，桌面端接入时应保存到系统安全存储，不要放进普通 `localStorage`。

## 本地开发

```powershell
Set-Location lingshu-cloud
Copy-Item .dev.vars.example .dev.vars
# 编辑 .dev.vars，至少设置一个随机 JWT_SECRET
npm install
npx wrangler d1 migrations apply lingshu-cloud-dev --local
npm test
npm run check
npm run build
npm run dev
```

没有 `RESEND_API_KEY` 时，development 环境会把验证 URL 输出到 Wrangler 日志，不会发送真实邮件。
部署环境必须配置 `JWT_SECRET` 和 `RESEND_API_KEY`；`RESEND_FROM_EMAIL`、`APP_BASE_URL`
及 D1/R2 绑定在 `wrangler.jsonc` 中维护。不要把 Secret 写进该文件。

## 订阅与账单

账户中心采用三级 C 端订阅结构；完整产品线另包含企业版与嵌入版，详见
[产品定位与版本体系](../docs/product-positioning.md)：

- 免费版 Free（当前可用）：完整本地会话、离线使用、本地基础动态记忆、多模型自由接入（本地模型
  或自备 API Key，官方托管模型按量计费）、云账户和设备会话管理。本地 MCP Server 属于免费核心
  能力；具体开放状态以当前版本说明为准。
- 专业版 Pro：在免费版基础上增加个人私有记忆加密同步与冲突处理；高级经验沉淀、
  记忆参数自定义与跨设备 MCP 接入；以时长卡形式购买（1 个月 / 3 个月 / 6 个月 / 1 年），到期
  提醒手动续费。
- 团队版 Team：有效测试权益已可创建团队工作区、管理成员、共享记忆与审计记录；受邀成员无需
  单独购买 Team 即可进入已有工作区。团队 MCP 空间和按席位统一账单尚未开放，当前不公开购买。

本地核心能力和本地 MCP 不设付费墙，付费方案扩展云同步、跨设备连续性与协作治理。多模型接入属于本地能力，免费版同样可用：
用户可接入本地模型（如 Ollama）或自备第三方 API Key，不产生订阅费用；官方托管模型按量计费，与订阅费独立核算。

## 收款方案

面向国内用户，计划采用**微信支付 + 支付宝**收款：费率 0.6%，无单笔固定费，无拒付罚金，
提现免费。当前 `src/billing.ts` 仍是 Stripe 实现，作为支付通道的占位与联调用途，后续替换为
微信支付 / 支付宝；订单相关结构应保持渠道无关，以便平滑切换。

**自动续费（周期扣款）的准入门槛决定了收款形态，这一点比费率更重要：**

- 微信支付「委托代扣」：主体仅限企业 / 政府 / 事业单位 / 社会组织及其服务商——
  **个体工商户不在准入范围**（2025 年 7 月《微信支付自动续费管理规则调整公告》）；
  并按最近 4 周有效订单去重用户数分档：≥ 300 人且升级投诉率 ≤ 0.05% 维持 T+1 结算，
  20–300 人为 T+7，新主体（< 20 人）为 T+14，按月动态考核。
- 支付宝「商家扣款」：仅企业账号，注册资本 ≥ 2,000 万元，且近半年每月主动支付用户数 ≥ 300 人。
- 支付宝「周期扣款」：企业或个体工商户，注册资本 ≥ 100 万元（认缴，受 5 年实缴约束）；
  与「商家扣款」为不同产品页，两处口径不一致，以实际申请结果为准。

因此在取得公司主体与足够经营记录之前，**权益按时长卡出售（1 个月 / 3 个月 / 6 个月 / 1 年 + 到期提醒手动续费）**，
而不是自动续费订阅；会员分级与权益模型保持不变。按 0.6% 费率，按次购买不会增加通道成本。

## 支付渠道配置（当前 Stripe 占位实现）

支付渠道配置完整前，查询接口仍返回免费版，下单接口会安全返回 `BILLING_NOT_CONFIGURED`，不会产生费用。

- Secret：`STRIPE_SECRET_KEY`、`STRIPE_WEBHOOK_SECRET`
- Worker 变量：`STRIPE_PRICE_DEVELOPER_MONTHLY`、`STRIPE_PRICE_DEVELOPER_YEARLY`
- 可选展示文案：`STRIPE_DEVELOPER_MONTHLY_LABEL`、`STRIPE_DEVELOPER_YEARLY_LABEL`
- 可选门户配置：`STRIPE_PORTAL_CONFIGURATION_ID`

Webhook 地址为 `https://account.lingeringdream.com/api/v1/billing/webhook`，至少订阅
`checkout.session.completed`、`customer.subscription.created`、
`customer.subscription.updated` 和 `customer.subscription.deleted`。

## 加密同步与存储边界

默认桌面入口使用逐条增量协议：`GET/POST /api/v1/sync/vault`、`GET /api/v1/sync/records`、`GET/POST /api/v1/sync/records/:id` 与 `GET /api/v1/sync/records/:id/history`。正文在客户端使用随机恢复密钥加密，D1 仅保存密文与版本，每条保留最近 10 个旧版本；冲突由用户选择。聊天、项目和团队数据不进入此同步范围。详见[加密云同步及发布状态](../docs/encrypted-cloud-sync.md)。

以下整库快照实现保留为兼容 API，与默认协议使用不同的表、密钥和格式；当前界面不启动这套快照自动同步。新增迁移按完整文件名识别，`0006_encrypted_sync.sql` 与 `0006_pro_sync.sql` 互不替代。

### 整库快照兼容协议

- D1 `lingshu-cloud-dev`：待验证注册、云用户、设备、Refresh Token 会话、登录限流、订阅、同步修订
  元数据，以及团队成员、邀请、记忆权限和审计记录。
- R2 `lingshu-memory-dev`：通过 `MEMORY_BUCKET` 保存客户端生成的 AES-256-GCM 密文快照。Worker 不持有
  同步口令，不能解密记忆正文；每个账户最多保留最近 50 次修订对象。
- 邮箱验证 Token 与 Refresh Token 只保存 SHA-256 哈希；密码使用 Worker Web Crypto 的 PBKDF2-HMAC-SHA-256。
- Refresh Token 按会话家族轮换；检测到旧 Token 重放时，会撤销整个家族。

兼容客户端的整库快照同步先读取云端密文，在本机解密并按记忆 `updated_at` 合并，再以当前修订号写回新密文；
删除使用 tombstone 传播。同步加密口令至少 12 个字符，只在软件本次运行期间保存在内存中，同一账户
的各设备必须使用相同口令。跨设备 MCP 由“每台设备同步到本地，再由免费本机 MCP 读取”实现，
云端不提供持有明文的 MCP。

## 当前产品边界

桌面控制台现已增加「云账户」入口，使用本站已验证的邮箱与密码登录，可显示账户、
订阅和本机设备，并打开网页账户中心、账单与设备管理。Windows 凭据管理器、macOS
Keychain 和 Linux Secret Service 保存 Refresh Token；Access Token 仅保存在内存。
浏览器开发模式只在当前页面内存中保留云凭据。非敏感设备 ID 单独保存用于重复登录时复用设备。

软件启动时恢复云会话，控制台可见时每分钟及窗口重新聚焦时检查账户；网页撤销本机设备后，
下一次检查会要求重新登录。断网不会删除凭据或阻止本地控制台，退出云账户需联网完成撤销，
失败时保留会话并允许重试。网页与软件共用账号，但分别维护会话，不自动交换网页 Cookie。

现有 Rust 本地认证路由与 `local-session` 保持独立。网页登录本身不会上传、迁移、合并或删除本地数据；
只有用户明确开启并执行同步后才会读写云端密文。本阶段不包含忘记密码、修改密码、
账号删除、团队 MCP 和按席位统一账单。

## 通过网页登录软件

桌面端「云账户 → 通过网页登录」会打开系统浏览器：网页已有有效登录状态时自动完成验证，
否则在网页登录后继续。软件自动接收结果并显示当前账户；用户无需复制验证码或密码。
新版网页采用 Secure / HttpOnly / SameSite=Lax 的第一方会话 Cookie，使新标签页能识别登录状态；
旧版已打开的标签页刷新后会从原会话迁移。软件和网页分别持有设备会话，软件退出不会退出网页。

协议使用 S256 PKCE、随机 state、只绑定 `127.0.0.1` 随机端口的临时 HTTP 回调。回调监听最长
5 分钟，可在软件取消；授权码仅有效 60 秒，在 D1 内原子消费，错误密钥、重复兑换、过期及
授权网页会话已撤销均会被拒绝。回调 URL 只携带一次性 code 和 state，不携带 Access/Refresh Token。
软件回调校验 Host、state 和参数唯一性，完成或取消后释放端口。网页登录仅在桌面软件可用。
