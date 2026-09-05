# LingShu Cloud

独立的 LingShu 云账户 Worker。它只负责云账户和设备会话，不替换现有
`lingshu-server` 的 PostgreSQL 用户表或 `local-session`，因此桌面端离线能力保持不变。

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

下载中心以网盘分发为首选，并提供 GitHub Release 下载作为备用。版本、文件名、校验值、
网盘链接、GitHub 资产地址和提取码集中维护在 `public/downloads.json`；只有有效的 HTTPS
地址才会开放对应入口。未配置网盘链接时页面明确显示“网盘链接整理中”，GitHub 下载仍可用。
当前 GitHub 仓库为私有仓库，因此 GitHub 下载仅对已登录且拥有仓库权限的账户可用；面向普通
用户的公开下载仍应配置网盘链接，不得在 Worker 或网页中暴露 GitHub Token。

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

账户中心采用三级订阅结构：

- 免费版（当前可用）：完整本地会话、离线使用、本地基础动态记忆、多模型自由接入（本地模型
  或自备 API Key，官方托管模型按量计费）、云账户和设备会话管理。
- 开发者版（随云同步开放）：在免费版基础上增加加密增量同步、多设备冲突历史、高级经验沉淀
  与记忆参数自定义；以时长卡形式购买（1 个月 / 3 个月 / 6 个月 / 1 年），到期提醒手动续费。
- 团队版（规划中）：增加团队工作区、共享记忆空间、成员权限、审计记录、设备治理和按席位
  统一账单；当前不开放购买。

本地核心能力不设付费墙，付费方案只扩展云端与协作能力。多模型接入属于本地能力，免费版同样可用：
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

## 存储边界

- D1 `lingshu-cloud-dev`：待验证注册、云用户、设备、Refresh Token 会话和登录限流记录。
- R2 `lingshu-memory-dev`：已预留为 `MEMORY_BUCKET` 绑定，首批账号接口不会读写记忆对象。
- 邮箱验证 Token 与 Refresh Token 只保存 SHA-256 哈希；密码使用 Worker Web Crypto 的 PBKDF2-HMAC-SHA-256。
- Refresh Token 按会话家族轮换；检测到旧 Token 重放时，会撤销整个家族。

## 当前刻意不做的事

桌面控制台现已增加「云账户」入口，使用本站已验证的邮箱与密码登录，可显示账户、
订阅和本机设备，并打开网页账户中心、账单与设备管理。Windows 凭据管理器、macOS
Keychain 和 Linux Secret Service 保存 Refresh Token；Access Token 仅保存在内存。
浏览器开发模式只在当前页面内存中保留云凭据。非敏感设备 ID 单独保存用于重复登录时复用设备。

软件启动时恢复云会话，控制台可见时每分钟及窗口重新聚焦时检查账户；网页撤销本机设备后，
下一次检查会要求重新登录。断网不会删除凭据或阻止本地控制台，退出云账户需联网完成撤销，
失败时保留会话并允许重试。网页与软件共用账号，但分别维护会话，不自动交换网页 Cookie。

现有 Rust 本地认证路由与 `local-session` 保持独立。本阶段不包含忘记密码、账号删除、
记忆同步和冲突合并。登录与退出不会迁移、合并或删除本地数据。

## 通过网页登录软件

桌面端「云账户 → 通过网页登录」会打开系统浏览器：网页已有有效登录状态时自动完成验证，
否则在网页登录后继续。软件自动接收结果并显示当前账户；用户无需复制验证码或密码。
新版网页采用 Secure / HttpOnly / SameSite=Lax 的第一方会话 Cookie，使新标签页能识别登录状态；
旧版已打开的标签页刷新后会从原会话迁移。软件和网页分别持有设备会话，软件退出不会退出网页。

协议使用 S256 PKCE、随机 state、只绑定 `127.0.0.1` 随机端口的临时 HTTP 回调。回调监听最长
5 分钟，可在软件取消；授权码仅有效 60 秒，在 D1 内原子消费，错误密钥、重复兑换、过期及
授权网页会话已撤销均会被拒绝。回调 URL 只携带一次性 code 和 state，不携带 Access/Refresh Token。
软件回调校验 Host、state 和参数唯一性，完成或取消后释放端口。网页登录仅在桌面软件可用。
