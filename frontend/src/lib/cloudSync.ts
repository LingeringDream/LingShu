/* global CryptoKey, Storage, atob, localStorage */
import { apiFetch } from './api';
import { cloudAccountClient } from './cloudAccount';

export interface SyncData { id: string; content: string; deleted_at: string | null; [key: string]: unknown }
export interface LocalRecord { data: SyncData; token: string }
export interface Envelope { version: 1; key_id: string; iv: string; ciphertext: string }
export interface RemoteRecord { id: string; revision: number; envelope: Envelope }
type RemoteVersion = Pick<RemoteRecord, 'id' | 'revision'>;
interface Baseline { revision: number; hash: string }
export interface SyncConflict { local: LocalRecord; remote: RemoteVersion; data: SyncData }
export interface SyncKey { key: CryptoKey; id: string }
const encoder = new TextEncoder();
const hex = (bytes: Uint8Array) => Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
const b64 = (bytes: Uint8Array) => btoa(Array.from(bytes, b => String.fromCharCode(b)).join(''));
const unb64 = (value: string) => Uint8Array.from(atob(value), c => c.charCodeAt(0));
export const generateRecoveryKey = () => hex(crypto.getRandomValues(new Uint8Array(32)));

export async function importRecoveryKey(value: string): Promise<SyncKey> {
  const clean = value.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(clean)) throw new Error('请输入完整的 64 位恢复密钥。');
  const raw = Uint8Array.from(clean.match(/../g)!, pair => parseInt(pair, 16));
  const id = hex(new Uint8Array(await crypto.subtle.digest('SHA-256', raw)));
  const key = await crypto.subtle.importKey('raw', raw, 'AES-GCM', false, ['encrypt', 'decrypt']);
  raw.fill(0);
  return { key, id };
}

function aad(user: string, id: string, revision: number, key: SyncKey) {
  return encoder.encode(JSON.stringify(['lingshu-sync', 1, user, id, revision, key.id]));
}
export async function encryptRecord(key: SyncKey, user: string, data: SyncData, revision: number): Promise<Envelope> {
  const plaintext = encoder.encode(JSON.stringify(data));
  if (plaintext.length > 35_000) throw new Error('单条记忆超过 35 KB，同步已暂停；本地内容不受影响。');
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv, additionalData: aad(user, data.id, revision, key) }, key.key, plaintext);
  return { version: 1, key_id: key.id, iv: b64(iv), ciphertext: b64(new Uint8Array(ciphertext)) };
}
export async function decryptRecord(key: SyncKey, user: string, record: RemoteRecord): Promise<SyncData> {
  try {
    const e = record.envelope;
    if (e.version !== 1 || e.key_id !== key.id || !Number.isSafeInteger(record.revision) || record.revision < 1 ||
      e.iv.length !== 16 || e.ciphertext.length > 48_000) throw new Error();
    const bytes = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: unb64(e.iv), additionalData: aad(user, record.id, record.revision, key) }, key.key, unb64(e.ciphertext));
    const data = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
    if (data.id !== record.id || typeof data.content !== 'string' || !(data.deleted_at === null || typeof data.deleted_at === 'string')) throw new Error();
    return data;
  } catch { throw new Error('密文验证失败：恢复密钥不正确或云端记录已损坏。本地记录未被该密文覆盖。'); }
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (value && typeof value === 'object') return '{' + Object.keys(value).sort().map(k => JSON.stringify(k) + ':' + canonical((value as Record<string, unknown>)[k])).join(',') + '}';
  return JSON.stringify(value);
}
export async function fingerprint(data: SyncData): Promise<string> {
  return hex(new Uint8Array(await crypto.subtle.digest('SHA-256', encoder.encode(canonical(data)))));
}
export function direction(local: string | undefined, remote: string | undefined, revision: number, base?: Baseline): 'same' | 'push' | 'pull' | 'conflict' {
  if (local === remote) return 'same';
  if (local === undefined) return 'pull';
  if (remote === undefined) return base ? 'conflict' : 'push';
  if (base && local === base.hash) return 'pull';
  if (base && revision === base.revision) return 'push';
  return 'conflict';
}

async function localRequest<T>(path: string, body?: unknown): Promise<T> {
  const response = await apiFetch('/api/v1/cloud-sync/memories' + path, body === undefined ? {} : {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  });
  if (!response.ok) {
    const error = await response.json().catch(() => null);
    throw new Error(error?.error?.message || '本地记忆读写失败，请重新同步。');
  }
  return response.json();
}

export interface SyncIO {
  cloud<T>(path: string, body?: unknown): Promise<T>;
  local<T>(path: string, body?: unknown): Promise<T>;
  storage: Pick<Storage, 'getItem' | 'setItem'>;
}
export class MemorySync {
  private baseline: Record<string, Baseline> = {};
  private storageKey = '';
  private localOwner = '';
  private running = false;
  private io: SyncIO;
  constructor(private user: string, private key: SyncKey, private signal: AbortSignal, io?: SyncIO) {
    this.io = io || { cloud: (path, body) => cloudAccountClient.syncRequest(user, path, body), local: localRequest, storage: localStorage };
  }
  private active() { this.signal.throwIfAborted(); }
  private save(id: string, revision: number, hash: string) {
    if (this.baseline[id]?.revision === revision && this.baseline[id]?.hash === hash) return;
    this.baseline[id] = { revision, hash };
    this.io.storage.setItem(this.storageKey, JSON.stringify(this.baseline));
  }
  private async push(local: LocalRecord, revision: number) {
    this.active();
    const envelope = await encryptRecord(this.key, this.user, local.data, revision + 1);
    this.active();
    const result = await this.io.cloud<{revision: number}>('/sync/records/' + local.data.id, { base_revision: revision, envelope });
    this.save(local.data.id, result.revision, await fingerprint(local.data));
  }
  private async pull(remote: RemoteVersion, data: SyncData, local?: LocalRecord) {
    this.active();
    await this.io.local('/' + remote.id, { data, expected_token: local?.token ?? null, user_id: this.localOwner });
    this.save(remote.id, remote.revision, await fingerprint(data));
  }
  async run(): Promise<{ uploaded: number; downloaded: number; conflicts: SyncConflict[] }> {
    if (this.running) throw new Error('同步正在进行。');
    this.running = true;
    try {
      this.active();
      const vault = await this.io.cloud<{vault: {key_id: string} | null}>('/sync/vault');
      if (!vault.vault || vault.vault.key_id !== this.key.id) throw new Error('请先使用原恢复密钥解锁云同步。');
      const local = new Map<string, LocalRecord>();
      let after: string | null = null;
      let owner = '';
      do {
        this.active();
        const page: { user_id: string; records: LocalRecord[]; next: string | null } = await this.io.local(after ? '?after=' + after : '');
        if (owner && owner !== page.user_id) throw new Error('本地账户已切换，请重新同步。');
        owner = page.user_id;
        for (const r of page.records) local.set(r.data.id, r);
        after = page.next;
      } while (after);
      this.storageKey = `lingshu_sync_v1:${this.user}:${owner}:${this.key.id}`;
      this.localOwner = owner;
      const saved = JSON.parse(this.io.storage.getItem(this.storageKey) || '{}');
      if (!saved || typeof saved !== 'object' || Array.isArray(saved) || Object.values(saved).some(b => !b || typeof b !== 'object' || !Number.isSafeInteger((b as Baseline).revision) || (b as Baseline).revision < 1 || !/^[0-9a-f]{64}$/.test((b as Baseline).hash))) {
        throw new Error('本地同步索引损坏，请先备份数据后清理同步索引。');
      }
      this.baseline = saved;
      const remote = new Map<string, { record: RemoteVersion; data?: SyncData; hash: string }>();
      after = null;
      do {
        this.active();
        const page: { records: Array<{id: string; revision: number}>; next: string | null } = await this.io.cloud('/sync/records' + (after ? '?after=' + after : ''));
        for (const r of page.records) {
          if (this.baseline[r.id] && r.revision < this.baseline[r.id].revision) throw new Error('云端版本回退，同步已停止。');
          const base = this.baseline[r.id];
          if (base?.revision === r.revision && local.has(r.id)) {
            remote.set(r.id, { record: r, hash: base.hash });
          } else {
            this.active();
            const record = await this.io.cloud<RemoteRecord>('/sync/records/' + r.id);
            if (record.revision < r.revision) throw new Error('云端版本回退，同步已停止。');
            const data = await decryptRecord(this.key, this.user, record);
            remote.set(r.id, { record, data, hash: await fingerprint(data) });
          }
        }
        after = page.next;
      } while (after);
      // Authenticate all downloaded ciphertext before applying anything locally.
      const result = { uploaded: 0, downloaded: 0, conflicts: [] as SyncConflict[] };
      for (const id of new Set([...local.keys(), ...remote.keys()])) {
        this.active();
        const l = local.get(id), r = remote.get(id);
        const hash = l ? await fingerprint(l.data) : undefined;
        const action = direction(hash, r?.hash, r?.record.revision || 0, this.baseline[id]);
        if (action === 'same' && r) this.save(id, r.record.revision, r.hash);
        else if (action === 'push' && l) { await this.push(l, r?.record.revision || 0); result.uploaded++; }
        else if (action === 'pull' && r?.data) { await this.pull(r.record, r.data, l); result.downloaded++; }
        else if (action === 'conflict') {
          if (!l || !r?.data) throw new Error('已同步的云端记录缺失，同步已停止。');
          result.conflicts.push({ local: l, remote: r.record, data: r.data });
        }
      }
      return result;
    } finally { this.running = false; }
  }
  async resolve(conflict: SyncConflict, choice: 'local' | 'cloud') {
    if (this.running) throw new Error('同步正在进行。');
    this.running = true;
    try {
      if (choice === 'local') await this.push(conflict.local, conflict.remote.revision);
      else await this.pull(conflict.remote, conflict.data, conflict.local);
    } finally { this.running = false; }
  }
}
