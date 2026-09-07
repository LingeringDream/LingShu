import { getEntitlements, type SubscriptionState } from "./entitlements";
import { HttpError } from "./errors";
import type { AppEnv, AuthContext } from "./types";

const MAX_SYNC_BYTES = 10 * 1024 * 1024;
const BASE64_PATTERN = /^[A-Za-z0-9_-]+={0,2}$/;

interface SyncRow {
  revision: number;
  object_key: string;
  checksum: string;
  byte_size: number;
  item_count: number;
  key_version: number;
  algorithm: string;
  updated_by_device_id: string;
  updated_at: string;
}

interface EncryptedSnapshot {
  algorithm: "AES-256-GCM";
  key_version: number;
  iv: string;
  ciphertext: string;
  checksum: string;
  item_count: number;
}

function nowIso(): string { return new Date().toISOString(); }

function response(request: Request, env: AppEnv, status: number, body?: unknown): Response {
  const headers = new Headers(body === undefined ? undefined : { "Content-Type": "application/json; charset=utf-8" });
  headers.set("Cache-Control", "no-store");
  headers.set("X-Content-Type-Options", "nosniff");
  const origin = request.headers.get("Origin");
  const allowed = new Set((env.CORS_ORIGINS || "").split(",").map(value => value.trim()).filter(Boolean));
  if (origin && (allowed.has(origin) || allowed.has("*"))) {
    headers.set("Access-Control-Allow-Origin", origin);
    headers.set("Vary", "Origin");
  }
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers,
  });
}

function metadata(row: SyncRow) {
  return {
    revision: Number(row.revision), checksum: row.checksum, byte_size: Number(row.byte_size),
    item_count: Number(row.item_count), key_version: Number(row.key_version), algorithm: row.algorithm,
    device_id: row.updated_by_device_id, updated_at: row.updated_at,
  };
}

async function requireSyncEntitlement(env: AppEnv, auth: AuthContext): Promise<void> {
  const subscription = await env.DB.prepare(
    "SELECT plan_code, status, current_period_end FROM subscriptions WHERE user_id = ?",
  ).bind(auth.userId).first<SubscriptionState>();
  const membership = await env.DB.prepare(
    "SELECT 1 AS present FROM team_members WHERE cloud_user_id = ? LIMIT 1",
  ).bind(auth.userId).first<{ present: number }>();
  const enabled = getEntitlements(subscription || null, Date.now(), Boolean(membership))
    .features.some(feature => feature.code === "cloud_sync" && feature.enabled);
  if (!enabled) throw new HttpError(403, "Pro cloud sync entitlement required", "PRO_REQUIRED");
}

async function current(env: AppEnv, userId: string): Promise<SyncRow | null> {
  return env.DB.prepare("SELECT * FROM user_sync_state WHERE user_id = ?")
    .bind(userId).first<SyncRow>();
}

async function readPayload(request: Request): Promise<{ base_revision: number; snapshot: EncryptedSnapshot }> {
  const length = Number(request.headers.get("Content-Length"));
  if (Number.isFinite(length) && length > MAX_SYNC_BYTES) throw new HttpError(413, "Sync payload is too large", "SYNC_TOO_LARGE");
  const text = await request.text();
  if (new TextEncoder().encode(text).byteLength > MAX_SYNC_BYTES) throw new HttpError(413, "Sync payload is too large", "SYNC_TOO_LARGE");
  let body: Record<string, unknown>;
  try { body = JSON.parse(text) as Record<string, unknown>; } catch { throw new HttpError(400, "Invalid sync payload", "INVALID_SYNC_PAYLOAD"); }
  const base = body.base_revision;
  const raw = body.snapshot as Partial<EncryptedSnapshot> | undefined;
  if (!Number.isSafeInteger(base) || Number(base) < 0 || !raw || raw.algorithm !== "AES-256-GCM" ||
      !Number.isSafeInteger(raw.key_version) || Number(raw.key_version) < 1 ||
      !Number.isSafeInteger(raw.item_count) || Number(raw.item_count) < 0 ||
      typeof raw.iv !== "string" || raw.iv.length < 12 || raw.iv.length > 64 || !BASE64_PATTERN.test(raw.iv) ||
      typeof raw.ciphertext !== "string" || raw.ciphertext.length < 1 || !BASE64_PATTERN.test(raw.ciphertext) ||
      typeof raw.checksum !== "string" || !/^[a-f0-9]{64}$/i.test(raw.checksum)) {
    throw new HttpError(400, "Invalid encrypted sync payload", "INVALID_SYNC_PAYLOAD");
  }
  return { base_revision: Number(base), snapshot: raw as EncryptedSnapshot };
}

export async function syncRequest(request: Request, env: AppEnv, auth: AuthContext, path: string): Promise<Response | null> {
  if (!path.startsWith("/api/v1/sync")) return null;
  await requireSyncEntitlement(env, auth);

  if (request.method === "GET" && path === "/api/v1/sync") {
    const row = await current(env, auth.userId);
    if (!row) return response(request, env, 200, { revision: 0, snapshot: null });
    const object = await env.MEMORY_BUCKET.get(row.object_key);
    if (!object) throw new HttpError(503, "Sync object is temporarily unavailable", "SYNC_OBJECT_MISSING");
    return response(request, env, 200, { ...metadata(row), snapshot: await object.json<EncryptedSnapshot>() });
  }

  if (request.method === "GET" && path === "/api/v1/sync/history") {
    const rows = await env.DB.prepare(`SELECT revision, object_key, checksum, byte_size, item_count,
      key_version, algorithm, device_id AS updated_by_device_id, created_at AS updated_at
      FROM user_sync_history WHERE user_id = ? ORDER BY revision DESC LIMIT 50`)
      .bind(auth.userId).all<SyncRow>();
    return response(request, env, 200, { history: rows.results.map(metadata) });
  }

  if (request.method === "PUT" && path === "/api/v1/sync") {
    const { base_revision, snapshot } = await readPayload(request);
    const prior = await current(env, auth.userId);
    const revision = prior ? Number(prior.revision) : 0;
    if (revision !== base_revision) return response(request, env, 409, { error: { code: "SYNC_CONFLICT", message: "Remote data changed" }, current: prior ? metadata(prior) : { revision: 0 } });

    const next = revision + 1;
    // Each contender owns its R2 object. A losing CAS must never overwrite or
    // delete the winning writer's ciphertext at the same revision.
    const objectKey = `users/${auth.userId}/sync/${next}-${crypto.randomUUID()}.json`;
    const encoded = JSON.stringify(snapshot);
    await env.MEMORY_BUCKET.put(objectKey, encoded, { httpMetadata: { contentType: "application/json" } });
    const updatedAt = nowIso();
    const write = env.DB.prepare(`INSERT INTO user_sync_state
      (user_id, revision, object_key, checksum, byte_size, item_count, key_version, algorithm, updated_by_device_id, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision, object_key=excluded.object_key,
      checksum=excluded.checksum, byte_size=excluded.byte_size, item_count=excluded.item_count,
      key_version=excluded.key_version, algorithm=excluded.algorithm,
      updated_by_device_id=excluded.updated_by_device_id, updated_at=excluded.updated_at
      WHERE user_sync_state.revision = ?`)
      .bind(auth.userId, next, objectKey, snapshot.checksum, encoded.length, snapshot.item_count,
        snapshot.key_version, snapshot.algorithm, auth.deviceId, updatedAt, base_revision);
    const result = await write.run();
    if ((result.meta.changes || 0) !== 1) {
      await env.MEMORY_BUCKET.delete(objectKey);
      const latest = await current(env, auth.userId);
      return response(request, env, 409, { error: { code: "SYNC_CONFLICT", message: "Remote data changed" }, current: latest ? metadata(latest) : { revision: 0 } });
    }
    await env.DB.prepare(`INSERT INTO user_sync_history
      (user_id, revision, object_key, checksum, byte_size, item_count, key_version, algorithm, device_id, created_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`)
      .bind(auth.userId, next, objectKey, snapshot.checksum, encoded.length, snapshot.item_count,
        snapshot.key_version, snapshot.algorithm, auth.deviceId, updatedAt).run();
    const expired = await env.DB.prepare(
      "SELECT revision, object_key FROM user_sync_history WHERE user_id = ? AND revision <= ?",
    ).bind(auth.userId, next - 50).all<{ revision: number; object_key: string }>();
    if (expired.results.length) {
      await Promise.allSettled(expired.results.map(item => env.MEMORY_BUCKET.delete(item.object_key)));
      await env.DB.prepare("DELETE FROM user_sync_history WHERE user_id = ? AND revision <= ?")
        .bind(auth.userId, next - 50).run();
    }
    return response(request, env, 200, { revision: next, checksum: snapshot.checksum, updated_at: updatedAt });
  }

  throw new HttpError(405, "Method not allowed", "METHOD_NOT_ALLOWED");
}
