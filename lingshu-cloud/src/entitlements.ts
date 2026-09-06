/** Server-owned product policy. Subscription rows are never accepted from clients. */
export type Plan = "free" | "pro" | "team";
export interface SubscriptionState {
  plan_code: string;
  status: string;
  current_period_end: string | null;
}

export function normalizePlan(code: string): Plan | null {
  if (code === "developer") return "pro";
  return ["free", "pro", "team"].includes(code) ? code as Plan : null;
}

export function subscriptionIsEntitled(row: SubscriptionState | null, now = Date.now()): boolean {
  if (!row || !normalizePlan(row.plan_code)) return false;
  if (!["active", "trialing", "past_due"].includes(row.status)) return false;
  // A known expiry is authoritative, including for stale active/trialing rows.
  if (row.current_period_end !== null) {
    const end = Date.parse(row.current_period_end);
    if (!Number.isFinite(end) || end <= now) return false;
  }
  return row.status !== "past_due" || row.current_period_end !== null;
}

const features = [
  { code: "local_memory", name: "本地聊天与动态记忆", plans: ["free", "pro", "team"], available: true },
  { code: "local_mcp", name: "本地 MCP 接入", plans: ["free", "pro", "team"], available: true },
  { code: "local_backup", name: "本地数据备份", plans: ["free", "pro", "team"], available: true },
  { code: "cloud_sync", name: "加密云同步与跨设备 MCP", plans: ["pro", "team"], available: false },
  { code: "advanced_governance", name: "高级记忆治理", plans: ["pro", "team"], available: false },
  { code: "team_create", name: "创建团队工作区", plans: ["team"], available: true },
] as const;

export function getEntitlements(row: SubscriptionState | null, now = Date.now()) {
  const plan: Plan = subscriptionIsEntitled(row, now) ? normalizePlan(row!.plan_code)! : "free";
  return {
    effective_plan: plan,
    features: features.map(feature => {
      const included = (feature.plans as readonly string[]).includes(plan);
      return { code: feature.code, name: feature.name, plans: feature.plans, included,
        available: feature.available, enabled: included && feature.available };
    }),
  };
}

export function canCreateTeam(row: SubscriptionState | null): boolean {
  return getEntitlements(row).features.some(feature => feature.code === "team_create" && feature.enabled);
}
