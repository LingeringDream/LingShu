import { canCreateTeam } from "./entitlements";
import { randomToken, sha256Base64Url, verifyAccessToken } from "./crypto";
import { sendTeamInviteEmail } from "./email";
import { HttpError } from "./errors";
import type { AppEnv, AuthContext } from "./types";

type Role = "owner" | "admin" | "editor" | "viewer";
type MemoryScope = "private" | "shared" | "global";

interface AuthSessionRow {
  id: string;
  user_id: string;
  device_id: string;
  expires_at: string;
  email: string;
  display_name: string;
}

interface TeamRow {
  id: string;
  name: string;
  role: Role;
}

interface MembershipRow {
  name: string;
  role: Role;
}

interface InviteRow {
  id: string;
  email: string;
  role: Role;
  expires_at: string;
  used_at: string | null;
  revoked_at: string | null;
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function nowIso(): string {
  return new Date().toISOString();
}

function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value);
}

function changesOf(result: { meta?: { changes?: number } } | undefined): number {
  return result?.meta?.changes ?? 0;
}

function maxBodyBytes(env: AppEnv): number {
  const value = Number(env.MAX_BODY_BYTES);
  return Number.isSafeInteger(value) && value >= 1_024 && value <= 1_048_576
    ? value
    : 65_536;
}

async function readJson(request: Request, env: AppEnv): Promise<Record<string, unknown>> {
  const contentLength = Number(request.headers.get("Content-Length"));
  const maximum = maxBodyBytes(env);
  if (Number.isFinite(contentLength) && contentLength > maximum) {
    throw new HttpError(413, "Request body is too large", "BODY_TOO_LARGE");
  }
  const text = await request.text();
  if (new TextEncoder().encode(text).byteLength > maximum) {
    throw new HttpError(413, "Request body is too large", "BODY_TOO_LARGE");
  }
  if (!text.trim()) return {};
  try {
    const value: unknown = JSON.parse(text);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("object required");
    return value as Record<string, unknown>;
  } catch {
    throw new HttpError(400, "Invalid JSON body", "INVALID_JSON");
  }
}

function requiredString(body: Record<string, unknown>, field: string, maximum: number): string {
  const value = body[field];
  if (typeof value !== "string" || value.trim().length === 0 || value.length > maximum) {
    throw new HttpError(400, `Invalid ${field}`, "INVALID_INPUT");
  }
  return value.trim();
}

function normalizeEmail(value: unknown): string {
  if (typeof value !== "string") throw new HttpError(400, "A valid email is required", "INVALID_EMAIL");
  const email = value.trim().toLowerCase();
  if (email.length > 254 || !EMAIL_PATTERN.test(email)) {
    throw new HttpError(400, "A valid email is required", "INVALID_EMAIL");
  }
  return email;
}

function jsonResponse(request: Request, env: AppEnv, status: number, value: unknown): Response {
  const headers = new Headers({
    "Cache-Control": "no-store",
    "Content-Type": "application/json; charset=utf-8",
    "X-Content-Type-Options": "nosniff",
  });
  const origin = request.headers.get("Origin");
  const origins = new Set((env.CORS_ORIGINS || "").split(",").map((item) => item.trim()).filter(Boolean));
  if (origin && (origins.has(origin) || origins.has("*"))) {
    headers.set("Access-Control-Allow-Origin", origin);
    headers.set("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS");
    headers.set("Access-Control-Allow-Headers", "Authorization, Content-Type, X-LingShu-Device-ID");
    headers.set("Vary", "Origin");
  }
  return new Response(JSON.stringify(value), { status, headers });
}

function emptyResponse(request: Request, env: AppEnv, status = 204): Response {
  const headers = new Headers({
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  const origin = request.headers.get("Origin");
  const origins = new Set((env.CORS_ORIGINS || "").split(",").map((item) => item.trim()).filter(Boolean));
  if (origin && (origins.has(origin) || origins.has("*"))) {
    headers.set("Access-Control-Allow-Origin", origin);
    headers.set("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS");
    headers.set("Access-Control-Allow-Headers", "Authorization, Content-Type, X-LingShu-Device-ID");
    headers.set("Vary", "Origin");
  }
  return new Response(null, { status, headers });
}

async function authenticate(request: Request, env: AppEnv): Promise<AuthContext> {
  const authorization = request.headers.get("Authorization") || "";
  if (!authorization.startsWith("Bearer ")) {
    throw new HttpError(401, "Bearer access token required", "UNAUTHORIZED");
  }
  const token = authorization.slice("Bearer ".length).trim();
  const secret = env.JWT_SECRET?.trim();
  if (!token || !secret || secret.length < 32) {
    throw new HttpError(401, "Invalid access token", "UNAUTHORIZED");
  }
  const claims = await verifyAccessToken(token, secret);
  if (!claims) throw new HttpError(401, "Invalid access token", "UNAUTHORIZED");
  const session = await env.DB.prepare(
    `SELECT s.id, s.user_id, s.device_id, s.expires_at, u.email, u.display_name
     FROM sessions s JOIN devices d ON d.id = s.device_id JOIN users u ON u.id = s.user_id
     WHERE s.id = ? AND s.user_id = ? AND s.device_id = ? AND s.revoked_at IS NULL
       AND s.expires_at > ? AND d.revoked_at IS NULL AND u.deleted_at IS NULL`,
  ).bind(claims.sid, claims.sub, claims.did, nowIso()).first<AuthSessionRow>();
  if (!session) throw new HttpError(401, "Session is no longer active", "SESSION_REVOKED");
  return {
    userId: session.user_id,
    sessionId: session.id,
    deviceId: session.device_id,
    email: session.email,
    displayName: session.display_name,
    sessionExpiresAt: session.expires_at,
  };
}

async function membership(env: AppEnv, teamId: string, auth: AuthContext): Promise<MembershipRow> {
  const row = await env.DB.prepare(
    `SELECT t.name, m.role FROM team_workspaces t JOIN team_members m ON m.team_id = t.id
     WHERE t.id = ? AND m.cloud_user_id = ?`,
  ).bind(teamId, auth.userId).first<MembershipRow>();
  if (!row) throw new HttpError(403, "You do not have access to this team", "TEAM_FORBIDDEN");
  return row;
}

function requireAdmin(role: Role): void {
  if (role !== "owner" && role !== "admin") {
    throw new HttpError(403, "Only team administrators can perform this action", "TEAM_ADMIN_REQUIRED");
  }
}

function requireRole(value: unknown): "admin" | "editor" | "viewer" {
  if (value !== "admin" && value !== "editor" && value !== "viewer") {
    throw new HttpError(400, "Invalid role", "INVALID_ROLE");
  }
  return value;
}

async function hasTeamEntitlement(env: AppEnv, userId: string): Promise<boolean> {
  const row = await env.DB.prepare(
    `SELECT plan_code, status, current_period_end FROM subscriptions WHERE user_id = ?`,
  ).bind(userId).first<{ plan_code: string; status: string; current_period_end: string | null }>();
  return canCreateTeam(row);
}

async function audit(env: AppEnv, teamId: string, actor: string, action: string, target: string): Promise<void> {
  await env.DB.prepare(
    `INSERT INTO team_audit_events (id, team_id, actor, action, target, created_at) VALUES (?, ?, ?, ?, ?, ?)`,
  ).bind(crypto.randomUUID(), teamId, actor, action, target, nowIso()).run();
}

function teamPath(path: string): string[] | null {
  const prefix = "/api/v1/teams";
  if (path !== prefix && !path.startsWith(`${prefix}/`)) return null;
  return path.slice(prefix.length).split("/").filter(Boolean).map((part) => decodeURIComponent(part));
}

async function listTeams(request: Request, env: AppEnv, auth: AuthContext): Promise<Response> {
  const teamsResult = await env.DB.prepare(
    `SELECT t.id, t.name, m.role FROM team_workspaces t JOIN team_members m ON m.team_id = t.id
     WHERE m.cloud_user_id = ? ORDER BY t.created_at`,
  ).bind(auth.userId).all<TeamRow>();
  const invitesResult = await env.DB.prepare(
    `SELECT i.id AS invite_id, i.team_id, t.name AS team_name, i.role, i.expires_at, i.created_at
       FROM team_invites i JOIN team_workspaces t ON t.id = i.team_id
      WHERE i.email = ? AND i.used_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > ?
      ORDER BY i.created_at DESC`,
  ).bind(auth.email.toLowerCase(), nowIso()).all<PendingInviteRow>();
  return jsonResponse(request, env, 200, {
    teams: teamsResult.results,
    pending_invites_for_me: invitesResult.results,
  });
}

async function createTeam(request: Request, env: AppEnv, auth: AuthContext): Promise<Response> {
  if (!await hasTeamEntitlement(env, auth.userId)) {
    throw new HttpError(403, "An active Team plan is required to create a team", "TEAM_PLAN_REQUIRED");
  }
  const body = await readJson(request, env);
  const name = requiredString(body, "name", 100);
  const teamId = crypto.randomUUID();
  const now = nowIso();
  await env.DB.batch([
    env.DB.prepare("INSERT INTO team_workspaces (id, name, created_at) VALUES (?, ?, ?)").bind(teamId, name, now),
    env.DB.prepare("INSERT INTO team_members (team_id, cloud_user_id, email, role, created_at) VALUES (?, ?, ?, 'owner', ?)").bind(teamId, auth.userId, auth.email.toLowerCase(), now),
    env.DB.prepare("INSERT INTO team_audit_events (id, team_id, actor, action, target, created_at) VALUES (?, ?, ?, 'team.created', ?, ?)").bind(crypto.randomUUID(), teamId, auth.userId, name, now),
  ]);
  return jsonResponse(request, env, 201, { id: teamId, name, role: "owner" });
}

interface PendingInviteRow {
  invite_id: string;
  team_id: string;
  team_name: string;
  role: Role;
  expires_at: string;
  created_at: string;
}

async function joinTeam(request: Request, env: AppEnv, auth: AuthContext): Promise<Response> {
  const body = await readJson(request, env);
  const token = requiredString(body, "token", 128);
  const tokenHash = await sha256Base64Url(token);
  const now = nowIso();
  const invite = await env.DB.prepare(
    `SELECT id, team_id, email, role FROM team_invites
     WHERE token_hash = ? AND email = ? AND expires_at > ? AND used_at IS NULL AND revoked_at IS NULL`,
  ).bind(tokenHash, auth.email.toLowerCase(), now).first<{ id: string; team_id: string; email: string; role: Role }>();
  if (!invite) throw new HttpError(403, "Invite is invalid, expired, revoked, or for another email", "INVITE_INVALID");
  return consumeInvite(env, invite, auth, request);
}

async function acceptInviteById(request: Request, env: AppEnv, auth: AuthContext, inviteId: string): Promise<Response> {
  const now = nowIso();
  const invite = await env.DB.prepare(
    `SELECT id, team_id, email, role FROM team_invites
     WHERE id = ? AND email = ? AND expires_at > ? AND used_at IS NULL AND revoked_at IS NULL`,
  ).bind(inviteId, auth.email.toLowerCase(), now).first<{ id: string; team_id: string; email: string; role: Role }>();
  if (!invite) throw new HttpError(403, "Invite is invalid, expired, revoked, or for another email", "INVITE_INVALID");
  return consumeInvite(env, invite, auth, request);
}

async function consumeInvite(env: AppEnv, invite: { id: string; team_id: string; email: string; role: Role }, auth: AuthContext, request: Request): Promise<Response> {
  const now = nowIso();
  const used = await env.DB.prepare(
    `UPDATE team_invites SET used_at = ? WHERE id = ? AND used_at IS NULL AND revoked_at IS NULL AND expires_at > ?`,
  ).bind(now, invite.id, now).run();
  if (changesOf(used) !== 1) throw new HttpError(403, "Invite is no longer available", "INVITE_INVALID");
  await env.DB.batch([
    env.DB.prepare(`INSERT INTO team_members (team_id, cloud_user_id, email, role, created_at) VALUES (?, ?, ?, ?, ?)
      ON CONFLICT(team_id, cloud_user_id) DO NOTHING`).bind(invite.team_id, auth.userId, auth.email.toLowerCase(), invite.role, now),
    env.DB.prepare("INSERT INTO team_audit_events (id, team_id, actor, action, target, created_at) VALUES (?, ?, ?, 'member.joined', ?, ?)").bind(crypto.randomUUID(), invite.team_id, auth.userId, auth.email.toLowerCase(), now),
  ]);
  const team = await env.DB.prepare(
    `SELECT t.id, t.name, m.role FROM team_workspaces t JOIN team_members m ON m.team_id = t.id
     WHERE t.id = ? AND m.cloud_user_id = ?`,
  ).bind(invite.team_id, auth.userId).first<TeamRow>();
  if (!team) throw new HttpError(500, "Team membership could not be created", "TEAM_JOIN_FAILED");
  return jsonResponse(request, env, 200, team);
}

async function listMembers(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  await membership(env, teamId, auth);
  const rows = await env.DB.prepare(
    `SELECT cloud_user_id, email, role FROM team_members WHERE team_id = ? ORDER BY created_at`,
  ).bind(teamId).all();
  return jsonResponse(request, env, 200, rows.results);
}

async function changeMember(request: Request, env: AppEnv, auth: AuthContext, teamId: string, memberId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  if (current.role !== "owner") throw new HttpError(403, "Only the owner can change roles", "TEAM_OWNER_REQUIRED");
  const body = await readJson(request, env);
  const role = requireRole(body.role);
  const result = await env.DB.prepare(
    `UPDATE team_members SET role = ? WHERE team_id = ? AND cloud_user_id = ? AND role <> 'owner'`,
  ).bind(role, teamId, memberId).run();
  if (changesOf(result) !== 1) throw new HttpError(403, "Owner or member not found", "MEMBER_CHANGE_FORBIDDEN");
  await audit(env, teamId, auth.userId, "member.role_changed", `${memberId}:${role}`);
  return emptyResponse(request, env);
}

async function removeMember(request: Request, env: AppEnv, auth: AuthContext, teamId: string, memberId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  const target = await env.DB.prepare("SELECT role FROM team_members WHERE team_id = ? AND cloud_user_id = ?")
    .bind(teamId, memberId).first<{ role: Role }>();
  if (!target || target.role === "owner" || (current.role !== "owner" && target.role === "admin")) {
    throw new HttpError(403, "This member cannot be removed", "MEMBER_REMOVE_FORBIDDEN");
  }
  await env.DB.prepare("DELETE FROM team_members WHERE team_id = ? AND cloud_user_id = ?")
    .bind(teamId, memberId).run();
  await audit(env, teamId, auth.userId, "member.removed", memberId);
  return emptyResponse(request, env);
}

async function listInvites(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  const rows = await env.DB.prepare(
    `SELECT id, email, role, expires_at, used_at, revoked_at FROM team_invites WHERE team_id = ? ORDER BY created_at DESC LIMIT 100`,
  ).bind(teamId).all<InviteRow>();
  return jsonResponse(request, env, 200, rows.results);
}

async function inviteMember(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  const body = await readJson(request, env);
  const email = normalizeEmail(body.email);
  const role = requireRole(body.role);
  if (role === "admin" && current.role !== "owner") throw new HttpError(403, "Only the owner can invite administrators", "TEAM_OWNER_REQUIRED");
  const existing = await env.DB.prepare("SELECT 1 FROM team_members WHERE team_id = ? AND email = ?")
    .bind(teamId, email).first();
  if (existing) throw new HttpError(409, "This email is already a team member", "MEMBER_EXISTS");
  const team = await env.DB.prepare("SELECT name FROM team_workspaces WHERE id = ?").bind(teamId).first<{ name: string }>();
  if (!team) throw new HttpError(404, "Team not found", "NOT_FOUND");
  const inviteId = crypto.randomUUID();
  const token = randomToken();
  const now = nowIso();
  const acceptUrl = `${env.APP_BASE_URL?.trim() || "https://account.lingeringdream.com"}/team/invite/${inviteId}`;
  try {
    await sendTeamInviteEmail(
      { to: email, inviterName: auth.displayName || auth.email, teamName: team.name, role, inviteId, acceptUrl },
      env,
    );
  } catch (error) {
    // Email is mandatory: never silently leave the invitee without a heads-up.
    throw error instanceof HttpError
        ? error
        : new HttpError(502, "Email delivery failed", "EMAIL_DELIVERY_FAILED");
  }
  await env.DB.prepare(
    `INSERT INTO team_invites (id, team_id, email, role, token_hash, expires_at, notified_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
  ).bind(inviteId, teamId, email, role, await sha256Base64Url(token), new Date(Date.now() + 7 * 86_400_000).toISOString(), now, now).run();
  await audit(env, teamId, auth.userId, "invite.created", email);
  return jsonResponse(request, env, 201, { token, invite_id: inviteId, email_sent: true });
}

async function revokeInvite(request: Request, env: AppEnv, auth: AuthContext, teamId: string, inviteId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  await env.DB.prepare("UPDATE team_invites SET revoked_at = ? WHERE team_id = ? AND id = ? AND used_at IS NULL")
    .bind(nowIso(), teamId, inviteId).run();
  await audit(env, teamId, auth.userId, "invite.revoked", inviteId);
  return emptyResponse(request, env);
}

async function listKeys(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  const rows = await env.DB.prepare(
    `SELECT id, agent_key, name, permission, expires_at, revoked_at FROM team_agent_keys WHERE team_id = ? ORDER BY created_at DESC LIMIT 100`,
  ).bind(teamId).all();
  return jsonResponse(request, env, 200, rows.results);
}

async function createKey(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  const body = await readJson(request, env);
  const agentKey = requiredString(body, "agent_key", 64);
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(agentKey)) throw new HttpError(400, "Invalid agent_key", "INVALID_INPUT");
  const name = typeof body.name === "string" && body.name.trim() ? body.name.trim().slice(0, 100) : agentKey;
  const permission = body.permission === "write" ? "write" : body.permission === "read" ? "read" : null;
  if (!permission) throw new HttpError(400, "Invalid permission", "INVALID_INPUT");
  const days = Number(body.expires_in_days);
  if (!Number.isSafeInteger(days) || days < 1 || days > 365) throw new HttpError(400, "Invalid expires_in_days", "INVALID_INPUT");
  const token = randomToken();
  await env.DB.prepare(
    `INSERT INTO team_agent_keys (id, team_id, agent_key, name, token_hash, permission, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
  ).bind(crypto.randomUUID(), teamId, agentKey, name, await sha256Base64Url(token), permission, new Date(Date.now() + days * 86_400_000).toISOString(), nowIso()).run();
  await audit(env, teamId, auth.userId, "agent_key.created", agentKey);
  return jsonResponse(request, env, 201, { token });
}

async function revokeKey(request: Request, env: AppEnv, auth: AuthContext, teamId: string, keyId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  await env.DB.prepare("UPDATE team_agent_keys SET revoked_at = ? WHERE team_id = ? AND id = ? AND revoked_at IS NULL")
    .bind(nowIso(), teamId, keyId).run();
  await audit(env, teamId, auth.userId, "agent_key.revoked", keyId);
  return emptyResponse(request, env);
}

async function listAudit(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  requireAdmin(current.role);
  const rows = await env.DB.prepare(
    `SELECT id, actor, action, target, created_at FROM team_audit_events WHERE team_id = ? ORDER BY created_at DESC LIMIT 100`,
  ).bind(teamId).all();
  return jsonResponse(request, env, 200, rows.results);
}

const visibleMemorySql = `
  SELECT m.id, m.content, m.memory_layer, m.memory_scope, m.owner_agent_id
  FROM team_memories m
  WHERE m.team_id = ? AND (
    m.memory_scope = 'global' OR
    (m.memory_scope = 'private' AND m.owner_agent_id = ?) OR
    (m.memory_scope = 'shared' AND EXISTS (
      SELECT 1 FROM team_memory_grants g WHERE g.memory_id = m.id AND g.grantee_agent_id = ? AND g.can_read = 1
    ))
  )`;

async function listMemories(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  await membership(env, teamId, auth);
  const owner = `member-${auth.userId}`;
  const rows = await env.DB.prepare(`${visibleMemorySql} ORDER BY m.created_at DESC LIMIT 100`).bind(teamId, owner, owner).all();
  return jsonResponse(request, env, 200, rows.results);
}

async function writeMemory(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  if (current.role === "viewer") throw new HttpError(403, "Read-only members cannot write memories", "MEMORY_WRITE_FORBIDDEN");
  const body = await readJson(request, env);
  const content = requiredString(body, "content", 10_000);
  const scope = body.memory_scope;
  if (scope !== "private" && scope !== "shared" && scope !== "global") throw new HttpError(400, "Invalid memory_scope", "INVALID_INPUT");
  const id = crypto.randomUUID();
  await env.DB.prepare(
    `INSERT INTO team_memories (id, team_id, content, memory_layer, memory_scope, owner_agent_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)`,
  ).bind(id, teamId, content, typeof body.memory_layer === "string" ? body.memory_layer.slice(0, 50) : "episode", scope, `member-${auth.userId}`, nowIso()).run();
  await audit(env, teamId, auth.userId, "memory.created", id);
  const rows = await env.DB.prepare(`${visibleMemorySql} AND m.id = ?`).bind(teamId, `member-${auth.userId}`, `member-${auth.userId}`, id).all();
  return jsonResponse(request, env, 201, rows.results[0]);
}

async function searchMemory(request: Request, env: AppEnv, auth: AuthContext, teamId: string): Promise<Response> {
  await membership(env, teamId, auth);
  const body = await readJson(request, env);
  const query = requiredString(body, "query", 500);
  const limit = Number(body.top_k);
  const topK = Number.isSafeInteger(limit) ? Math.max(1, Math.min(limit, 100)) : 20;
  const owner = `member-${auth.userId}`;
  const rows = await env.DB.prepare(`${visibleMemorySql} AND m.content LIKE ? ORDER BY m.created_at DESC LIMIT ?`)
    .bind(teamId, owner, owner, `%${query}%`, topK).all();
  return jsonResponse(request, env, 200, { results: rows.results });
}

async function shareMemory(request: Request, env: AppEnv, auth: AuthContext, teamId: string, memoryId: string): Promise<Response> {
  const current = await membership(env, teamId, auth);
  if (current.role === "viewer") throw new HttpError(403, "Read-only members cannot share memories", "MEMORY_SHARE_FORBIDDEN");
  const body = await readJson(request, env);
  const grantee = requiredString(body, "grantee_agent_id", 128);
  const owner = `member-${auth.userId}`;
  const memory = await env.DB.prepare("SELECT id FROM team_memories WHERE id = ? AND team_id = ? AND owner_agent_id = ?")
    .bind(memoryId, teamId, owner).first();
  if (!memory) throw new HttpError(403, "Only the memory owner can change this grant", "MEMORY_SHARE_FORBIDDEN");
  const canRead = body.can_read === true;
  if (canRead) {
    await env.DB.prepare(`INSERT INTO team_memory_grants (memory_id, grantee_agent_id, can_read) VALUES (?, ?, 1)
      ON CONFLICT(memory_id, grantee_agent_id) DO UPDATE SET can_read = 1`).bind(memoryId, grantee).run();
    await env.DB.prepare("UPDATE team_memories SET memory_scope = 'shared' WHERE id = ?").bind(memoryId).run();
  } else {
    await env.DB.prepare("DELETE FROM team_memory_grants WHERE memory_id = ? AND grantee_agent_id = ?").bind(memoryId, grantee).run();
  }
  await audit(env, teamId, auth.userId, "memory.grant_changed", `${memoryId}:${grantee}:${canRead}`);
  return jsonResponse(request, env, 200, { memory_id: memoryId, grantee_agent_id: grantee, can_read: canRead });
}

export async function teamRequest(request: Request, env: AppEnv, path: string): Promise<Response | null> {
  const parts = teamPath(path);
  if (!parts) return null;
  const auth = await authenticate(request, env);
  if (parts.length === 0 && request.method === "GET") return listTeams(request, env, auth);
  if (parts.length === 0 && request.method === "POST") return createTeam(request, env, auth);
  if (parts.length === 1 && parts[0] === "join" && request.method === "POST") return joinTeam(request, env, auth);
  if (parts.length === 3 && parts[0] === "invites" && parts[2] === "accept" && request.method === "POST" && isUuid(parts[1])) return acceptInviteById(request, env, auth, parts[1]);
  if (parts.length === 1 || !isUuid(parts[0])) throw new HttpError(404, "Not found", "NOT_FOUND");
  const teamId = parts[0];
  if (parts.length === 2 && parts[1] === "members" && request.method === "GET") return listMembers(request, env, auth, teamId);
  if (parts.length === 3 && parts[1] === "members" && request.method === "PATCH" && isUuid(parts[2])) return changeMember(request, env, auth, teamId, parts[2]);
  if (parts.length === 3 && parts[1] === "members" && request.method === "DELETE" && isUuid(parts[2])) return removeMember(request, env, auth, teamId, parts[2]);
  if (parts.length === 2 && parts[1] === "invites" && request.method === "GET") return listInvites(request, env, auth, teamId);
  if (parts.length === 2 && parts[1] === "invites" && request.method === "POST") return inviteMember(request, env, auth, teamId);
  if (parts.length === 3 && parts[1] === "invites" && request.method === "DELETE" && isUuid(parts[2])) return revokeInvite(request, env, auth, teamId, parts[2]);
  if (parts.length === 2 && parts[1] === "keys" && request.method === "GET") return listKeys(request, env, auth, teamId);
  if (parts.length === 2 && parts[1] === "keys" && request.method === "POST") return createKey(request, env, auth, teamId);
  if (parts.length === 3 && parts[1] === "keys" && request.method === "DELETE" && isUuid(parts[2])) return revokeKey(request, env, auth, teamId, parts[2]);
  if (parts.length === 2 && parts[1] === "audit" && request.method === "GET") return listAudit(request, env, auth, teamId);
  if (parts.length === 2 && parts[1] === "memories" && request.method === "GET") return listMemories(request, env, auth, teamId);
  if (parts.length === 2 && parts[1] === "memories" && request.method === "POST") return writeMemory(request, env, auth, teamId);
  if (parts.length === 2 && parts[1] === "search" && request.method === "POST") return searchMemory(request, env, auth, teamId);
  if (parts.length === 4 && parts[1] === "memories" && parts[3] === "grants" && request.method === "POST" && isUuid(parts[2])) return shareMemory(request, env, auth, teamId, parts[2]);
  throw new HttpError(404, "Not found", "NOT_FOUND");
}
