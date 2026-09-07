import { getEntitlements, type SubscriptionState } from './entitlements';
import { HttpError } from './errors';
import type { AppEnv, AuthContext } from './types';

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const keyIdPattern = /^[0-9a-f]{64}$/;
function invalid(): never { throw new HttpError(400, 'Invalid encrypted sync payload', 'INVALID_SYNC'); }

export async function requireSync(env: AppEnv, userId: string) {
  const [subscription, member] = await Promise.all([
    env.DB.prepare('SELECT plan_code, status, current_period_end FROM subscriptions WHERE user_id = ?')
      .bind(userId).first<SubscriptionState>(),
    env.DB.prepare('SELECT 1 AS ok FROM team_members WHERE cloud_user_id = ? LIMIT 1').bind(userId).first(),
  ]);
  if (!getEntitlements(subscription, Date.now(), !!member).features.some(f => f.code === 'cloud_sync' && f.enabled)) {
    throw new HttpError(403, 'Pro or Team is required', 'SYNC_PLAN_REQUIRED');
  }
}

export async function syncRequest(env: AppEnv, auth: AuthContext, url: URL, method: string, body: Record<string, unknown>) {
  const path = url.pathname.replace(/\/$/, '');
  const user = auth.userId;
  // Called after authentication and entitlement checks, including reads/history.
  if (path === '/api/v1/sync/vault') {
    if (method === 'POST') {
      if (typeof body.key_id !== 'string' || !keyIdPattern.test(body.key_id)) invalid();
      await env.DB.prepare('INSERT INTO sync_vaults (user_id, key_id) VALUES (?, ?) ON CONFLICT DO NOTHING')
        .bind(user, body.key_id).run();
    } else if (method !== 'GET') invalid();
    const vault = await env.DB.prepare('SELECT key_id FROM sync_vaults WHERE user_id = ?').bind(user).first();
    if (method === 'POST' && vault?.key_id !== body.key_id) {
      throw new HttpError(409, 'Use the existing recovery key', 'SYNC_KEY_MISMATCH');
    }
    return { vault };
  }
  const vault = await env.DB.prepare('SELECT key_id FROM sync_vaults WHERE user_id = ?').bind(user).first<{key_id: string}>();
  if (!vault) throw new HttpError(409, 'Create a sync vault first', 'SYNC_NO_VAULT');
  if (path === '/api/v1/sync/records' && method === 'GET') {
    const after = url.searchParams.get('after') || '';
    if (after && !uuid.test(after)) invalid();
    const rows = await env.DB.prepare('SELECT id, revision, updated_at FROM sync_records WHERE user_id = ? AND id > ? ORDER BY id LIMIT 100')
      .bind(user, after).all();
    return { records: rows.results,
      next: rows.results.length === 100 ? rows.results[99].id : null };
  }
  const match = path.match(/^\/api\/v1\/sync\/records\/([^/]+)(\/history)?$/);
  if (!match || !uuid.test(match[1])) invalid();
  const id = match[1];
  if (!match[2] && method === 'GET') {
    const row = await env.DB.prepare('SELECT id, revision, envelope, updated_at FROM sync_records WHERE user_id = ? AND id = ?').bind(user, id).first();
    if (!row) throw new HttpError(404, 'Sync record not found', 'SYNC_NOT_FOUND');
    return { ...row, envelope: JSON.parse(row.envelope as string) };
  }
  if (match[2] && method === 'GET') {
    const rows = await env.DB.prepare('SELECT revision, envelope, updated_at FROM sync_history WHERE user_id = ? AND id = ? ORDER BY revision DESC')
      .bind(user, id).all();
    return { records: rows.results.map(row => ({ ...row, id, envelope: JSON.parse(row.envelope as string) })) };
  }
  if (method !== 'POST' || match[2]) invalid();
  const base = body.base_revision;
  const e = body.envelope as Record<string, unknown> | undefined;
  if (!Number.isSafeInteger(base) || (base as number) < 0 || (base as number) > 1_000_000_000 || !e ||
    e.version !== 1 || e.key_id !== vault.key_id || typeof e.iv !== 'string' || !/^[A-Za-z0-9+/]{16}$/.test(e.iv) ||
    typeof e.ciphertext !== 'string' || e.ciphertext.length < 24 || e.ciphertext.length > 48_000 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(e.ciphertext)) invalid();
  const envelope = JSON.stringify({ version: 1, key_id: e.key_id, iv: e.iv, ciphertext: e.ciphertext });
  const now = new Date().toISOString();
  const result = base === 0
    ? await env.DB.prepare(`INSERT INTO sync_records (user_id, id, revision, envelope, updated_at)
        SELECT ?, ?, 1, ?, ? WHERE (SELECT COUNT(*) FROM sync_records WHERE user_id = ?) < 10000
        ON CONFLICT DO NOTHING RETURNING revision`).bind(user, id, envelope, now, user).first<{revision: number}>()
    : await env.DB.prepare('UPDATE sync_records SET revision = revision + 1, envelope = ?, updated_at = ? WHERE user_id = ? AND id = ? AND revision = ? RETURNING revision')
      .bind(envelope, now, user, id, base).first<{revision: number}>();
  if (!result) throw new HttpError(409, 'Record changed or vault quota reached; synchronize again', 'SYNC_CONFLICT');
  return result;
}
