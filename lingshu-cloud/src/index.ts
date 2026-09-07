import { canCreateTeam } from "./entitlements";
import {
  burnPasswordKdf,
  hashPassword,
  randomToken,
  sha256Base64Url,
  signAccessToken,
  verifyAccessToken,
  verifyPassword,
} from "./crypto";
import {
  createCheckoutSession,
  createPortalSession,
  getBillingOverview,
  processStripeWebhook,
  type StripeFetch,
} from "./billing";
import { sendVerificationEmail } from "./email";
import { HttpError, isHttpError } from "./errors";
import { teamRequest } from "./teams";
import { requireSync, syncRequest } from "./sync";
import type {
  AppEnv,
  AuthContext,
  Mailer,
  PublicUser,
} from "./types";

const GENERIC_REGISTRATION_MESSAGE =
  "If this address is eligible, a verification email has been sent.";
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

interface PendingRegistrationRow {
  id: string;
  email: string;
  display_name: string;
  password_hash: string;
  verification_token_hash: string;
  token_expires_at: string;
  consumed_at: string | null;
  created_at: string;
  updated_at: string;
}

interface UserRow {
  id: string;
  email: string;
  display_name: string;
  password_hash: string;
  email_verified_at: string;
  last_login_at: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
}

interface DeviceRow {
  id: string;
  user_id: string;
  name: string;
  platform: string;
  created_at: string;
  last_seen_at: string;
  revoked_at: string | null;
}

interface SessionRow {
  id: string;
  user_id: string;
  device_id: string;
  family_id: string;
  refresh_token_hash: string;
  created_at: string;
  last_used_at: string;
  expires_at: string;
  revoked_at: string | null;
  replaced_by: string | null;
  email: string;
  display_name: string;
  email_verified_at: string;
  user_created_at: string;
  device_name: string;
  platform: string;
  device_created_at: string;
  device_last_seen_at: string;
  device_revoked_at: string | null;
  user_deleted_at: string | null;
}

interface AuthSessionRow {
  id: string;
  user_id: string;
  device_id: string;
  expires_at: string;
  email: string;
  display_name: string;
}

interface RateLimitRow {
  window_started_at: string;
  attempts: number;
  blocked_until: string | null;
}

interface LoginDeviceInput {
  id?: string;
  name: string;
  platform: string;
  fingerprint: string;
}

interface TokenResponseOptions {
  sessionId: string;
  refreshToken: string;
  refreshExpiresAt: string;
  user: UserRow;
  device: DeviceRow;
  env: AppEnv;
}

function nowIso(): string {
  return new Date().toISOString();
}

function configNumber(
  raw: string | undefined,
  fallback: number,
  minimum: number,
  maximum: number,
): number {
  const parsed = Number(raw);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    return fallback;
  }
  return parsed;
}

function accessTokenTtl(env: AppEnv): number {
  return configNumber(env.ACCESS_TOKEN_TTL_SECONDS, 900, 60, 3_600);
}

function refreshTokenTtl(env: AppEnv): number {
  return configNumber(env.REFRESH_TOKEN_TTL_SECONDS, 2_592_000, 3_600, 31_536_000);
}

function verificationTokenTtl(env: AppEnv): number {
  return configNumber(env.VERIFICATION_TOKEN_TTL_SECONDS, 86_400, 300, 604_800);
}

function loginWindowSeconds(env: AppEnv): number {
  return configNumber(env.LOGIN_WINDOW_SECONDS, 900, 60, 86_400);
}

function loginMaxAttempts(env: AppEnv): number {
  return configNumber(env.LOGIN_MAX_ATTEMPTS, 5, 1, 100);
}

function loginBlockSeconds(env: AppEnv): number {
  return configNumber(env.LOGIN_BLOCK_SECONDS, 900, 60, 86_400);
}

function maxBodyBytes(env: AppEnv): number {
  return configNumber(env.MAX_BODY_BYTES, 65_536, 1_024, 1_048_576);
}

function requireJwtSecret(env: AppEnv): string {
  const secret = env.JWT_SECRET?.trim();
  if (!secret || secret.length < 32) {
    throw new HttpError(
      500,
      "JWT secret is not configured",
      "JWT_SECRET_NOT_CONFIGURED",
    );
  }
  return secret;
}

function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value);
}

function normalizeEmail(value: unknown): string {
  if (typeof value !== "string") {
    throw new HttpError(400, "A valid email is required", "INVALID_EMAIL");
  }
  const email = value.trim().toLowerCase();
  if (email.length > 254 || !EMAIL_PATTERN.test(email)) {
    throw new HttpError(400, "A valid email is required", "INVALID_EMAIL");
  }
  return email;
}

function requiredString(
  body: Record<string, unknown>,
  field: string,
  maximum: number,
): string {
  const value = body[field];
  if (typeof value !== "string" || value.length === 0 || value.length > maximum) {
    throw new HttpError(400, `Invalid ${field}`, "INVALID_INPUT");
  }
  return value;
}

function optionalString(
  body: Record<string, unknown>,
  field: string,
  maximum: number,
): string | undefined {
  const value = body[field];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string" || value.length > maximum) {
    throw new HttpError(400, `Invalid ${field}`, "INVALID_INPUT");
  }
  return value;
}

function normalizeDisplayName(value: unknown): string {
  if (typeof value !== "string") {
    throw new HttpError(400, "A display name is required", "INVALID_DISPLAY_NAME");
  }
  const displayName = value.trim();
  if (displayName.length === 0 || displayName.length > 100) {
    throw new HttpError(400, "A display name is required", "INVALID_DISPLAY_NAME");
  }
  return displayName;
}

function registrationPassword(value: unknown): string {
  if (typeof value !== "string" || value.length < 12 || value.length > 128) {
    throw new HttpError(
      400,
      "Password must be between 12 and 128 characters",
      "INVALID_PASSWORD",
    );
  }
  return value;
}

function loginPassword(value: unknown): string {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) {
    throw new HttpError(400, "Invalid password", "INVALID_PASSWORD");
  }
  return value;
}

function publicUser(user: UserRow): PublicUser {
  return {
    id: user.id,
    email: user.email,
    display_name: user.display_name,
    email_verified_at: user.email_verified_at,
    created_at: user.created_at,
  };
}

function requiredDeviceId(value: string | undefined): string | undefined {
  if (value === undefined) return undefined;
  if (!isUuid(value)) {
    throw new HttpError(400, "Invalid device_id", "INVALID_DEVICE_ID");
  }
  return value;
}

function getDeviceInput(
  request: Request,
  body: Record<string, unknown>,
): LoginDeviceInput {
  const rawId = optionalString(body, "device_id", 64);
  const id = requiredDeviceId(rawId);
  const name = optionalString(body, "device_name", 100)?.trim() || "Unknown device";
  const platform = optionalString(body, "platform", 50)?.trim() || "unknown";
  const headerId = request.headers.get("X-LingShu-Device-ID")?.trim();
  const fingerprint = id || headerId || `${platform}:${name}`;
  return { id, name, platform, fingerprint };
}

function clientIp(request: Request): string {
  const cloudflareIp = request.headers.get("CF-Connecting-IP")?.trim();
  if (cloudflareIp) return cloudflareIp;
  const forwarded = request.headers.get("X-Forwarded-For")?.split(",")[0]?.trim();
  return forwarded || "unknown";
}

interface RateLimitKey {
  type: "ip" | "email" | "device";
  value: string;
}

function loginRateLimitKeys(
  request: Request,
  email: string,
  device: LoginDeviceInput,
): RateLimitKey[] {
  return [
    { type: "ip", value: clientIp(request) },
    { type: "email", value: email },
    { type: "device", value: device.fingerprint },
  ];
}

async function hashedRateLimitKey(key: RateLimitKey): Promise<string> {
  return sha256Base64Url(`${key.type}:${key.value}`);
}

function changesOf(result: { meta?: { changes?: number }} | undefined): number {
  return result?.meta?.changes ?? 0;
}

async function readJson(
  request: Request,
  env: AppEnv,
): Promise<Record<string, unknown>> {
  const maximum = maxBodyBytes(env);
  const contentLength = Number(request.headers.get("Content-Length"));
  if (Number.isFinite(contentLength) && contentLength > maximum) {
    throw new HttpError(413, "Request body is too large", "BODY_TOO_LARGE");
  }

  const text = await request.text();
  if (new TextEncoder().encode(text).byteLength > maximum) {
    throw new HttpError(413, "Request body is too large", "BODY_TOO_LARGE");
  }
  if (text.trim().length === 0) return {};

  try {
    const value: unknown = JSON.parse(text);
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      throw new Error("not an object");
    }
    return value as Record<string, unknown>;
  } catch {
    throw new HttpError(400, "Invalid JSON body", "INVALID_JSON");
  }
}

function allowedOrigins(env: AppEnv): Set<string> {
  return new Set(
    (env.CORS_ORIGINS || "")
      .split(",")
      .map((origin) => origin.trim())
      .filter(Boolean),
  );
}

function securityHeaders(request: Request, env: AppEnv): Headers {
  const headers = new Headers({
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
    "Referrer-Policy": "no-referrer",
  });
  const origin = request.headers.get("Origin");
  if (origin && (allowedOrigins(env).has(origin) || allowedOrigins(env).has("*"))) {
    headers.set("Access-Control-Allow-Origin", origin);
    headers.set("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS");
    headers.set("Access-Control-Allow-Headers", "Authorization, Content-Type, X-LingShu-Device-ID");
    headers.set("Access-Control-Expose-Headers", "Retry-After");
    headers.set("Vary", "Origin");
  }
  return headers;
}

function jsonResponse(
  request: Request,
  env: AppEnv,
  status: number,
  value: unknown,
  extraHeaders: Record<string, string> = {},
): Response {
  const headers = securityHeaders(request, env);
  headers.set("Content-Type", "application/json; charset=utf-8");
  for (const [key, headerValue] of Object.entries(extraHeaders)) {
    headers.set(key, headerValue);
  }
  return new Response(JSON.stringify(value), { status, headers });
}

function emptyResponse(
  request: Request,
  env: AppEnv,
  status = 204,
): Response {
  return new Response(null, { status, headers: securityHeaders(request, env) });
}

async function assetResponse(request: Request, env: AppEnv): Promise<Response> {
  if (!env.ASSETS) {
    throw new HttpError(404, "Not found", "NOT_FOUND");
  }
  const requestUrl = new URL(request.url);
  const appRoutes = new Set(["/account", "/billing", "/devices", "/docs", "/download", "/team"]);
  // Deep-link target for team invites sent via email: `/team/invite/<uuid>`.
  // Falling back to "/" hands the URL over to the account-center SPA so the
  // "Pending invites" banner can surface the matching row after sign-in.
  const cleanPath = requestUrl.pathname.replace(/\/$/, "") || "/";
  const isInviteDeepLink = /^\/team\/invite\/[0-9a-f-]{36}$/i.test(cleanPath);
  const isAppRoute = appRoutes.has(cleanPath) || isInviteDeepLink;
  const assetRequest = isAppRoute
    ? new Request(new URL("/", requestUrl), request)
    : request;
  const asset = await env.ASSETS.fetch(assetRequest);
  const headers = new Headers(asset.headers);
  for (const [name, value] of securityHeaders(request, env)) {
    headers.set(name, value);
  }
  headers.set(
    "Content-Security-Policy",
    "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
  );
  const path = requestUrl.pathname;
  if (/\.(?:css|js)$/.test(path)) {
    headers.set("Cache-Control", "public, max-age=0, must-revalidate");
  } else if (/\.(?:png|svg)$/.test(path)) {
    headers.set("Cache-Control", "public, max-age=3600");
  } else if (path === "/" || appRoutes.has(path.replace(/\/$/, "")) || path.endsWith(".json")) {
    headers.set("Cache-Control", "no-store");
  }
  return new Response(asset.body, {
    status: asset.status,
    statusText: asset.statusText,
    headers,
  });
}

// The team console is a static asset, so without this gate anyone (including
// signed-out visitors) could load its UI by typing the URL directly. Data was
// never exposed because /api/v1/teams enforces its own checks; this keeps the
// unreleased interface private too.
async function teamPageGate(request: Request, env: AppEnv): Promise<Response> {
  const identity = await cookieIdentity(request, env);
  if (!identity || !(await canAccessTeam(env, identity.userId))) {
    return Response.redirect(new URL("/billing?team=required", request.url).toString(), 302);
  }
  return assetResponse(request, env);
}

function errorResponse(
  request: Request,
  env: AppEnv,
  error: HttpError,
): Response {
  return jsonResponse(
    request,
    env,
    error.status,
    { error: { code: error.code, message: error.message } },
    error.headers,
  );
}

function buildVerificationUrl(env: AppEnv, token: string): string {
  const baseUrl = env.APP_BASE_URL?.trim() || "http://localhost:8787";
  try {
    const url = new URL("/", baseUrl);
    url.searchParams.set("verify", token);
    return url.toString();
  } catch {
    throw new HttpError(
      500,
      "Application base URL is invalid",
      "INVALID_APP_BASE_URL",
    );
  }
}

async function register(
  request: Request,
  env: AppEnv,
  mailer: Mailer,
): Promise<Response> {
  const body = await readJson(request, env);
  const email = normalizeEmail(body.email);
  const displayName = normalizeDisplayName(body.display_name ?? body.displayName);
  const password = registrationPassword(body.password);

  const existingUser = await env.DB.prepare(
    "SELECT id FROM users WHERE email = ? AND deleted_at IS NULL",
  )
    .bind(email)
    .first<{ id: string }>();
  if (existingUser) {
    return jsonResponse(request, env, 202, { message: GENERIC_REGISTRATION_MESSAGE });
  }

  const existingPending = await env.DB.prepare(
    "SELECT * FROM pending_registrations WHERE email = ?",
  )
    .bind(email)
    .first<PendingRegistrationRow>();
  const now = nowIso();
  if (
    existingPending &&
    !existingPending.consumed_at &&
    Date.parse(existingPending.token_expires_at) > Date.now()
  ) {
    return jsonResponse(request, env, 202, { message: GENERIC_REGISTRATION_MESSAGE });
  }

  const pendingId = crypto.randomUUID();
  const token = randomToken();
  const tokenHash = await sha256Base64Url(token);
  const passwordHash = await hashPassword(password);
  const expiresAt = new Date(Date.now() + verificationTokenTtl(env) * 1_000).toISOString();

  const result = await env.DB.prepare(
    `INSERT INTO pending_registrations
      (id, email, display_name, password_hash, verification_token_hash,
       token_expires_at, consumed_at, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)
     ON CONFLICT(email) DO UPDATE SET
       id = excluded.id,
       display_name = excluded.display_name,
       password_hash = excluded.password_hash,
       verification_token_hash = excluded.verification_token_hash,
       token_expires_at = excluded.token_expires_at,
       consumed_at = NULL,
       created_at = excluded.created_at,
       updated_at = excluded.updated_at
     WHERE pending_registrations.token_expires_at <= ?`,
  )
    .bind(
      pendingId,
      email,
      displayName,
      passwordHash,
      tokenHash,
      expiresAt,
      now,
      now,
      now,
    )
    .run();

  // A concurrent request may have created a still-valid pending record. Do
  // not send a second token or reveal which request won the race.
  if (changesOf(result) === 0) {
    return jsonResponse(request, env, 202, { message: GENERIC_REGISTRATION_MESSAGE });
  }

  try {
    await mailer(
      {
        to: email,
        displayName: displayName,
        token,
        verificationUrl: buildVerificationUrl(env, token),
      },
      env,
    );
  } catch (error) {
    // Do not leave an unusable pending credential if delivery failed. The
    // conditional delete cannot remove a newer concurrent registration.
    await env.DB.prepare(
      "DELETE FROM pending_registrations WHERE id = ? AND verification_token_hash = ?",
    )
      .bind(pendingId, tokenHash)
      .run();
    throw error;
  }

  return jsonResponse(request, env, 202, { message: GENERIC_REGISTRATION_MESSAGE });
}

async function verifyEmail(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const url = new URL(request.url);
  const body = request.method === "POST" ? await readJson(request, env) : {};
  const token =
    request.method === "GET"
      ? url.searchParams.get("token") || ""
      : requiredString(body, "token", 128);
  if (!/^[A-Za-z0-9_-]{20,128}$/.test(token)) {
    throw new HttpError(400, "Invalid or expired verification token", "INVALID_TOKEN");
  }

  const tokenHash = await sha256Base64Url(token);
  const pending = await env.DB.prepare(
    "SELECT * FROM pending_registrations WHERE verification_token_hash = ?",
  )
    .bind(tokenHash)
    .first<PendingRegistrationRow>();
  if (
    !pending ||
    pending.consumed_at ||
    Date.parse(pending.token_expires_at) <= Date.now()
  ) {
    throw new HttpError(400, "Invalid or expired verification token", "INVALID_TOKEN");
  }

  const userId = crypto.randomUUID();
  const now = nowIso();
  const results = await env.DB.batch([
    env.DB.prepare(
      `UPDATE pending_registrations
       SET consumed_at = ?, updated_at = ?
       WHERE id = ? AND verification_token_hash = ?
         AND consumed_at IS NULL AND token_expires_at > ?`,
    ).bind(now, now, pending.id, tokenHash, now),
    env.DB.prepare(
      `INSERT INTO users
        (id, email, display_name, password_hash, email_verified_at,
         created_at, updated_at)
       SELECT ?, email, display_name, password_hash, ?, created_at, ?
       FROM pending_registrations
       WHERE id = ? AND consumed_at = ?
       ON CONFLICT(email) DO NOTHING`,
    ).bind(userId, now, now, pending.id, now),
    env.DB.prepare(
      "DELETE FROM pending_registrations WHERE id = ? AND consumed_at = ?",
    ).bind(pending.id, now),
  ]);

  if (changesOf(results[0]) !== 1) {
    throw new HttpError(400, "Invalid or expired verification token", "INVALID_TOKEN");
  }

  const user = await env.DB.prepare(
    "SELECT * FROM users WHERE email = ? AND deleted_at IS NULL",
  )
    .bind(pending.email)
    .first<UserRow>();
  if (!user) {
    throw new HttpError(500, "Account activation failed", "ACCOUNT_ACTIVATION_FAILED");
  }

  return jsonResponse(request, env, 200, {
    message: "Email verified",
    user: publicUser(user),
  });
}

async function rateLimitStatus(
  env: AppEnv,
  keys: RateLimitKey[],
): Promise<number> {
  const hashes = await Promise.all(keys.map(hashedRateLimitKey));
  const rows = await Promise.all(
    hashes.map((hash) =>
      env.DB.prepare(
        "SELECT window_started_at, attempts, blocked_until FROM login_attempts WHERE key_type = ? AND key_hash = ?",
      )
        .bind(keys[hashes.indexOf(hash)].type, hash)
        .first<RateLimitRow>(),
    ),
  );
  const now = Date.now();
  let retryAfter = 0;
  for (const row of rows) {
    if (!row?.blocked_until) continue;
    const remaining = Math.ceil((Date.parse(row.blocked_until) - now) / 1_000);
    retryAfter = Math.max(retryAfter, remaining);
  }
  return Math.max(0, retryAfter);
}

async function recordLoginFailure(
  env: AppEnv,
  keys: RateLimitKey[],
): Promise<void> {
  const hashes = await Promise.all(keys.map(hashedRateLimitKey));
  const now = new Date();
  const nowText = now.toISOString();
  const cutoffText = new Date(now.getTime() - loginWindowSeconds(env) * 1_000).toISOString();
  const blockedUntil = new Date(now.getTime() + loginBlockSeconds(env) * 1_000).toISOString();
  await env.DB.batch(
    hashes.map((hash, index) =>
      env.DB.prepare(
        `INSERT INTO login_attempts
          (key_type, key_hash, window_started_at, attempts, blocked_until)
         VALUES (?, ?, ?, 1, NULL)
         ON CONFLICT(key_type, key_hash) DO UPDATE SET
           attempts = CASE
             WHEN login_attempts.window_started_at <= ? THEN 1
             ELSE login_attempts.attempts + 1
           END,
           window_started_at = CASE
             WHEN login_attempts.window_started_at <= ? THEN excluded.window_started_at
             ELSE login_attempts.window_started_at
           END,
           blocked_until = CASE
             WHEN login_attempts.window_started_at <= ? THEN NULL
             WHEN login_attempts.attempts + 1 >= ? THEN ?
             ELSE login_attempts.blocked_until
           END`,
      ).bind(
        keys[index].type,
        hash,
        nowText,
        cutoffText,
        cutoffText,
        cutoffText,
        loginMaxAttempts(env),
        blockedUntil,
      ),
    ),
  );
}

async function clearLoginFailures(
  env: AppEnv,
  keys: RateLimitKey[],
): Promise<void> {
  const hashes = await Promise.all(keys.map(hashedRateLimitKey));
  await env.DB.batch(
    hashes.map((hash, index) =>
      env.DB.prepare("DELETE FROM login_attempts WHERE key_type = ? AND key_hash = ?")
        .bind(keys[index].type, hash),
    ),
  );
}

function invalidLoginError(): HttpError {
  return new HttpError(401, "Invalid email or password", "INVALID_CREDENTIALS");
}

async function createTokens(options: TokenResponseOptions): Promise<Record<string, unknown>> {
  const { env, user, device, sessionId, refreshToken, refreshExpiresAt } = options;
  const secret = requireJwtSecret(env);
  const issuedAt = Math.floor(Date.now() / 1_000);
  const accessExpiresAt = issuedAt + accessTokenTtl(env);
  const accessToken = await signAccessToken(
    {
      iss: "lingshu-cloud",
      aud: "lingshu-client",
      sub: user.id,
      sid: sessionId,
      did: device.id,
      typ: "access",
      jti: crypto.randomUUID(),
      iat: issuedAt,
      exp: accessExpiresAt,
    },
    secret,
  );
  return {
    access_token: accessToken,
    token_type: "Bearer",
    expires_in: accessExpiresAt - issuedAt,
    refresh_token: refreshToken,
    refresh_expires_in: Math.max(0, Math.floor((Date.parse(refreshExpiresAt) - Date.now()) / 1_000)),
    user: publicUser(user),
    device: {
      id: device.id,
      name: device.name,
      platform: device.platform,
    },
  };
}

async function login(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const body = await readJson(request, env);
  const email = normalizeEmail(body.email);
  const password = loginPassword(body.password);
  const deviceInput = getDeviceInput(request, body);
  const rateKeys = loginRateLimitKeys(request, email, deviceInput);
  const retryAfter = await rateLimitStatus(env, rateKeys);
  if (retryAfter > 0) {
    throw new HttpError(
      429,
      "Too many login attempts",
      "LOGIN_RATE_LIMITED",
      { "Retry-After": String(retryAfter) },
    );
  }

  const user = await env.DB.prepare(
    "SELECT * FROM users WHERE email = ? AND deleted_at IS NULL",
  )
    .bind(email)
    .first<UserRow>();
  const passwordMatches = user
    ? await verifyPassword(password, user.password_hash)
    : (await burnPasswordKdf(password), false);
  if (!user || !passwordMatches) {
    await recordLoginFailure(env, rateKeys);
    throw invalidLoginError();
  }

  await clearLoginFailures(env, rateKeys);
  return createDeviceSession(request, env, user, deviceInput);
}

async function createDeviceSession(request: Request, env: AppEnv, user: UserRow, deviceInput: LoginDeviceInput): Promise<Response> {
  const now = nowIso();
  const existingDevice = deviceInput.id
    ? await env.DB.prepare("SELECT * FROM devices WHERE id = ?")
        .bind(deviceInput.id)
        .first<DeviceRow>()
    : null;
  const useExistingDevice =
    existingDevice &&
    existingDevice.user_id === user.id &&
    existingDevice.revoked_at === null;
  const deviceId = useExistingDevice ? existingDevice.id : crypto.randomUUID();
  const device: DeviceRow = useExistingDevice
    ? {
        ...existingDevice,
        name: deviceInput.name,
        platform: deviceInput.platform,
        last_seen_at: now,
      }
    : {
        id: deviceId,
        user_id: user.id,
        name: deviceInput.name,
        platform: deviceInput.platform,
        created_at: now,
        last_seen_at: now,
        revoked_at: null,
      };
  const sessionId = crypto.randomUUID();
  const familyId = crypto.randomUUID();
  const refreshToken = randomToken();
  const refreshTokenHash = await sha256Base64Url(refreshToken);
  const refreshExpiresAt = new Date(Date.now() + refreshTokenTtl(env) * 1_000).toISOString();

  await env.DB.batch([
    useExistingDevice
      ? env.DB.prepare(
          "UPDATE devices SET name = ?, platform = ?, last_seen_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
        ).bind(device.name, device.platform, now, device.id, user.id)
      : env.DB.prepare(
          "INSERT INTO devices (id, user_id, name, platform, created_at, last_seen_at) VALUES (?, ?, ?, ?, ?, ?)",
        ).bind(device.id, user.id, device.name, device.platform, now, now),
    env.DB.prepare(
      `INSERT INTO sessions
        (id, user_id, device_id, family_id, refresh_token_hash,
         created_at, last_used_at, expires_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    ).bind(
      sessionId,
      user.id,
      device.id,
      familyId,
      refreshTokenHash,
      now,
      now,
      refreshExpiresAt,
    ),
    env.DB.prepare("UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?")
      .bind(now, now, user.id),
  ]);

  return tokenResponse(request, {
      env,
      user,
      device,
      sessionId,
      refreshToken,
      refreshExpiresAt,
    });
}

async function revokeSessionFamily(
  env: AppEnv,
  familyId: string,
  at = nowIso(),
): Promise<void> {
  await env.DB.prepare(
    "UPDATE sessions SET revoked_at = COALESCE(revoked_at, ?) WHERE family_id = ?",
  )
    .bind(at, familyId)
    .run();
}

async function refresh(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const body = await readJson(request, env);
  const refreshToken = requiredString(body, "refresh_token", 128);
  const refreshTokenHash = await sha256Base64Url(refreshToken);
  const row = await env.DB.prepare(
    `SELECT
       s.*, u.email, u.display_name, u.email_verified_at,
       u.created_at AS user_created_at, u.deleted_at AS user_deleted_at,
       d.name AS device_name, d.platform,
       d.created_at AS device_created_at,
       d.last_seen_at AS device_last_seen_at,
       d.revoked_at AS device_revoked_at
     FROM sessions s
     JOIN users u ON u.id = s.user_id
     JOIN devices d ON d.id = s.device_id
     WHERE s.refresh_token_hash = ?`,
  )
    .bind(refreshTokenHash)
    .first<SessionRow>();
  if (!row || row.user_deleted_at || row.device_revoked_at) {
    throw new HttpError(401, "Invalid refresh token", "INVALID_REFRESH_TOKEN");
  }
  if (row.revoked_at || row.replaced_by) {
    await revokeSessionFamily(env, row.family_id);
    throw new HttpError(401, "Invalid refresh token", "REFRESH_TOKEN_REUSE");
  }

  const now = nowIso();
  if (Date.parse(row.expires_at) <= Date.now()) {
    await env.DB.prepare("UPDATE sessions SET revoked_at = ? WHERE id = ?")
      .bind(now, row.id)
      .run();
    throw new HttpError(401, "Invalid refresh token", "REFRESH_TOKEN_EXPIRED");
  }

  const newSessionId = crypto.randomUUID();
  const newRefreshToken = randomToken();
  const newRefreshTokenHash = await sha256Base64Url(newRefreshToken);
  const newRefreshExpiresAt = new Date(Date.now() + refreshTokenTtl(env) * 1_000).toISOString();
  const results = await env.DB.batch([
    env.DB.prepare(
      `INSERT INTO sessions
        (id, user_id, device_id, family_id, refresh_token_hash,
         created_at, last_used_at, expires_at)
       SELECT ?, user_id, device_id, family_id, ?, ?, ?, ?
       FROM sessions
       WHERE id = ? AND refresh_token_hash = ?
         AND revoked_at IS NULL AND expires_at > ?`,
    ).bind(
      newSessionId,
      newRefreshTokenHash,
      now,
      now,
      newRefreshExpiresAt,
      row.id,
      refreshTokenHash,
      now,
    ),
    env.DB.prepare(
      `UPDATE sessions
       SET revoked_at = ?, last_used_at = ?, replaced_by = ?
       WHERE id = ? AND refresh_token_hash = ?
         AND revoked_at IS NULL AND expires_at > ?`,
    ).bind(now, now, newSessionId, row.id, refreshTokenHash, now),
    env.DB.prepare(
      "UPDATE devices SET last_seen_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
    ).bind(now, row.device_id, row.user_id),
  ]);

  if (changesOf(results[0]) !== 1 || changesOf(results[1]) !== 1) {
    await revokeSessionFamily(env, row.family_id);
    throw new HttpError(401, "Invalid refresh token", "REFRESH_TOKEN_REUSE");
  }

  const user: UserRow = {
    id: row.user_id,
    email: row.email,
    display_name: row.display_name,
    password_hash: "",
    email_verified_at: row.email_verified_at,
    last_login_at: null,
    deleted_at: row.user_deleted_at,
    created_at: row.user_created_at,
    updated_at: now,
  };
  const device: DeviceRow = {
    id: row.device_id,
    user_id: row.user_id,
    name: row.device_name,
    platform: row.platform,
    created_at: row.device_created_at,
    last_seen_at: now,
    revoked_at: null,
  };
  return tokenResponse(request, {
      env,
      user,
      device,
      sessionId: newSessionId,
      refreshToken: newRefreshToken,
      refreshExpiresAt: newRefreshExpiresAt,
    });
}

async function authenticate(request: Request, env: AppEnv): Promise<AuthContext> {
  const authorization = request.headers.get("Authorization") || "";
  if (!authorization.startsWith("Bearer ")) {
    throw new HttpError(401, "Bearer access token required", "UNAUTHORIZED");
  }
  const token = authorization.slice("Bearer ".length).trim();
  if (!token) {
    throw new HttpError(401, "Bearer access token required", "UNAUTHORIZED");
  }
  const claims = await verifyAccessToken(token, requireJwtSecret(env));
  if (!claims) {
    throw new HttpError(401, "Invalid access token", "UNAUTHORIZED");
  }
  const session = await env.DB.prepare(
    `SELECT
       s.id, s.user_id, s.device_id, s.expires_at,
       u.email, u.display_name
     FROM sessions s
     JOIN devices d ON d.id = s.device_id
     JOIN users u ON u.id = s.user_id
     WHERE s.id = ? AND s.user_id = ? AND s.device_id = ?
       AND s.revoked_at IS NULL AND s.expires_at > ?
       AND d.revoked_at IS NULL AND u.deleted_at IS NULL`,
  )
    .bind(claims.sid, claims.sub, claims.did, nowIso())
    .first<AuthSessionRow>();
  if (!session) {
    throw new HttpError(401, "Session is no longer active", "SESSION_REVOKED");
  }
  return {
    userId: session.user_id,
    sessionId: session.id,
    deviceId: session.device_id,
    email: session.email,
    displayName: session.display_name,
    sessionExpiresAt: session.expires_at,
  };
}

const BROWSER_COOKIE = "__Host-lingshu_browser";

function sameOriginBrowser(request: Request): boolean {
  return request.headers.get("Origin") === new URL(request.url).origin;
}

function requireBrowserOrigin(request: Request): void {
  if (!sameOriginBrowser(request)) throw new HttpError(403, "Same-origin browser request required", "INVALID_ORIGIN");
}

async function tokenResponse(request: Request, options: TokenResponseOptions): Promise<Response> {
  const response = jsonResponse(request, options.env, 200, await createTokens(options));
  // A first-party HttpOnly session lets a newly opened browser tab recognise
  // the signed-in user. Desktop API responses never set a browser cookie.
  if (sameOriginBrowser(request)) {
    const now = Math.floor(Date.now() / 1000);
    const expires = Math.floor(Date.parse(options.refreshExpiresAt) / 1000);
    const cookie = await signAccessToken({
      iss: "lingshu-cloud", aud: "lingshu-client", typ: "access", jti: crypto.randomUUID(),
      sub: options.user.id, sid: options.sessionId, did: options.device.id, iat: now, exp: expires,
    }, requireJwtSecret(options.env));
    response.headers.set("Set-Cookie", `${BROWSER_COOKIE}=${cookie}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=${Math.max(0, expires - now)}`);
  }
  return response;
}

async function browserIdentity(request: Request, env: AppEnv): Promise<AuthContext> {
  requireBrowserOrigin(request);
  const value = (request.headers.get("Cookie") || "").split(";").map(part => part.trim())
    .find(part => part.startsWith(`${BROWSER_COOKIE}=`))?.slice(BROWSER_COOKIE.length + 1);
  if (!value) throw new HttpError(401, "Browser session required", "UNAUTHORIZED");
  const headers = new Headers(request.headers);
  headers.set("Authorization", `Bearer ${value}`);
  return authenticate(new Request(request.url, { headers }), env);
}

// Page navigations (address bar, sidebar links) carry no Origin header, so the
// same-origin guard in browserIdentity() would reject them. Read the durable
// browser cookie only: absent or invalid simply means "signed out".
async function cookieIdentity(request: Request, env: AppEnv): Promise<AuthContext | null> {
  const value = (request.headers.get("Cookie") || "").split(";").map(part => part.trim())
    .find(part => part.startsWith(`${BROWSER_COOKIE}=`))?.slice(BROWSER_COOKIE.length + 1);
  if (!value) return null;
  const headers = new Headers(request.headers);
  headers.set("Authorization", `Bearer ${value}`);
  try {
    return await authenticate(new Request(request.url, { headers }), env);
  } catch {
    return null;
  }
}

// A team seat is paid by the workspace owner, so invited members may keep free
// accounts. Gate on entitlement OR membership; the entitlement half mirrors
// hasTeamEntitlement() in teams.ts.
async function canAccessTeam(env: AppEnv, userId: string): Promise<boolean> {
  const [subscription, member] = await Promise.all([
    env.DB.prepare("SELECT plan_code, status, current_period_end FROM subscriptions WHERE user_id = ?")
      .bind(userId)
      .first<{ plan_code: string; status: string; current_period_end: string | null }>(),
    env.DB.prepare("SELECT 1 AS ok FROM team_members WHERE cloud_user_id = ? LIMIT 1")
      .bind(userId)
      .first<{ ok: number }>(),
  ]);
  if (member) return true;
  return canCreateTeam(subscription);
}

async function browserSession(request: Request, env: AppEnv): Promise<Response> {
  const session = await browserIdentity(request, env);
  const now = Math.floor(Date.now() / 1000);
  const expires = Math.min(now + accessTokenTtl(env), Math.floor(Date.parse(session.sessionExpiresAt) / 1000));
  const token = await signAccessToken({
    iss: "lingshu-cloud", aud: "lingshu-client", typ: "access", jti: crypto.randomUUID(),
    sub: session.userId, sid: session.sessionId, did: session.deviceId, iat: now, exp: expires,
  }, requireJwtSecret(env));
  // Never expose the durable browser credential to JavaScript.
  return jsonResponse(request, env, 200, { access_token: token, expires_in: expires - now, token_type: "Bearer" });
}

function desktopRedirect(value: unknown): string {
  if (typeof value !== "string") throw new HttpError(400, "Invalid redirect", "INVALID_REDIRECT");
  let url: URL;
  try { url = new URL(value); } catch { throw new HttpError(400, "Invalid redirect", "INVALID_REDIRECT"); }
  if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || !url.port || Number(url.port) < 1024 ||
      url.pathname !== "/callback" || url.username || url.password || url.search || url.hash || url.href !== value) {
    throw new HttpError(400, "Only the desktop loopback callback is allowed", "INVALID_REDIRECT");
  }
  return value;
}

async function desktopAuthorize(request: Request, env: AppEnv): Promise<Response> {
  requireBrowserOrigin(request);
  const session = request.headers.has("Authorization") ? await authenticate(request, env) : await browserIdentity(request, env);
  const body = await readJson(request, env);
  const redirectUri = desktopRedirect(body.redirect_uri);
  const challenge = requiredString(body, "code_challenge", 43);
  const state = requiredString(body, "state", 128);
  if (!/^[A-Za-z0-9_-]{43}$/.test(challenge) || body.code_challenge_method !== "S256" || !/^[a-f0-9]{64}$/.test(state)) {
    throw new HttpError(400, "Invalid PKCE request", "INVALID_AUTHORIZATION");
  }
  const device = getDeviceInput(request, body);
  const now = nowIso();
  await env.DB.prepare("DELETE FROM desktop_authorization_codes WHERE expires_at <= ?").bind(now).run();
  const pending = await env.DB.prepare("SELECT COUNT(*) AS count FROM desktop_authorization_codes WHERE source_session_id = ? AND consumed_at IS NULL")
    .bind(session.sessionId).first<{ count: number }>();
  if ((pending?.count || 0) >= 10) throw new HttpError(429, "Too many pending authorizations", "LOGIN_RATE_LIMITED");
  const code = randomToken();
  await env.DB.prepare(`INSERT INTO desktop_authorization_codes
    (code_hash, user_id, source_session_id, code_challenge, redirect_uri, device_id, device_name, platform, expires_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`)
    .bind(await sha256Base64Url(code), session.userId, session.sessionId, challenge, redirectUri,
      device.id || null, device.name, device.platform, new Date(Date.now() + 60_000).toISOString()).run();
  const callback = new URL(redirectUri);
  callback.searchParams.set("code", code);
  callback.searchParams.set("state", state);
  return jsonResponse(request, env, 200, { redirect_uri: callback.href });
}

async function desktopExchange(request: Request, env: AppEnv): Promise<Response> {
  const body = await readJson(request, env);
  const code = requiredString(body, "code", 128);
  const verifier = requiredString(body, "code_verifier", 128);
  const redirectUri = desktopRedirect(body.redirect_uri);
  if (!/^[A-Za-z0-9._~-]{43,128}$/.test(verifier)) throw new HttpError(400, "Invalid verifier", "INVALID_AUTHORIZATION");
  const now = nowIso();
  // Consume atomically and only with the correct PKCE proof, exact redirect,
  // unexpired code, and a still-active authorising browser session.
  const grant = await env.DB.prepare(`UPDATE desktop_authorization_codes SET consumed_at = ?
    WHERE code_hash = ? AND code_challenge = ? AND redirect_uri = ? AND consumed_at IS NULL AND expires_at > ?
    AND EXISTS (SELECT 1 FROM sessions s JOIN devices d ON d.id = s.device_id JOIN users u ON u.id = s.user_id
      WHERE s.id = source_session_id AND s.revoked_at IS NULL AND s.expires_at > ? AND d.revoked_at IS NULL AND u.deleted_at IS NULL)
    RETURNING user_id, device_id, device_name, platform`)
    .bind(now, await sha256Base64Url(code), await sha256Base64Url(verifier), redirectUri, now, now)
    .first<{ user_id: string; device_id: string | null; device_name: string; platform: string }>();
  if (!grant) throw new HttpError(401, "Authorization expired or already used", "INVALID_AUTHORIZATION");
  const user = await env.DB.prepare("SELECT * FROM users WHERE id = ? AND deleted_at IS NULL").bind(grant.user_id).first<UserRow>();
  if (!user) throw new HttpError(401, "Account unavailable", "INVALID_AUTHORIZATION");
  return createDeviceSession(request, env, user, { id: grant.device_id || undefined, name: grant.device_name, platform: grant.platform, fingerprint: "desktop-browser" });
}

async function logout(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const body = await readJson(request, env);
  const refreshToken = optionalString(body, "refresh_token", 128);
  if (refreshToken) {
    const hash = await sha256Base64Url(refreshToken);
    const row = await env.DB.prepare(
      "SELECT family_id FROM sessions WHERE refresh_token_hash = ?",
    )
      .bind(hash)
      .first<{ family_id: string }>();
    if (row) await revokeSessionFamily(env, row.family_id);
    return browserLogoutResponse(request, env);
  }

  const session = await authenticate(request, env);
  await env.DB.prepare(
    "UPDATE sessions SET revoked_at = ? WHERE id = ? AND user_id = ?",
  )
    .bind(nowIso(), session.sessionId, session.userId)
    .run();
  return browserLogoutResponse(request, env);
}

function browserLogoutResponse(request: Request, env: AppEnv): Response {
  const response = emptyResponse(request, env);
  if (sameOriginBrowser(request)) response.headers.set("Set-Cookie", `${BROWSER_COOKIE}=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0`);
  return response;
}

async function account(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const session = await authenticate(request, env);
  const user = await env.DB.prepare(
    "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL",
  )
    .bind(session.userId)
    .first<UserRow>();
  const device = await env.DB.prepare(
    "SELECT * FROM devices WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
  )
    .bind(session.deviceId, session.userId)
    .first<DeviceRow>();
  if (!user || !device) {
    throw new HttpError(401, "Session is no longer active", "SESSION_REVOKED");
  }
  return jsonResponse(request, env, 200, {
    user: publicUser(user),
    device: {
      id: device.id,
      name: device.name,
      platform: device.platform,
      last_seen_at: device.last_seen_at,
    },
    session: {
      id: session.sessionId,
      expires_at: session.sessionExpiresAt,
    },
  });
}

async function devices(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const session = await authenticate(request, env);
  const rows = await env.DB.prepare(
    `SELECT
       d.id, d.name, d.platform, d.created_at, d.last_seen_at,
       COUNT(CASE WHEN s.revoked_at IS NULL AND s.expires_at > ? THEN 1 END)
         AS active_session_count
     FROM devices d
     LEFT JOIN sessions s ON s.device_id = d.id
     WHERE d.user_id = ? AND d.revoked_at IS NULL
     GROUP BY d.id, d.name, d.platform, d.created_at, d.last_seen_at
     ORDER BY d.last_seen_at DESC`,
  )
    .bind(nowIso(), session.userId)
    .all<{
      id: string;
      name: string;
      platform: string;
      created_at: string;
      last_seen_at: string;
      active_session_count: number;
    }>();
  return jsonResponse(request, env, 200, {
    devices: rows.results.map((row) => ({
      id: row.id,
      name: row.name,
      platform: row.platform,
      created_at: row.created_at,
      last_seen_at: row.last_seen_at,
      active_session_count: Number(row.active_session_count),
    })),
  });
}

async function revokeDevice(
  request: Request,
  env: AppEnv,
  deviceId: string,
): Promise<Response> {
  if (!isUuid(deviceId)) {
    throw new HttpError(400, "Invalid device id", "INVALID_DEVICE_ID");
  }
  const session = await authenticate(request, env);
  const device = await env.DB.prepare(
    "SELECT id FROM devices WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
  )
    .bind(deviceId, session.userId)
    .first<{ id: string }>();
  if (!device) {
    throw new HttpError(404, "Device not found", "DEVICE_NOT_FOUND");
  }
  const now = nowIso();
  await env.DB.batch([
    env.DB.prepare("UPDATE devices SET revoked_at = ? WHERE id = ? AND user_id = ?")
      .bind(now, deviceId, session.userId),
    env.DB.prepare("UPDATE sessions SET revoked_at = ? WHERE device_id = ? AND user_id = ?")
      .bind(now, deviceId, session.userId),
  ]);
  return emptyResponse(request, env);
}

async function billingOverview(
  request: Request,
  env: AppEnv,
): Promise<Response> {
  const session = await authenticate(request, env);
  return jsonResponse(request, env, 200, await getBillingOverview(env, session));
}

async function billingCheckout(
  request: Request,
  env: AppEnv,
  stripeFetch: StripeFetch,
): Promise<Response> {
  const session = await authenticate(request, env);
  const body = await readJson(request, env);
  const interval = requiredString(body, "interval", 16);
  if (interval !== "month" && interval !== "year") {
    throw new HttpError(400, "Invalid billing interval", "INVALID_BILLING_INTERVAL");
  }
  return jsonResponse(
    request,
    env,
    200,
    await createCheckoutSession(env, session, interval, stripeFetch),
  );
}

async function billingPortal(
  request: Request,
  env: AppEnv,
  stripeFetch: StripeFetch,
): Promise<Response> {
  const session = await authenticate(request, env);
  return jsonResponse(
    request,
    env,
    200,
    await createPortalSession(env, session, stripeFetch),
  );
}

async function route(
  request: Request,
  env: AppEnv,
  mailer: Mailer,
  stripeFetch: StripeFetch,
): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname.replace(/\/$/, "") || "/";

  if (
    (request.method === "GET" || request.method === "HEAD") &&
    !path.startsWith("/api/") &&
    path !== "/health"
  ) {
    if (path === "/team" || path === "/team/index.html") {
      return teamPageGate(request, env);
    }
    return assetResponse(request, env);
  }
  if (request.method === "GET" && path === "/health") {
    return jsonResponse(request, env, 200, {
      service: "lingshu-cloud",
      status: "ok",
    });
  }
  if (request.method === "POST" && path === "/api/v1/auth/register") {
    return register(request, env, mailer);
  }
  if (
    (request.method === "POST" || request.method === "GET") &&
    path === "/api/v1/auth/verify-email"
  ) {
    return verifyEmail(request, env);
  }
  if (request.method === "POST" && path === "/api/v1/auth/login") {
    return login(request, env);
  }
  if (request.method === "POST" && path === "/api/v1/auth/refresh") {
    return refresh(request, env);
  }
  if (request.method === "POST" && path === "/api/v1/auth/logout") {
    return logout(request, env);
  }
  if (request.method === "POST" && path === "/api/v1/auth/browser-session") return browserSession(request, env);
  if (request.method === "POST" && path === "/api/v1/auth/desktop/authorize") return desktopAuthorize(request, env);
  if (request.method === "POST" && path === "/api/v1/auth/desktop/exchange") return desktopExchange(request, env);
  if (request.method === "POST" && path === "/api/v1/billing/webhook") {
    return jsonResponse(request, env, 200, await processStripeWebhook(request, env));
  }
  if (request.method === "GET" && path === "/api/v1/account/subscription") {
    return billingOverview(request, env);
  }
  if (request.method === "POST" && path === "/api/v1/billing/checkout") {
    return billingCheckout(request, env, stripeFetch);
  }
  if (request.method === "POST" && path === "/api/v1/billing/portal") {
    return billingPortal(request, env, stripeFetch);
  }
  if (
    request.method === "GET" &&
    (path === "/api/v1/account" || path === "/api/v1/auth/me")
  ) {
    return account(request, env);
  }
  if (
    request.method === "GET" &&
    (path === "/api/v1/account/devices" || path === "/api/v1/auth/devices")
  ) {
    return devices(request, env);
  }
  const revokeMatch = path.match(/^\/api\/v1\/account\/devices\/([^/]+)$/);
  if (request.method === "DELETE" && revokeMatch) {
    return revokeDevice(request, env, decodeURIComponent(revokeMatch[1]));
  }
  if (path.startsWith("/api/v1/teams")) {
    const response = await teamRequest(request, env, path);
    if (response) return response;
  }
  if (path.startsWith('/api/v1/sync/')) {
    const session = await authenticate(request, env);
    await requireSync(env, session.userId);
    const body = request.method === 'POST' ? await readJson(request, env) : {};
    return jsonResponse(request, env, 200, await syncRequest(env, session, url, request.method, body));
  }
  throw new HttpError(404, "Not found", "NOT_FOUND");
}

export async function handleRequest(
  request: Request,
  env: AppEnv,
  ctx: ExecutionContext,
  mailer: Mailer = sendVerificationEmail,
  stripeFetch: StripeFetch = fetch,
): Promise<Response> {
  void ctx;
  if (request.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: securityHeaders(request, env) });
  }

  try {
    return await route(request, env, mailer, stripeFetch);
  } catch (error) {
    if (isHttpError(error)) return errorResponse(request, env, error);
    console.error("lingshu-cloud request failed", error);
    return jsonResponse(request, env, 500, {
      error: { code: "INTERNAL_ERROR", message: "Internal server error" },
    });
  }
}

const worker: ExportedHandler<AppEnv> = {
  fetch(request, env, ctx) {
    return handleRequest(request, env, ctx);
  },
};

export default worker;
