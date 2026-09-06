import { describe, expect, it } from "vitest";
import { canCreateTeam, getEntitlements, subscriptionIsEntitled } from "../src/entitlements";

describe("product entitlements", () => {
  const now = Date.parse("2026-09-06T00:00:00Z");
  it.each(["developer", "pro", "team", "enterprise", "embedded"])("expires stale active %s subscriptions", plan_code => {
    for (const status of ["active", "trialing", "past_due"]) {
      const row = { plan_code, status, current_period_end: new Date(now).toISOString() };
      expect(subscriptionIsEntitled(row, now)).toBe(false);
      expect(getEntitlements(row, now).effective_plan).toBe("free");
    }
  });
  it.each(["free", "developer", "pro", "team", "enterprise", "embedded", "unknown"])("enforces the %s team boundary", plan_code => {
    expect(canCreateTeam({ plan_code, status: "active", current_period_end: null })).toBe(plan_code === "team");
  });
  it("preserves local access and never enables unreleased features", () => {
    for (const plan_code of ["free", "pro", "team", "enterprise", "embedded"]) {
      const result = getEntitlements({ plan_code, status: "active", current_period_end: null }, now);
      expect(result.features.find(f => f.code === "local_mcp")?.enabled).toBe(true);
      expect(result.features.filter(f => !f.available).every(f => !f.enabled)).toBe(true);
    }
  });
  it("only exposes three personal-product editions and excludes business entitlements", () => {
    for (const plan_code of ["enterprise", "embedded"]) {
      const result = getEntitlements({ plan_code, status: "active", current_period_end: null });
      expect(result.effective_plan).toBe("free");
      expect(result.features.every(f => f.plans.every(p => ["free", "pro", "team"].includes(p)))).toBe(true);
      expect(result.features.some(f => /enterprise|embedded/.test(f.code))).toBe(false);
    }
  });
  it("handles legacy codes, missing subscriptions and invalid dates conservatively", () => {
    expect(getEntitlements(null).effective_plan).toBe("free");
    expect(getEntitlements({ plan_code: "developer", status: "active", current_period_end: null }).effective_plan).toBe("pro");
    for (const current_period_end of [null, "invalid", new Date(now - 1).toISOString()]) {
      expect(subscriptionIsEntitled({ plan_code: "team", status: "past_due", current_period_end }, now)).toBe(false);
    }
    expect(subscriptionIsEntitled({ plan_code: "team", status: "past_due", current_period_end: new Date(now + 1).toISOString() }, now)).toBe(true);
    expect(canCreateTeam({ plan_code: "enterprise", status: "canceled", current_period_end: null })).toBe(false);
  });
});
