import { getEntitlements, normalizePlan, subscriptionIsEntitled } from "./entitlements";
import { HttpError } from "./errors";
import type { AppEnv, AuthContext } from "./types";

export type StripeFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

interface BillingCustomerRow {
  user_id: string;
  provider_customer_id: string;
}

interface SubscriptionRow {
  user_id: string;
  provider_customer_id: string;
  provider_subscription_id: string;
  price_id: string | null;
  plan_code: string;
  billing_interval: string | null;
  status: string;
  current_period_end: string | null;
  cancel_at_period_end: number;
  created_at: string;
  updated_at: string;
}

interface StripeEvent {
  id: string;
  type: string;
  created?: number;
  data?: { object?: Record<string, unknown> };
}

interface StripeObject {
  id?: string;
  url?: string;
  customer?: string | { id?: string };
  subscription?: string | { id?: string };
  client_reference_id?: string;
  status?: string;
  cancel_at_period_end?: boolean;
  current_period_end?: number;
  metadata?: Record<string, string>;
  items?: {
    data?: Array<{
      current_period_end?: number;
      price?: { id?: string; recurring?: { interval?: string } };
    }>;
  };
}

const STRIPE_API_VERSION = "2026-02-25.clover";
const WEBHOOK_TOLERANCE_SECONDS = 300;
const textEncoder = new TextEncoder();

// 专业版购买档位：interval → 实际时长（月）。复用 Stripe 月付 / 年付 price 收款，
// 到期时间由我方按所选时长重算，而非依赖 Stripe 的原生自动续费周期。
interface DeveloperOptionSpec {
  interval: string;
  durationMonths: number;
  // 复用哪个已配置的 price：monthly 用月付 price，yearly 用年付 price
  priceKind: "monthly" | "yearly";
  defaultLabel: string;
}
const DEVELOPER_OPTIONS: DeveloperOptionSpec[] = [
  { interval: "month", durationMonths: 1, priceKind: "monthly", defaultLabel: "购买 1 个月" },
  { interval: "month3", durationMonths: 3, priceKind: "monthly", defaultLabel: "购买 3 个月" },
  { interval: "halfyear", durationMonths: 6, priceKind: "yearly", defaultLabel: "购买 6 个月" },
  { interval: "year", durationMonths: 12, priceKind: "yearly", defaultLabel: "购买 1 年" },
];

function developerOption(interval: string): DeveloperOptionSpec | undefined {
  return DEVELOPER_OPTIONS.find((o) => o.interval === interval);
}

function arrayBufferOf(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

function nowIso(): string {
  return new Date().toISOString();
}

function changesOf(result: { meta?: { changes?: number } } | undefined): number {
  return result?.meta?.changes ?? 0;
}

function appBaseUrl(env: AppEnv): string {
  const raw = env.APP_BASE_URL?.trim() || "http://localhost:8787";
  try {
    return new URL(raw).origin;
  } catch {
    throw new HttpError(500, "Application base URL is invalid", "INVALID_APP_BASE_URL");
  }
}

function stripeSecret(env: AppEnv): string {
  const secret = env.STRIPE_SECRET_KEY?.trim();
  if (!secret) {
    throw new HttpError(503, "Billing is not configured", "BILLING_NOT_CONFIGURED");
  }
  return secret;
}

function checkoutPrice(env: AppEnv, interval: string): string {
  const spec = developerOption(interval);
  const price = spec?.priceKind === "monthly"
    ? env.STRIPE_PRICE_DEVELOPER_MONTHLY?.trim()
    : spec?.priceKind === "yearly"
      ? env.STRIPE_PRICE_DEVELOPER_YEARLY?.trim()
      : undefined;
  if (!price) {
    throw new HttpError(503, "This billing interval is not configured", "BILLING_NOT_CONFIGURED");
  }
  return price;
}

function isBillingConfigured(env: AppEnv): boolean {
  return Boolean(
    env.STRIPE_SECRET_KEY?.trim() &&
    (env.STRIPE_PRICE_DEVELOPER_MONTHLY?.trim() ||
      env.STRIPE_PRICE_DEVELOPER_YEARLY?.trim()),
  );
}

function unixTimeToIso(value: unknown): string | null {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return null;
  return new Date(value * 1_000).toISOString();
}

function objectId(value: unknown): string | null {
  if (typeof value === "string" && value.length > 0) return value;
  if (value && typeof value === "object" && "id" in value) {
    const id = (value as { id?: unknown }).id;
    return typeof id === "string" && id.length > 0 ? id : null;
  }
  return null;
}

function planName(planCode: string): string {
  if (normalizePlan(planCode) === "pro") return "专业版 Pro";
  if (planCode === "team") return "团队版";
  if (planCode === "free") return "免费版";
  return "未识别方案";
}

function displayLabel(value: string | undefined, fallback: string): string {
  return value?.trim() || fallback;
}

export async function getBillingOverview(
  env: AppEnv,
  auth: AuthContext,
): Promise<Record<string, unknown>> {
  const [customer, subscription, teamMembership] = await Promise.all([
    env.DB.prepare(
      "SELECT user_id, provider_customer_id FROM billing_customers WHERE user_id = ?",
    )
      .bind(auth.userId)
      .first<BillingCustomerRow>(),
    env.DB.prepare("SELECT * FROM subscriptions WHERE user_id = ?")
      .bind(auth.userId)
      .first<SubscriptionRow>(),
    env.DB.prepare("SELECT 1 AS present FROM team_members WHERE cloud_user_id = ? LIMIT 1")
      .bind(auth.userId)
      .first<{ present: number }>(),
  ]);
  const entitled = subscriptionIsEntitled(subscription);
  const configured = isBillingConfigured(env);
  return {
    entitlements: getEntitlements(subscription, Date.now(), Boolean(teamMembership)),
    subscription: subscription
      ? {
          plan_code: subscription.plan_code,
          plan_name: planName(subscription.plan_code),
          status: subscription.status,
          billing_interval: subscription.billing_interval,
          current_period_end: subscription.current_period_end,
          cancel_at_period_end: subscription.cancel_at_period_end === 1,
          entitlement_active: entitled,
        }
      : {
          plan_code: "free",
          plan_name: "免费版",
          status: "active",
          billing_interval: null,
          current_period_end: null,
          cancel_at_period_end: false,
          entitlement_active: true,
        },
    billing: {
      configured,
      customer_portal_available: Boolean(env.STRIPE_SECRET_KEY?.trim() && customer),
    },
    plans: [
      {
        code: "free",
        name: "免费版",
        availability: "available",
        description: "本地核心能力与基础云账户",
        features: ["完整本地会话与离线使用", "本地基础动态记忆", "多模型自由接入（本地模型 / 自备 API Key）", "云账户与设备会话管理", "本地数据自主备份", "本地 MCP 接入"],
        options: [],
      },
      {
        code: "developer",
        name: "专业版 Pro",
        availability: "upcoming",
        description: "为多设备和高级记忆工作流准备",
        features: ["全部免费版权益", "加密云端记忆同步", "多设备增量同步与冲突历史", "跨设备 MCP 接入", "高级经验沉淀与记忆参数自定义"],
        options: DEVELOPER_OPTIONS.map((spec) => ({
          interval: spec.interval,
          label: displayLabel(
            spec.priceKind === "monthly"
              ? env.STRIPE_DEVELOPER_MONTHLY_LABEL
              : env.STRIPE_DEVELOPER_YEARLY_LABEL,
            spec.defaultLabel,
          ),
          available: Boolean(
            env.STRIPE_SECRET_KEY?.trim() &&
              (spec.priceKind === "monthly"
                ? env.STRIPE_PRICE_DEVELOPER_MONTHLY?.trim()
                : env.STRIPE_PRICE_DEVELOPER_YEARLY?.trim()),
          ),
        })),
      },
      {
        code: "team",
        name: "团队版",
        availability: "planned",
        description: "为共享记忆边界与组织治理准备",
        features: ["全部专业版权益", "团队工作区与共享记忆空间", "成员角色与访问权限", "团队 MCP 空间", "团队审计记录与设备治理", "按席位统一账单"],
        options: [],
      },
    ],
  };
}

async function stripeRequest(
  env: AppEnv,
  path: string,
  parameters: URLSearchParams,
  stripeFetch: StripeFetch,
  idempotencyKey?: string,
): Promise<StripeObject> {
  const base = env.STRIPE_API_BASE_URL?.trim() || "https://api.stripe.com";
  const headers = new Headers({
    Authorization: `Bearer ${stripeSecret(env)}`,
    "Content-Type": "application/x-www-form-urlencoded",
    "Stripe-Version": STRIPE_API_VERSION,
  });
  if (idempotencyKey) headers.set("Idempotency-Key", idempotencyKey);
  let response: Response;
  try {
    response = await stripeFetch(`${base.replace(/\/$/, "")}/v1/${path}`, {
      method: "POST",
      headers,
      body: parameters,
    });
  } catch {
    throw new HttpError(502, "Billing provider is unavailable", "BILLING_PROVIDER_UNAVAILABLE");
  }
  const value = await response.json().catch(() => null) as StripeObject | null;
  if (!response.ok || !value) {
    console.error("Stripe API request failed", {
      path,
      status: response.status,
      requestId: response.headers.get("Request-Id"),
    });
    throw new HttpError(502, "Billing provider rejected the request", "BILLING_PROVIDER_ERROR");
  }
  return value;
}

async function ensureStripeCustomer(
  env: AppEnv,
  auth: AuthContext,
  stripeFetch: StripeFetch,
): Promise<string> {
  const existing = await env.DB.prepare(
    "SELECT user_id, provider_customer_id FROM billing_customers WHERE user_id = ?",
  )
    .bind(auth.userId)
    .first<BillingCustomerRow>();
  if (existing) return existing.provider_customer_id;

  const parameters = new URLSearchParams({
    email: auth.email,
    name: auth.displayName,
    "metadata[user_id]": auth.userId,
    "metadata[service]": "lingshu-cloud",
  });
  const created = await stripeRequest(
    env,
    "customers",
    parameters,
    stripeFetch,
    `lingshu-customer-${auth.userId}`,
  );
  if (!created.id?.startsWith("cus_")) {
    throw new HttpError(502, "Billing provider returned an invalid customer", "BILLING_PROVIDER_ERROR");
  }
  const now = nowIso();
  await env.DB.prepare(
    `INSERT INTO billing_customers
      (user_id, provider, provider_customer_id, created_at, updated_at)
     VALUES (?, 'stripe', ?, ?, ?)
     ON CONFLICT(user_id) DO NOTHING`,
  )
    .bind(auth.userId, created.id, now, now)
    .run();
  const canonical = await env.DB.prepare(
    "SELECT user_id, provider_customer_id FROM billing_customers WHERE user_id = ?",
  )
    .bind(auth.userId)
    .first<BillingCustomerRow>();
  if (!canonical) {
    throw new HttpError(500, "Billing customer could not be saved", "BILLING_STATE_ERROR");
  }
  return canonical.provider_customer_id;
}

export async function createCheckoutSession(
  env: AppEnv,
  auth: AuthContext,
  interval: string,
  stripeFetch: StripeFetch = fetch,
): Promise<{ url: string }> {
  const priceId = checkoutPrice(env, interval);
  const existing = await env.DB.prepare("SELECT * FROM subscriptions WHERE user_id = ?")
    .bind(auth.userId)
    .first<SubscriptionRow>();
  if (existing && ["active", "trialing", "past_due", "incomplete"].includes(existing.status)) {
    throw new HttpError(
      409,
      "Manage the existing subscription in the billing portal",
      "SUBSCRIPTION_ALREADY_EXISTS",
    );
  }
  const customerId = await ensureStripeCustomer(env, auth, stripeFetch);
  const base = appBaseUrl(env);
  const parameters = new URLSearchParams({
    mode: "subscription",
    customer: customerId,
    "line_items[0][price]": priceId,
    "line_items[0][quantity]": "1",
    client_reference_id: auth.userId,
    "metadata[user_id]": auth.userId,
    "metadata[plan_code]": "developer",
    "metadata[billing_interval]": interval,
    "subscription_data[metadata][user_id]": auth.userId,
    "subscription_data[metadata][plan_code]": "developer",
    "subscription_data[metadata][billing_interval]": interval,
    allow_promotion_codes: "true",
    success_url: `${base}/billing?billing=success&session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${base}/billing?billing=canceled`,
  });
  const session = await stripeRequest(
    env,
    "checkout/sessions",
    parameters,
    stripeFetch,
    `lingshu-checkout-${auth.userId}-${interval}-${Math.floor(Date.now() / 600_000)}`,
  );
  if (!session.url?.startsWith("https://checkout.stripe.com/")) {
    throw new HttpError(502, "Billing provider returned an invalid checkout URL", "BILLING_PROVIDER_ERROR");
  }
  return { url: session.url };
}

export async function createPortalSession(
  env: AppEnv,
  auth: AuthContext,
  stripeFetch: StripeFetch = fetch,
): Promise<{ url: string }> {
  stripeSecret(env);
  const customer = await env.DB.prepare(
    "SELECT user_id, provider_customer_id FROM billing_customers WHERE user_id = ?",
  )
    .bind(auth.userId)
    .first<BillingCustomerRow>();
  if (!customer) {
    throw new HttpError(409, "No billing customer exists", "BILLING_CUSTOMER_NOT_FOUND");
  }
  const parameters = new URLSearchParams({
    customer: customer.provider_customer_id,
    return_url: `${appBaseUrl(env)}/billing?billing=returned`,
  });
  if (env.STRIPE_PORTAL_CONFIGURATION_ID?.trim()) {
    parameters.set("configuration", env.STRIPE_PORTAL_CONFIGURATION_ID.trim());
  }
  const session = await stripeRequest(env, "billing_portal/sessions", parameters, stripeFetch);
  if (!session.url?.startsWith("https://billing.stripe.com/")) {
    throw new HttpError(502, "Billing provider returned an invalid portal URL", "BILLING_PROVIDER_ERROR");
  }
  return { url: session.url };
}

function hexBytes(value: string): Uint8Array | null {
  if (!/^[0-9a-f]{64}$/i.test(value)) return null;
  const bytes = new Uint8Array(value.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

async function verifyWebhookSignature(
  payload: string,
  signatureHeader: string,
  secret: string,
): Promise<boolean> {
  const parts = signatureHeader.split(",");
  const timestamp = parts.find((part) => part.startsWith("t="))?.slice(2);
  const signatures = parts
    .filter((part) => part.startsWith("v1="))
    .map((part) => part.slice(3));
  const timestampNumber = Number(timestamp);
  if (
    !Number.isSafeInteger(timestampNumber) ||
    Math.abs(Math.floor(Date.now() / 1_000) - timestampNumber) > WEBHOOK_TOLERANCE_SECONDS ||
    signatures.length === 0
  ) {
    return false;
  }
  const key = await crypto.subtle.importKey(
    "raw",
    textEncoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"],
  );
  const signedPayload = textEncoder.encode(`${timestamp}.${payload}`);
  for (const signature of signatures) {
    const bytes = hexBytes(signature);
    if (bytes && await crypto.subtle.verify("HMAC", key, arrayBufferOf(bytes), signedPayload)) {
      return true;
    }
  }
  return false;
}

async function knownUser(env: AppEnv, userId: string | null): Promise<string | null> {
  if (!userId) return null;
  const user = await env.DB.prepare("SELECT id FROM users WHERE id = ? AND deleted_at IS NULL")
    .bind(userId)
    .first<{ id: string }>();
  return user?.id || null;
}

async function recordCustomer(
  env: AppEnv,
  userId: string,
  customerId: string,
): Promise<void> {
  const now = nowIso();
  await env.DB.prepare(
    `INSERT INTO billing_customers
      (user_id, provider, provider_customer_id, created_at, updated_at)
     VALUES (?, 'stripe', ?, ?, ?)
     ON CONFLICT(user_id) DO UPDATE SET
       provider_customer_id = excluded.provider_customer_id,
       updated_at = excluded.updated_at`,
  )
    .bind(userId, customerId, now, now)
    .run();
}

async function handleCheckoutCompleted(
  env: AppEnv,
  object: StripeObject,
): Promise<void> {
  const userId = await knownUser(
    env,
    object.metadata?.user_id || object.client_reference_id || null,
  );
  const customerId = objectId(object.customer);
  if (userId && customerId) await recordCustomer(env, userId, customerId);
}

function configuredInterval(env: AppEnv, priceId: string | null, object: StripeObject): string | null {
  // 优先信任下单时写入的 metadata 时长（支持 month3 / halfyear 等扩展档位）
  const metadataInterval = object.metadata?.billing_interval;
  if (metadataInterval && developerOption(metadataInterval)) return metadataInterval;
  if (priceId && priceId === env.STRIPE_PRICE_DEVELOPER_MONTHLY?.trim()) return "month";
  if (priceId && priceId === env.STRIPE_PRICE_DEVELOPER_YEARLY?.trim()) return "year";
  const recurring = object.items?.data?.[0]?.price?.recurring?.interval;
  if (recurring === "month" || recurring === "year") return recurring;
  return null;
}

// 按档位实际时长重算到期时间：month3/halfyear 复用 Stripe 月付/年付 price 收款，
// 但到期应加「所选时长」而非 Stripe 的 +1 个月/年。
function addMonths(base: Date, months: number): Date {
  const d = new Date(base);
  const day = d.getDate();
  d.setMonth(d.getMonth() + months);
  // 月末溢出修正：例如 1/31 + 1 个月 → 2/28 而非 3/3
  if (d.getDate() !== day) d.setDate(0);
  return d;
}

function subscriptionPeriodEnd(env: AppEnv, priceId: string | null, object: StripeObject): string | null {
  const interval = configuredInterval(env, priceId, object);
  const spec = interval ? developerOption(interval) : undefined;
  const stripeEnd = object.current_period_end ?? object.items?.data?.[0]?.current_period_end;
  if (!spec || !stripeEnd) return unixTimeToIso(stripeEnd);
  // Stripe 月付 price 的 current_period_end = 起付 + 1 个月；年付 = + 1 年。
  // 反推周期起点，再按所选时长重算到期。
  const stripeMonths = spec.priceKind === "monthly" ? 1 : 12;
  const anchor = addMonths(new Date(stripeEnd * 1000), -stripeMonths);
  return addMonths(anchor, spec.durationMonths).toISOString();
}

async function handleSubscriptionEvent(
  env: AppEnv,
  eventType: string,
  object: StripeObject,
): Promise<void> {
  const customerId = objectId(object.customer);
  const subscriptionId = object.id;
  if (!customerId || !subscriptionId) return;
  const mapped = await env.DB.prepare(
    "SELECT user_id, provider_customer_id FROM billing_customers WHERE provider_customer_id = ?",
  )
    .bind(customerId)
    .first<BillingCustomerRow>();
  const userId = await knownUser(env, object.metadata?.user_id || mapped?.user_id || null);
  if (!userId) return;
  await recordCustomer(env, userId, customerId);

  const item = object.items?.data?.[0];
  const priceId = item?.price?.id || null;
  const knownDeveloperPrice = Boolean(
    priceId &&
    (priceId === env.STRIPE_PRICE_DEVELOPER_MONTHLY?.trim() ||
      priceId === env.STRIPE_PRICE_DEVELOPER_YEARLY?.trim()),
  );
  const planCode = knownDeveloperPrice ? "developer" : "unrecognized";
  const periodEnd = subscriptionPeriodEnd(env, priceId, object);
  const status = eventType === "customer.subscription.deleted" ? "canceled" : object.status || "unknown";
  const now = nowIso();
  await env.DB.prepare(
    `INSERT INTO subscriptions
      (user_id, provider, provider_customer_id, provider_subscription_id,
       price_id, plan_code, billing_interval, status, current_period_end,
       cancel_at_period_end, created_at, updated_at)
     VALUES (?, 'stripe', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(user_id) DO UPDATE SET
       provider_customer_id = excluded.provider_customer_id,
       provider_subscription_id = excluded.provider_subscription_id,
       price_id = excluded.price_id,
       plan_code = excluded.plan_code,
       billing_interval = excluded.billing_interval,
       status = excluded.status,
       current_period_end = excluded.current_period_end,
       cancel_at_period_end = excluded.cancel_at_period_end,
       updated_at = excluded.updated_at`,
  )
    .bind(
      userId,
      customerId,
      subscriptionId,
      priceId,
      planCode,
      configuredInterval(env, priceId, object),
      status,
      periodEnd,
      object.cancel_at_period_end ? 1 : 0,
      now,
      now,
    )
    .run();
}

async function processEvent(env: AppEnv, event: StripeEvent): Promise<void> {
  const object = (event.data?.object || {}) as StripeObject;
  if (event.type === "checkout.session.completed") {
    await handleCheckoutCompleted(env, object);
    return;
  }
  if (
    event.type === "customer.subscription.created" ||
    event.type === "customer.subscription.updated" ||
    event.type === "customer.subscription.deleted"
  ) {
    await handleSubscriptionEvent(env, event.type, object);
  }
}

export async function processStripeWebhook(
  request: Request,
  env: AppEnv,
): Promise<{ received: true; duplicate?: true }> {
  const webhookSecret = env.STRIPE_WEBHOOK_SECRET?.trim();
  if (!webhookSecret) {
    throw new HttpError(503, "Billing webhook is not configured", "BILLING_NOT_CONFIGURED");
  }
  const rawBody = await request.text();
  if (textEncoder.encode(rawBody).byteLength > 1_048_576) {
    throw new HttpError(413, "Webhook payload is too large", "BODY_TOO_LARGE");
  }
  const signature = request.headers.get("Stripe-Signature") || "";
  if (!(await verifyWebhookSignature(rawBody, signature, webhookSecret))) {
    throw new HttpError(400, "Invalid webhook signature", "INVALID_WEBHOOK_SIGNATURE");
  }
  let event: StripeEvent;
  try {
    event = JSON.parse(rawBody) as StripeEvent;
  } catch {
    throw new HttpError(400, "Invalid webhook payload", "INVALID_WEBHOOK_PAYLOAD");
  }
  if (!event.id || event.id.length > 255 || !event.type || event.type.length > 255) {
    throw new HttpError(400, "Invalid webhook event", "INVALID_WEBHOOK_PAYLOAD");
  }

  const existing = await env.DB.prepare("SELECT status FROM billing_events WHERE id = ?")
    .bind(event.id)
    .first<{ status: string }>();
  if (existing?.status === "processed" || existing?.status === "processing") {
    return { received: true, duplicate: true };
  }
  const now = nowIso();
  if (existing) {
    await env.DB.prepare(
      "UPDATE billing_events SET status = 'processing', error_code = NULL, processed_at = NULL WHERE id = ?",
    )
      .bind(event.id)
      .run();
  } else {
    const inserted = await env.DB.prepare(
      `INSERT OR IGNORE INTO billing_events
        (id, event_type, event_created_at, status, created_at)
       VALUES (?, ?, ?, 'processing', ?)`,
    )
      .bind(event.id, event.type, unixTimeToIso(event.created), now)
      .run();
    if (changesOf(inserted) === 0) return { received: true, duplicate: true };
  }

  try {
    await processEvent(env, event);
    await env.DB.prepare(
      "UPDATE billing_events SET status = 'processed', processed_at = ?, error_code = NULL WHERE id = ?",
    )
      .bind(nowIso(), event.id)
      .run();
  } catch (error) {
    const code = error instanceof HttpError ? error.code : "PROCESSING_FAILED";
    await env.DB.prepare(
      "UPDATE billing_events SET status = 'failed', processed_at = ?, error_code = ? WHERE id = ?",
    )
      .bind(nowIso(), code, event.id)
      .run();
    throw error;
  }
  return { received: true };
}
