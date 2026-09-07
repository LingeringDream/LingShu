import { env } from "cloudflare:workers";
import { beforeEach, describe, expect, it } from "vitest";

import { handleRequest } from "../src/index";
import { hashPassword, sha256Base64Url } from "../src/crypto";
import type { StripeFetch } from "../src/billing";
import type { AppEnv, VerificationEmail } from "../src/types";

const TEST_JWT_SECRET = "test-only-secret-that-is-long-enough-32";

function testEnv(): AppEnv {
  return {
    DB: env.DB,
    MEMORY_BUCKET: env.MEMORY_BUCKET,
    ASSETS: env.ASSETS,
    JWT_SECRET: TEST_JWT_SECRET,
    ENVIRONMENT: "test",
    APP_BASE_URL: "http://localhost:8787",
    ACCESS_TOKEN_TTL_SECONDS: "900",
    REFRESH_TOKEN_TTL_SECONDS: "2592000",
    VERIFICATION_TOKEN_TTL_SECONDS: "86400",
    LOGIN_WINDOW_SECONDS: "900",
    LOGIN_MAX_ATTEMPTS: "5",
    LOGIN_BLOCK_SECONDS: "900",
    CORS_ORIGINS: "http://localhost:5173",
  };
}

const context = {
  waitUntil() {},
  passThroughOnException() {},
} as unknown as ExecutionContext;

async function request(
  path: string,
  init: RequestInit = {},
  mailer?: (message: VerificationEmail, env: AppEnv) => Promise<void>,
  envOverrides: Partial<AppEnv> = {},
  stripeFetch?: StripeFetch,
): Promise<Response> {
  return handleRequest(
    new Request(`http://localhost${path}`, init),
    { ...testEnv(), ...envOverrides },
    context,
    mailer,
    stripeFetch,
  );
}

function jsonInit(body: unknown, method = "POST"): RequestInit {
  return {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  };
}

async function responseJson(response: Response): Promise<Record<string, any>> {
  return (await response.json()) as Record<string, any>;
}

async function resetDatabase(): Promise<void> {
  await env.DB.batch([
    env.DB.prepare("DELETE FROM desktop_authorization_codes"),
    env.DB.prepare("DELETE FROM billing_events"),
    env.DB.prepare("DELETE FROM subscriptions"),
    env.DB.prepare("DELETE FROM billing_customers"),
    env.DB.prepare("DELETE FROM login_attempts"),
    env.DB.prepare("DELETE FROM sessions"),
    env.DB.prepare("DELETE FROM devices"),
    env.DB.prepare("DELETE FROM users"),
    env.DB.prepare("DELETE FROM pending_registrations"),
  ]);
}

async function registerAndVerify(email = "alice@example.com") {
  let emailMessage: VerificationEmail | undefined;
  const registerResponse = await request(
    "/api/v1/auth/register",
    jsonInit({
      email,
      display_name: "Alice",
      password: "correct horse battery staple",
    }),
    async (message) => {
      emailMessage = message;
    },
  );
  expect(registerResponse.status).toBe(202);
  expect(emailMessage?.token).toBeTruthy();

  const verifyResponse = await request(
    "/api/v1/auth/verify-email",
    jsonInit({ token: emailMessage?.token }),
  );
  expect(verifyResponse.status).toBe(200);
  return responseJson(verifyResponse);
}

async function loginVerifiedAccount(email = "billing@example.com") {
  const verified = await registerAndVerify(email);
  const login = await responseJson(
    await request(
      "/api/v1/auth/login",
      jsonInit({
        email,
        password: "correct horse battery staple",
        device_name: "Billing browser",
        platform: "web",
      }),
    ),
  );
  return { verified, login };
}

async function stripeSignature(payload: string, secret: string): Promise<string> {
  const timestamp = Math.floor(Date.now() / 1_000);
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const digest = new Uint8Array(
    await crypto.subtle.sign("HMAC", key, encoder.encode(`${timestamp}.${payload}`)),
  );
  const signature = Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("");
  return `t=${timestamp},v1=${signature}`;
}

beforeEach(async () => {
  await resetDatabase();
  await env.DB.batch([
    env.DB.prepare("DELETE FROM team_members"),
    env.DB.prepare("DELETE FROM team_workspaces"),
  ]);
});

describe("lingshu-cloud account service", () => {
  async function browserLogin() {
    await registerAndVerify("browser@example.com");
    const response = await request("/api/v1/auth/login", {
      ...jsonInit({ email: "browser@example.com", password: "correct horse battery staple", device_name: "Browser", platform: "windows" }),
      headers: { Origin: "http://localhost", "Content-Type": "application/json" },
    });
    const setCookie = response.headers.get("Set-Cookie")!;
    expect(setCookie).toContain("Secure; HttpOnly; SameSite=Lax");
    return { cookie: setCookie.split(";")[0], tokens: await responseJson(response) };
  }

  async function authorizeDesktop(cookie: string, overrides = {}) {
    const verifier = "v".repeat(64);
    const payload = { redirect_uri: "http://127.0.0.1:54321/callback", code_challenge: await sha256Base64Url(verifier), code_challenge_method: "S256", state: "a".repeat(64), device_name: "Desktop", platform: "windows", ...overrides };
    const response = await request("/api/v1/auth/desktop/authorize", { ...jsonInit(payload), headers: { Origin: "http://localhost", Cookie: cookie, "Content-Type": "application/json" } });
    return { response, verifier, payload };
  }

  it("recognises a signed-in browser in a new tab without exposing its durable cookie", async () => {
    const { cookie } = await browserLogin();
    const response = await request("/api/v1/auth/browser-session", { ...jsonInit({}), headers: { Origin: "http://localhost", Cookie: cookie } });
    expect(response.status).toBe(200);
    const data = await responseJson(response);
    expect(data.access_token).toBeTruthy();
    expect(data.refresh_token).toBeUndefined();
    expect(data.expires_in).toBeLessThanOrEqual(900);
    expect((await request("/api/v1/account", { headers: { Authorization: `Bearer ${data.access_token}` } })).status).toBe(200);
    expect((await request("/api/v1/auth/browser-session", { ...jsonInit({}), headers: { Origin: "https://attacker.example", Cookie: cookie } })).status).toBe(403);
  });

  it("automatically authorizes desktop via browser state, requires PKCE and consumes a code only once", async () => {
    const { cookie, tokens } = await browserLogin();
    const { response, verifier, payload } = await authorizeDesktop(cookie);
    expect(response.status).toBe(200);
    const callback = new URL((await responseJson(response)).redirect_uri);
    expect(callback.origin).toBe("http://127.0.0.1:54321");
    expect(callback.searchParams.get("state")).toBe(payload.state);
    const exchange = { code: callback.searchParams.get("code"), code_verifier: verifier, redirect_uri: payload.redirect_uri };
    expect((await request("/api/v1/auth/desktop/exchange", jsonInit({ ...exchange, code_verifier: "wrong".repeat(13) }))).status).toBe(401);
    expect((await request("/api/v1/auth/desktop/exchange", jsonInit({ ...exchange, redirect_uri: "http://127.0.0.1:54322/callback" }))).status).toBe(401);
    const results = await Promise.all([request("/api/v1/auth/desktop/exchange", jsonInit(exchange)), request("/api/v1/auth/desktop/exchange", jsonInit(exchange))]);
    expect(results.map(response => response.status).sort()).toEqual([200, 401]);
    const desktop = await responseJson(results.find(response => response.status === 200)!);
    expect(desktop.user.id).toBe(tokens.user.id);
    expect(desktop.device.id).not.toBe(tokens.device.id);
    expect(desktop.refresh_token).not.toBe(tokens.refresh_token);
  });

  it("rejects authorization without a signed-in browser and rejects remote redirects", async () => {
    expect((await authorizeDesktop("")).response.status).toBe(401);
    const { cookie } = await browserLogin();
    for (const redirect of ["https://attacker.example/callback", "http://localhost:54321/callback", "http://127.0.0.1:54321@attacker.example/callback", "http://127.0.0.1:54321/callback?next=evil", "http://127.0.0.1:80/callback"]) {
      expect((await authorizeDesktop(cookie, { redirect_uri: redirect })).response.status).toBe(400);
    }
    expect((await authorizeDesktop(cookie, { code_challenge_method: "plain" })).response.status).toBe(400);
  });

  it("rejects expired authorization and a revoked authorising browser session", async () => {
    const { cookie, tokens } = await browserLogin();
    const first = await authorizeDesktop(cookie);
    const firstCode = new URL((await responseJson(first.response)).redirect_uri).searchParams.get("code");
    await env.DB.prepare("UPDATE desktop_authorization_codes SET expires_at = ?").bind("2000-01-01T00:00:00.000Z").run();
    expect((await request("/api/v1/auth/desktop/exchange", jsonInit({ code: firstCode, code_verifier: first.verifier, redirect_uri: first.payload.redirect_uri }))).status).toBe(401);
    const second = await authorizeDesktop(cookie);
    const secondCode = new URL((await responseJson(second.response)).redirect_uri).searchParams.get("code");
    const logout = await request("/api/v1/auth/logout", { ...jsonInit({ refresh_token: tokens.refresh_token }), headers: { Origin: "http://localhost", "Content-Type": "application/json" } });
    expect(logout.headers.get("Set-Cookie")).toContain("Max-Age=0");
    expect((await request("/api/v1/auth/desktop/exchange", jsonInit({ code: secondCode, code_verifier: second.verifier, redirect_uri: second.payload.redirect_uri }))).status).toBe(401);
    expect((await request("/api/v1/auth/browser-session", { ...jsonInit({}), headers: { Origin: "http://localhost", Cookie: cookie } })).status).toBe(401);
  });

  it("shares website identity with desktop and allows the website to revoke only that device", async () => {
    const { login: web } = await loginVerifiedAccount("desktop@example.com");
    const desktopResponse = await request("/api/v1/auth/login", {
      ...jsonInit({ email: "desktop@example.com", password: "correct horse battery staple", device_name: "工作电脑", platform: "windows" }),
      headers: { "Content-Type": "application/json", Origin: "http://tauri.localhost" },
    }, undefined, { CORS_ORIGINS: "http://tauri.localhost" });
    expect(desktopResponse.headers.get("Access-Control-Allow-Origin")).toBe("http://tauri.localhost");
    const desktop = await responseJson(desktopResponse);
    expect(desktop.user.id).toBe(web.user.id);
    expect(desktop.device.id).not.toBe(web.device.id);
    const webHeaders = { Authorization: `Bearer ${web.access_token}` };
    const list = await responseJson(await request("/api/v1/account/devices", { headers: webHeaders }));
    expect(list.devices).toEqual(expect.arrayContaining([expect.objectContaining({ id: desktop.device.id, name: "工作电脑", platform: "windows" })]));
    const renewed = await responseJson(await request("/api/v1/auth/refresh", jsonInit({ refresh_token: desktop.refresh_token })));
    expect(renewed.user.id).toBe(web.user.id);
    expect((await request(`/api/v1/account/devices/${desktop.device.id}`, { method: "DELETE", headers: webHeaders })).status).toBe(204);
    expect((await request("/api/v1/account", { headers: { Authorization: `Bearer ${renewed.access_token}` } })).status).toBe(401);
    expect((await request("/api/v1/auth/refresh", jsonInit({ refresh_token: renewed.refresh_token }))).status).toBe(401);
    expect((await request("/api/v1/account", { headers: webHeaders })).status).toBe(200);
  });

  it("uses a password KDF iteration count supported by Workers", async () => {
    const encoded = await hashPassword("correct horse battery staple");
    expect(encoded.split("$").slice(0, 2)).toEqual(["pbkdf2_sha256", "100000"]);
  });

  it("exposes a health endpoint and reserves the memory bucket binding", async () => {
    const response = await request("/health", { method: "GET" });
    expect(response.status).toBe(200);
    expect(await responseJson(response)).toEqual({
      service: "lingshu-cloud",
      status: "ok",
    });
    expect(env.MEMORY_BUCKET).toBeDefined();
  });

  it("serves each account module as a separate application route", async () => {
    for (const path of ["/account", "/billing", "/devices", "/docs", "/download"]) {
      const response = await request(path, { method: "GET" });
      expect(response.status).toBe(200);
      expect(response.headers.get("Cache-Control")).toBe("no-store");
      expect(await response.text()).toContain("灵枢账户中心");
    }
  });

  it("publishes detailed user documentation with current feature boundaries", async () => {
    const response = await request("/docs", { method: "GET" });
    const html = await response.text();
    expect(html).toContain("注册与邮箱验证");
    expect(html).toContain("设备管理与远程撤销");
    expect(html).toContain("记忆同步：当前尚未开放");
    expect(html).toContain("Get-FileHash -Algorithm SHA256");
    expect(html).toContain("当前尚不提供忘记密码、修改密码、删除账户和记忆同步");
  });

  it("publishes the corrected product editions and enterprise contact entry", async () => {
    const response = await request("/billing", { method: "GET" });
    const html = await response.text();
    expect(html).toContain("专业版 <small>Pro</small>");
    expect(html).toContain("企业版 <small>Enterprise</small>");
    expect(html).toContain("嵌入版 <small>Embedded / OEM</small>");
    expect(html).toContain("mailto:admin@lingeringdream.com?subject=%E7%81%B5%E6%9E%A2%E4%BC%81%E4%B8%9A%E7%89%88%E5%92%A8%E8%AF%A2&amp;body=");
    expect(html).toContain("%E7%A7%B0%E5%91%BC%EF%BC%9A%5B%E8%AF%B7%E5%A1%AB%E5%86%99%5D");
    expect(html).toContain("%E8%81%94%E7%B3%BB%E6%96%B9%E5%BC%8F%EF%BC%9A");
    expect(html).toContain("%E8%81%8C%E4%BD%8D%EF%BC%9A");
    expect(html).toContain("%E5%85%AC%E5%8F%B8%E6%88%96%E7%BB%84%E7%BB%87%EF%BC%9A");
    expect(html).not.toContain("<h3>开发者版</h3>");
  });

  it("publishes the Windows drive download and GitHub fallback", async () => {
    const response = await request("/downloads.json", { method: "GET" });
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
    const manifest = await responseJson(response);
    expect(manifest.version).toBe("1.6.0");
    expect(manifest.github_requires_access).toBe(true);
    expect(manifest.access_code).toBe("cx6i");
    expect(manifest.downloads.windows.url).toBe("https://wwbty.lanzouv.com/b01giajdba");
    expect(manifest.downloads.windows.access_code).toBe("cx6i");
    expect(manifest.downloads.windows.sha256).toBe("8dc04ab5ef71c991f287ac5979c418d77ed8161713f01c3ce707400e59ba7df7");
    expect(manifest.downloads.macos.url).toBe("https://wwbty.lanzouv.com/iM0G646trhod");
    expect(manifest.downloads.linux_deb.url).toBe("https://wwbty.lanzouv.com/i9nDu46uc42j");
    expect(manifest.downloads.linux_deb.access_code).toBe("hrbk");
    expect(manifest.downloads.linux_appimage.url).toBe("https://wwbty.lanzouv.com/iDBQ646uck8b");
    expect(manifest.downloads.linux_appimage.access_code).toBe("gn19");
    expect(manifest.downloads.windows.github_url).toContain("/releases/download/v1.6.0/");
    expect(manifest.downloads.macos.github_url).toContain("/releases/download/v1.5.2/");
  });

  it("keeps registration pending until a one-time hashed email token is verified", async () => {
    let emailMessage: VerificationEmail | undefined;
    const registerResponse = await request(
      "/api/v1/auth/register",
      jsonInit({
        email: "pending@example.com",
        display_name: "Pending User",
        password: "correct horse battery staple",
      }),
      async (message) => {
        emailMessage = message;
      },
    );
    expect(registerResponse.status).toBe(202);
    expect(emailMessage?.verificationUrl).toContain("/?verify=");

    const pending = await env.DB.prepare(
      "SELECT verification_token_hash, password_hash FROM pending_registrations WHERE email = ?",
    )
      .bind("pending@example.com")
      .first<{ verification_token_hash: string; password_hash: string }>();
    expect(pending?.verification_token_hash).not.toBe(emailMessage?.token);
    expect(pending?.password_hash).not.toBe("correct horse battery staple");
    expect(
      await env.DB.prepare("SELECT COUNT(*) AS count FROM users").first<{ count: number }>(),
    ).toMatchObject({ count: 0 });

    const verificationUrl = new URL(emailMessage!.verificationUrl);
    const verifyResponse = await request(
      "/api/v1/auth/verify-email",
      jsonInit({ token: verificationUrl.searchParams.get("verify") }),
    );
    expect(verifyResponse.status).toBe(200);
    expect((await responseJson(verifyResponse)).user.email).toBe("pending@example.com");

    const replayResponse = await request(
      "/api/v1/auth/verify-email",
      jsonInit({ token: emailMessage?.token }),
    );
    expect(replayResponse.status).toBe(400);
  });

  it("does not send a second token for a still-pending registration", async () => {
    const messages: VerificationEmail[] = [];
    const mailer = async (message: VerificationEmail) => {
      messages.push(message);
    };
    await request(
      "/api/v1/auth/register",
      jsonInit({
        email: "same@example.com",
        display_name: "Same User",
        password: "correct horse battery staple",
      }),
      mailer,
    );
    const second = await request(
      "/api/v1/auth/register",
      jsonInit({
        email: "same@example.com",
        display_name: "Changed Name",
        password: "another correct password",
      }),
      mailer,
    );
    expect(second.status).toBe(202);
    expect(messages).toHaveLength(1);
  });

  it("creates device sessions, rotates refresh tokens, and invalidates revoked access", async () => {
    const verified = await registerAndVerify();
    expect(verified.user.email).toBe("alice@example.com");

    const loginResponse = await request(
      "/api/v1/auth/login",
      {
        ...jsonInit({
          email: "alice@example.com",
          password: "correct horse battery staple",
          device_name: "Alice laptop",
          platform: "windows",
        }),
        headers: {
          "Content-Type": "application/json",
          "CF-Connecting-IP": "203.0.113.10",
        },
      },
    );
    expect(loginResponse.status).toBe(200);
    const login = await responseJson(loginResponse);
    expect(login.device.name).toBe("Alice laptop");
    expect(login.access_token).toEqual(expect.any(String));
    expect(login.refresh_token).toEqual(expect.any(String));

    const accountResponse = await request("/api/v1/account", {
      method: "GET",
      headers: { Authorization: `Bearer ${login.access_token}` },
    });
    expect(accountResponse.status).toBe(200);

    const deviceResponse = await request("/api/v1/account/devices", {
      method: "GET",
      headers: { Authorization: `Bearer ${login.access_token}` },
    });
    const deviceData = await responseJson(deviceResponse);
    expect(deviceData.devices).toHaveLength(1);
    expect(deviceData.devices[0].active_session_count).toBe(1);

    const refreshResponse = await request(
      "/api/v1/auth/refresh",
      jsonInit({ refresh_token: login.refresh_token }),
    );
    expect(refreshResponse.status).toBe(200);
    const rotated = await responseJson(refreshResponse);
    expect(rotated.refresh_token).not.toBe(login.refresh_token);

    const oldAccessResponse = await request("/api/v1/account", {
      method: "GET",
      headers: { Authorization: `Bearer ${login.access_token}` },
    });
    expect(oldAccessResponse.status).toBe(401);

    const reuseResponse = await request(
      "/api/v1/auth/refresh",
      jsonInit({ refresh_token: login.refresh_token }),
    );
    expect(reuseResponse.status).toBe(401);

    const familyRevokedResponse = await request("/api/v1/account", {
      method: "GET",
      headers: { Authorization: `Bearer ${rotated.access_token}` },
    });
    expect(familyRevokedResponse.status).toBe(401);
  });

  it("rate-limits failed logins by IP, email, and device", async () => {
    await registerAndVerify("rate-limit@example.com");
    const init = {
      ...jsonInit({
        email: "rate-limit@example.com",
        password: "definitely-the-wrong-password",
        device_id: "550e8400-e29b-41d4-a716-446655440000",
      }),
      headers: {
        "Content-Type": "application/json",
        "CF-Connecting-IP": "203.0.113.20",
      },
    };
    for (let attempt = 0; attempt < 5; attempt += 1) {
      expect((await request("/api/v1/auth/login", init)).status).toBe(401);
    }
    const blocked = await request("/api/v1/auth/login", init);
    expect(blocked.status).toBe(429);
    expect(blocked.headers.get("Retry-After")).toBeTruthy();
  });

  it("supports logout with a refresh token and makes it idempotent", async () => {
    await registerAndVerify("logout@example.com");
    const login = await responseJson(
      await request(
        "/api/v1/auth/login",
        jsonInit({
          email: "logout@example.com",
          password: "correct horse battery staple",
        }),
      ),
    );
    const logoutResponse = await request(
      "/api/v1/auth/logout",
      jsonInit({ refresh_token: login.refresh_token }),
    );
    expect(logoutResponse.status).toBe(204);

    const repeatLogoutResponse = await request(
      "/api/v1/auth/logout",
      jsonInit({ refresh_token: login.refresh_token }),
    );
    expect(repeatLogoutResponse.status).toBe(204);

    const accountResponse = await request("/api/v1/auth/me", {
      method: "GET",
      headers: { Authorization: `Bearer ${login.access_token}` },
    });
    expect(accountResponse.status).toBe(401);
  });

  it("lists devices and lets one active device revoke another device", async () => {
    await registerAndVerify("devices@example.com");
    const firstLogin = await responseJson(
      await request(
        "/api/v1/auth/login",
        jsonInit({
          email: "devices@example.com",
          password: "correct horse battery staple",
          device_name: "Desktop",
          platform: "windows",
        }),
      ),
    );
    const secondLogin = await responseJson(
      await request(
        "/api/v1/auth/login",
        jsonInit({
          email: "devices@example.com",
          password: "correct horse battery staple",
          device_name: "Laptop",
          platform: "macos",
        }),
      ),
    );
    const listed = await responseJson(
      await request("/api/v1/account/devices", {
        method: "GET",
        headers: { Authorization: `Bearer ${firstLogin.access_token}` },
      }),
    );
    expect(listed.devices).toHaveLength(2);
    const laptop = listed.devices.find((device: { name: string }) => device.name === "Laptop");
    expect(laptop?.id).toEqual(secondLogin.device.id);

    const revokeResponse = await request(`/api/v1/account/devices/${laptop.id}`, {
      method: "DELETE",
      headers: { Authorization: `Bearer ${firstLogin.access_token}` },
    });
    expect(revokeResponse.status).toBe(204);

    const revokedAccount = await request("/api/v1/account", {
      method: "GET",
      headers: { Authorization: `Bearer ${secondLogin.access_token}` },
    });
    expect(revokedAccount.status).toBe(401);
    const remainingAccount = await request("/api/v1/account", {
      method: "GET",
      headers: { Authorization: `Bearer ${firstLogin.access_token}` },
    });
    expect(remainingAccount.status).toBe(200);
  });

  it("returns the free plan and keeps checkout safely disabled without Stripe configuration", async () => {
    const { login } = await loginVerifiedAccount("free-plan@example.com");
    const overviewResponse = await request("/api/v1/account/subscription", {
      method: "GET",
      headers: { Authorization: `Bearer ${login.access_token}` },
    });
    expect(overviewResponse.status).toBe(200);
    const overview = await responseJson(overviewResponse);
    expect(overview.subscription).toMatchObject({
      plan_code: "free",
      status: "active",
      entitlement_active: true,
    });
    expect(overview.plans.map((plan: { code: string }) => plan.code)).toEqual([
      "free",
      "developer",
      "team",
    ]);
    expect(overview.plans[1].availability).toBe("upcoming");
    expect(overview.plans[2]).toMatchObject({
      code: "team",
      availability: "planned",
      options: [],
    });
    expect(overview.billing.configured).toBe(false);

    const checkoutResponse = await request(
      "/api/v1/billing/checkout",
      {
        ...jsonInit({ interval: "month" }),
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${login.access_token}`,
        },
      },
    );
    expect(checkoutResponse.status).toBe(503);
    expect((await responseJson(checkoutResponse)).error.code).toBe("BILLING_NOT_CONFIGURED");
  });

  it("recognizes an active team test entitlement", async () => {
    const { verified, login } = await loginVerifiedAccount("team-plan@example.com");
    const now = new Date().toISOString();
    await env.DB.batch([
      env.DB.prepare(
        `INSERT INTO billing_customers
          (user_id, provider, provider_customer_id, created_at, updated_at)
         VALUES (?, 'stripe', ?, ?, ?)`,
      ).bind(verified.user.id, "cus_lingshu_team_test", now, now),
      env.DB.prepare(
        `INSERT INTO subscriptions
          (user_id, provider, provider_customer_id, provider_subscription_id,
           price_id, plan_code, billing_interval, status, current_period_end,
           cancel_at_period_end, created_at, updated_at)
         VALUES (?, 'stripe', ?, ?, NULL, 'team', NULL, 'active', NULL, 0, ?, ?)`,
      ).bind(verified.user.id, "cus_lingshu_team_test", "sub_lingshu_team_test", now, now),
    ]);

    const overview = await responseJson(
      await request("/api/v1/account/subscription", {
        method: "GET",
        headers: { Authorization: `Bearer ${login.access_token}` },
      }),
    );
    expect(overview.subscription).toMatchObject({
      plan_code: "team",
      plan_name: "团队版",
      status: "active",
      entitlement_active: true,
    });
  });

  it("shows an invited free account as Team without granting team creation", async () => {
    const email = "invited-free-plan@example.com";
    const { verified, login } = await loginVerifiedAccount(email);
    const teamId = crypto.randomUUID();
    const now = new Date().toISOString();
    await env.DB.batch([
      env.DB.prepare("INSERT INTO team_workspaces (id, name, created_at) VALUES (?, 'Invited team', ?)")
        .bind(teamId, now),
      env.DB.prepare("INSERT INTO team_members (team_id, cloud_user_id, email, role, created_at) VALUES (?, ?, ?, 'editor', ?)")
        .bind(teamId, verified.user.id, email, now),
    ]);

    const overview = await responseJson(await request("/api/v1/account/subscription", {
      method: "GET",
      headers: { Authorization: `Bearer ${login.access_token}` },
    }));
    expect(overview.subscription).toMatchObject({ plan_code: "free" });
    expect(overview.entitlements).toMatchObject({
      effective_plan: "team",
      access_source: "team_membership",
    });
    expect(overview.entitlements.features.find((feature: { code: string }) => feature.code === "team_create"))
      .toMatchObject({ included: false, enabled: false });
  });

  it("creates a hosted Stripe subscription Checkout session for an authenticated user", async () => {
    const { login } = await loginVerifiedAccount("checkout@example.com");
    const calls: Array<{ url: string; body: string }> = [];
    const stripeFetch: StripeFetch = async (input, init) => {
      const url = String(input);
      calls.push({ url, body: String(init?.body || "") });
      if (url.endsWith("/v1/customers")) {
        return Response.json({ id: "cus_lingshu_test" });
      }
      return Response.json({
        id: "cs_test_lingshu",
        url: "https://checkout.stripe.com/c/pay/cs_test_lingshu",
      });
    };
    const response = await request(
      "/api/v1/billing/checkout",
      {
        ...jsonInit({ interval: "month" }),
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${login.access_token}`,
        },
      },
      undefined,
      {
        STRIPE_SECRET_KEY: "sk_test_lingshu",
        STRIPE_PRICE_DEVELOPER_MONTHLY: "price_developer_monthly",
      },
      stripeFetch,
    );
    expect(response.status).toBe(200);
    expect((await responseJson(response)).url).toContain("checkout.stripe.com");
    expect(calls).toHaveLength(2);
    expect(calls[1].body).toContain("mode=subscription");
    expect(calls[1].body).toContain("line_items%5B0%5D%5Bprice%5D=price_developer_monthly");
    expect(calls[1].body).toContain("%2Fbilling%3Fbilling%3Dsuccess");
  });

  it("verifies and idempotently applies Stripe subscription webhooks", async () => {
    const webhookSecret = "whsec_lingshu_test_secret";
    const { verified, login } = await loginVerifiedAccount("webhook@example.com");
    const event = {
      id: "evt_lingshu_subscription_updated",
      type: "customer.subscription.updated",
      created: Math.floor(Date.now() / 1_000),
      data: {
        object: {
          id: "sub_lingshu_test",
          customer: "cus_lingshu_webhook",
          status: "active",
          cancel_at_period_end: false,
          metadata: {
            user_id: verified.user.id,
            plan_code: "developer",
            billing_interval: "month",
          },
          items: {
            data: [
              {
                current_period_end: Math.floor(Date.now() / 1_000) + 2_592_000,
                price: {
                  id: "price_developer_monthly",
                  recurring: { interval: "month" },
                },
              },
            ],
          },
        },
      },
    };
    const payload = JSON.stringify(event);
    const signature = await stripeSignature(payload, webhookSecret);
    const webhookInit = {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Stripe-Signature": signature,
      },
      body: payload,
    };
    const billingEnv = {
      STRIPE_WEBHOOK_SECRET: webhookSecret,
      STRIPE_SECRET_KEY: "sk_test_lingshu",
      STRIPE_PRICE_DEVELOPER_MONTHLY: "price_developer_monthly",
    };
    const first = await request(
      "/api/v1/billing/webhook",
      webhookInit,
      undefined,
      billingEnv,
    );
    expect(first.status).toBe(200);
    expect(await responseJson(first)).toEqual({ received: true });

    const duplicate = await request(
      "/api/v1/billing/webhook",
      webhookInit,
      undefined,
      billingEnv,
    );
    expect(await responseJson(duplicate)).toEqual({ received: true, duplicate: true });

    const overview = await responseJson(
      await request(
        "/api/v1/account/subscription",
        {
          method: "GET",
          headers: { Authorization: `Bearer ${login.access_token}` },
        },
        undefined,
        billingEnv,
      ),
    );
    expect(overview.subscription).toMatchObject({
      plan_code: "developer",
      status: "active",
      billing_interval: "month",
      entitlement_active: true,
    });
    expect(overview.billing.customer_portal_available).toBe(true);
  });

  it("rejects Stripe webhook payloads with an invalid signature", async () => {
    const response = await request(
      "/api/v1/billing/webhook",
      {
        method: "POST",
        headers: { "Stripe-Signature": "t=1,v1=invalid" },
        body: JSON.stringify({ id: "evt_invalid", type: "customer.subscription.updated" }),
      },
      undefined,
      { STRIPE_WEBHOOK_SECRET: "whsec_lingshu_test_secret" },
    );
    expect(response.status).toBe(400);
    expect((await responseJson(response)).error.code).toBe("INVALID_WEBHOOK_SIGNATURE");
  });

  it("does not grant developer entitlement for an unconfigured Stripe Price", async () => {
    const webhookSecret = "whsec_lingshu_unknown_price";
    const { verified, login } = await loginVerifiedAccount("unknown-price@example.com");
    const event = {
      id: "evt_lingshu_unknown_price",
      type: "customer.subscription.created",
      created: Math.floor(Date.now() / 1_000),
      data: {
        object: {
          id: "sub_unknown_price",
          customer: "cus_unknown_price",
          status: "active",
          metadata: { user_id: verified.user.id, plan_code: "developer" },
          items: {
            data: [{ price: { id: "price_not_allowlisted", recurring: { interval: "month" } } }],
          },
        },
      },
    };
    const payload = JSON.stringify(event);
    const response = await request(
      "/api/v1/billing/webhook",
      {
        method: "POST",
        headers: { "Stripe-Signature": await stripeSignature(payload, webhookSecret) },
        body: payload,
      },
      undefined,
      {
        STRIPE_WEBHOOK_SECRET: webhookSecret,
        STRIPE_SECRET_KEY: "sk_test_lingshu",
        STRIPE_PRICE_DEVELOPER_MONTHLY: "price_developer_monthly",
      },
    );
    expect(response.status).toBe(200);

    const overview = await responseJson(
      await request(
        "/api/v1/account/subscription",
        {
          method: "GET",
          headers: { Authorization: `Bearer ${login.access_token}` },
        },
        undefined,
        { STRIPE_SECRET_KEY: "sk_test_lingshu" },
      ),
    );
    expect(overview.subscription).toMatchObject({
      plan_code: "unrecognized",
      entitlement_active: false,
    });
  });
});

describe("team page gate", () => {
  const TEAM_REQUIRED = "http://localhost/billing?team=required";

  async function signIn(email: string) {
    await registerAndVerify(email);
    const response = await request("/api/v1/auth/login", {
      ...jsonInit({ email, password: "correct horse battery staple", device_name: "Gate browser", platform: "web" }),
      headers: { Origin: "http://localhost", "Content-Type": "application/json" },
    });
    const tokens = await responseJson(response);
    const account = await responseJson(
      await request("/api/v1/account", { method: "GET", headers: { Authorization: `Bearer ${tokens.access_token}` } }),
    );
    return { accessToken: tokens.access_token, cookie: response.headers.get("Set-Cookie")!.split(";")[0], userId: account.user.id as string };
  }

  // subscriptions.provider_customer_id is a FK to billing_customers, so the
  // customer row has to exist first.
  async function grantPlan(userId: string, planCode: string, status = "active", periodEnd: string | null = null) {
    const now = new Date().toISOString();
    const customerId = `cus_${userId}`;
    await env.DB.prepare(
      `INSERT INTO billing_customers (user_id, provider, provider_customer_id, created_at, updated_at)
       VALUES (?, 'stripe', ?, ?, ?)
       ON CONFLICT(user_id) DO UPDATE SET
         provider_customer_id = excluded.provider_customer_id, updated_at = excluded.updated_at`,
    ).bind(userId, customerId, now, now).run();
    await env.DB.prepare(
      `INSERT INTO subscriptions
         (user_id, provider, provider_customer_id, provider_subscription_id,
          price_id, plan_code, billing_interval, status, current_period_end,
          cancel_at_period_end, created_at, updated_at)
       VALUES (?, 'stripe', ?, ?, NULL, ?, 'month', ?, ?, 0, ?, ?)
       ON CONFLICT(user_id) DO UPDATE SET
         plan_code = excluded.plan_code, status = excluded.status,
         current_period_end = excluded.current_period_end, updated_at = excluded.updated_at`,
    ).bind(userId, customerId, `sub_${userId}_${planCode}`, planCode, status, periodEnd, now, now).run();
  }

  it.each([
    ["free", "active", null, 403],
    ["pro", "active", null, 403],
    ["enterprise", "active", null, 403],
    ["team", "active", null, 201],
    ["team", "active", "2020-01-01T00:00:00Z", 403],
    ["embedded", "active", null, 403],
  ] as const)("enforces %s %s %s at the creation API", async (plan, status, end, expected) => {
    const { accessToken, userId } = await signIn(`policy-${plan}@example.com`);
    await grantPlan(userId, plan, status, end);
    const response = await request("/api/v1/teams", {
      ...jsonInit({ name: "Policy team", plan_code: "enterprise", entitlement_active: true }),
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${accessToken}` },
    });
    expect(response.status).toBe(expected);
    const overview = await responseJson(await request("/api/v1/account/subscription", {
      headers: { Authorization: `Bearer ${accessToken}` },
    }));
    expect(overview.entitlements.features.find((f: { code: string }) => f.code === "team_create").enabled).toBe(expected === 201);
  });

  function grantTeamPlan(userId: string, status = "active", periodEnd: string | null = null) {
    return grantPlan(userId, "team", status, periodEnd);
  }

  async function makeTeamMember(userId: string, email: string) {
    const teamId = crypto.randomUUID();
    const now = new Date().toISOString();
    await env.DB.batch([
      env.DB.prepare("INSERT INTO team_workspaces (id, name, created_at) VALUES (?, 'Gate team', ?)").bind(teamId, now),
      env.DB.prepare("INSERT INTO team_members (team_id, cloud_user_id, email, role, created_at) VALUES (?, ?, ?, 'editor', ?)").bind(teamId, userId, email, now),
    ]);
  }

  it("redirects a signed-out visitor to billing", async () => {
    const response = await request("/team");
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(TEAM_REQUIRED);
  });

  it("redirects a visitor with a forged cookie", async () => {
    const response = await request("/team", { headers: { Cookie: "__Host-lingshu_browser=not.a.real.token" } });
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(TEAM_REQUIRED);
  });

  it("redirects a free-plan account", async () => {
    const { cookie } = await signIn("gate-free@example.com");
    const response = await request("/team", { headers: { Cookie: cookie } });
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(TEAM_REQUIRED);
  });

  it("redirects a developer-plan account", async () => {
    const { cookie, userId } = await signIn("gate-dev@example.com");
    await grantPlan(userId, "developer");
    const response = await request("/team", { headers: { Cookie: cookie } });
    expect(response.headers.get("Location")).toBe(TEAM_REQUIRED);
  });

  it("admits an active team plan", async () => {
    const { cookie, userId } = await signIn("gate-team@example.com");
    await grantTeamPlan(userId);
    const response = await request("/team", { headers: { Cookie: cookie } });
    expect(response.status).toBe(200);
    expect(response.headers.get("Location")).toBeNull();
  });

  it("admits an invited member who is still on the free plan", async () => {
    const email = "gate-member@example.com";
    const { cookie, userId } = await signIn(email);
    await makeTeamMember(userId, email);
    const response = await request("/team", { headers: { Cookie: cookie } });
    expect(response.status).toBe(200);
    expect(response.headers.get("Location")).toBeNull();
  });

  it("redirects when the team plan lapsed", async () => {
    const { cookie, userId } = await signIn("gate-lapsed@example.com");
    const past = new Date(Date.now() - 86_400_000).toISOString();
    await grantTeamPlan(userId, "past_due", past);
    const response = await request("/team", { headers: { Cookie: cookie } });
    expect(response.headers.get("Location")).toBe(TEAM_REQUIRED);
  });

  it("gates /team/index.html as well", async () => {
    const response = await request("/team/index.html");
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(TEAM_REQUIRED);
  });

  it("leaves other account routes public", async () => {
    for (const path of ["/", "/account", "/billing", "/devices", "/docs", "/download"]) {
      expect((await request(path)).headers.get("Location"), path).toBeNull();
    }
  });
});

describe('encrypted memory sync API', () => {
  async function client(email = 'sync@example.com', paid = true) {
    const { login } = await loginVerifiedAccount(email);
    const user = login.user.id;
    if (paid) {
      const now = new Date().toISOString();
      await env.DB.prepare("INSERT INTO billing_customers VALUES (?, 'stripe', ?, ?, ?)").bind(user, `cus_${user}`, now, now).run();
      await env.DB.prepare(`INSERT INTO subscriptions (user_id, provider, provider_customer_id, provider_subscription_id, plan_code, status, created_at, updated_at)
        VALUES (?, 'stripe', ?, ?, 'pro', 'active', ?, ?)`).bind(user, `cus_${user}`, `sub_${user}`, now, now).run();
    }
    return { user, login, call: (path: string, body?: unknown) => request('/api/v1/sync/' + path, {
      ...(body === undefined ? {} : jsonInit(body)), headers: { Authorization: `Bearer ${login.access_token}`, 'Content-Type': 'application/json', Origin: 'http://localhost:5173' },
    }) };
  }
  const id = '11111111-1111-4111-8111-111111111111';
  const keyId = 'a'.repeat(64);
  const envelope = { version: 1, key_id: keyId, iv: 'AAAAAAAAAAAAAAAA', ciphertext: 'AAAAAAAAAAAAAAAAAAAAAA==' };
  it('requires authenticated, active entitlements for every read and write', async () => {
    expect((await request('/api/v1/sync/vault')).status).toBe(401);
    const free = await client('free-sync@example.com', false);
    expect((await free.call('vault', { key_id: keyId, plan_code: 'pro' })).status).toBe(403);
    const pro = await client();
    expect((await pro.call('vault', { key_id: keyId })).status).toBe(200);
    await env.DB.prepare("UPDATE subscriptions SET current_period_end = '2020-01-01T00:00:00Z' WHERE user_id = ?").bind(pro.user).run();
    for (const path of ['vault', 'records', `records/${id}/history`]) expect((await pro.call(path)).status).toBe(403);
    expect((await pro.call(`records/${id}`, { base_revision: 0, envelope })).status).toBe(403);
  });
  it('isolates vaults, pins the recovery key, validates envelopes and applies atomic version checks', async () => {
    const a = await client(), b = await client('other-sync@example.com');
    expect((await a.call('vault', { key_id: keyId })).status).toBe(200);
    expect((await b.call('vault', { key_id: keyId })).status).toBe(200);
    expect((await a.call('vault', { key_id: 'b'.repeat(64) })).status).toBe(409);
    expect((await a.call(`records/${id}`, { base_revision: 0, envelope: { ...envelope, iv: 'bad' } })).status).toBe(400);
    const attempts = await Promise.all([1, 2].map(() => a.call(`records/${id}`, { base_revision: 0, envelope })));
    expect(attempts.map(r => r.status).sort()).toEqual([200, 409]);
    expect((await b.call(`records/${id}`)).status).toBe(404);
    expect((await responseJson(await b.call('records'))).records).toEqual([]);
    expect((await a.call(`records/${id}`, { base_revision: 0, envelope })).status).toBe(409);
    const manifest = await responseJson(await a.call('records'));
    expect(manifest.records[0]).toMatchObject({ id, revision: 1 });
    expect(manifest.records[0].envelope).toBeUndefined();
    expect((await responseJson(await a.call(`records/${id}`))).envelope).toEqual(envelope);
    expect((await a.call('records?after=invalid')).status).toBe(400);
    expect((await a.call(`records/${id}`, { base_revision: 1, envelope: { ...envelope, ciphertext: 'A'.repeat(48004) } })).status).toBe(400);
    expect((await a.call('records')).headers.get('Access-Control-Allow-Origin')).toBe('http://localhost:5173');
  });
  it('retains only ten previous encrypted revisions and rejects revoked devices', async () => {
    const a = await client();
    await a.call('vault', { key_id: keyId });
    for (let base_revision = 0; base_revision < 13; base_revision++) {
      expect((await a.call(`records/${id}`, { base_revision, envelope })).status).toBe(200);
    }
    const history = await responseJson(await a.call(`records/${id}/history`));
    expect(history.records.map((r: {revision: number}) => r.revision)).toEqual([12, 11, 10, 9, 8, 7, 6, 5, 4, 3]);
    expect(history.records.every((r: {envelope: unknown}) => JSON.stringify(r.envelope) === JSON.stringify(envelope))).toBe(true);
    await env.DB.prepare('UPDATE devices SET revoked_at = ? WHERE id = ?').bind(new Date().toISOString(), a.login.device.id).run();
    expect((await a.call('records')).status).toBe(401);
  });
});
