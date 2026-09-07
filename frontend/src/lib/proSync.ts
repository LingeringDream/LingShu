import { apiFetch } from './api';
import { cloudAccountClient, CloudAccountError } from './cloudAccount';

export interface SyncMemoryRecord {
  id: string;
  memory_type: string;
  content: string;
  importance: number;
  metadata: Record<string, unknown>;
  source_memory_ids?: string[];
  tier?: string;
  memory_layer?: string;
  owner_agent_id?: string;
  memory_scope?: string;
  scene_tags?: string[];
  episode_id?: string | null;
  occurred_at?: string;
  valid_from?: string;
  valid_to?: string | null;
  source_type?: string;
  source_trust?: number;
  fact_key?: string | null;
  fact_value?: string | null;
  conflict_status?: string;
  superseded_by?: string | null;
  retention_status?: string;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
}

interface SyncedSetting { value: unknown; updated_at: string }
interface SyncBundle { version: 1; exported_at: string; memories: SyncMemoryRecord[]; settings?: { retrieval_weights?: SyncedSetting } }
interface EncryptedSnapshot {
  algorithm: 'AES-256-GCM'; key_version: 1; iv: string; ciphertext: string;
  checksum: string; item_count: number;
}
interface CloudSyncState {
  revision: number; updated_at?: string; device_id?: string; snapshot: EncryptedSnapshot | null;
}
export interface SyncResult { revision: number; changed: number; itemCount: number; uploaded: boolean; updatedAt?: string }

let autoSyncContext: { passphrase: string; userId: string } | null = null;
let autoSyncTimer: number | null = null;
const runAutoSync = () => {
  if (autoSyncContext && globalThis.navigator.onLine) {
    void synchronizeProMemories(autoSyncContext.passphrase, autoSyncContext.userId)
      .catch(error => console.warn('[pro-sync] background sync failed:', error));
  }
};

export function startProAutoSync(passphrase: string, userId: string): void {
  autoSyncContext = { passphrase, userId };
  if (autoSyncTimer === null) {
    autoSyncTimer = window.setInterval(runAutoSync, 5 * 60_000);
    window.addEventListener('online', runAutoSync);
  }
}

export function stopProAutoSync(): void {
  autoSyncContext = null;
  if (autoSyncTimer !== null) window.clearInterval(autoSyncTimer);
  autoSyncTimer = null;
  window.removeEventListener('online', runAutoSync);
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function arrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

function base64Url(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function fromBase64Url(value: string): Uint8Array {
  const padded = value.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - value.length % 4) % 4);
  const binary = globalThis.atob(padded);
  return Uint8Array.from(binary, char => char.charCodeAt(0));
}

async function digestHex(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', arrayBuffer(bytes)));
  return Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}

async function deriveKey(passphrase: string, userId: string) {
  if (passphrase.length < 12) throw new Error('同步加密口令至少需要 12 个字符。');
  const material = await crypto.subtle.importKey('raw', encoder.encode(passphrase), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey({ name: 'PBKDF2', salt: encoder.encode(`lingshu-sync-v1:${userId}`), iterations: 250_000, hash: 'SHA-256' },
    material, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
}

export async function encryptSyncBundle(bundle: SyncBundle, passphrase: string, userId: string): Promise<EncryptedSnapshot> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const plaintext = encoder.encode(JSON.stringify(bundle));
  const encrypted = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv: arrayBuffer(iv) }, await deriveKey(passphrase, userId), arrayBuffer(plaintext)));
  return { algorithm: 'AES-256-GCM', key_version: 1, iv: base64Url(iv), ciphertext: base64Url(encrypted),
    checksum: await digestHex(encrypted), item_count: bundle.memories.length };
}

export async function decryptSyncBundle(snapshot: EncryptedSnapshot, passphrase: string, userId: string): Promise<SyncBundle> {
  try {
    const encrypted = fromBase64Url(snapshot.ciphertext);
    if (await digestHex(encrypted) !== snapshot.checksum.toLowerCase()) throw new Error('checksum');
    const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: arrayBuffer(fromBase64Url(snapshot.iv)) },
      await deriveKey(passphrase, userId), arrayBuffer(encrypted));
    const bundle = JSON.parse(decoder.decode(plaintext)) as SyncBundle;
    if (bundle.version !== 1 || !Array.isArray(bundle.memories)) throw new Error('unsupported');
    return bundle;
  } catch (error) {
    if (error instanceof Error && error.message.includes('至少需要')) throw error;
    throw new Error('无法解密云端记忆。请确认同步加密口令与其他设备一致。');
  }
}

export function mergeSyncMemories(local: SyncMemoryRecord[], remote: SyncMemoryRecord[]): SyncMemoryRecord[] {
  const merged = new Map<string, SyncMemoryRecord>();
  for (const memory of [...remote, ...local]) {
    const prior = merged.get(memory.id);
    if (!prior || memory.updated_at > prior.updated_at ||
      (memory.updated_at === prior.updated_at && JSON.stringify(memory) > JSON.stringify(prior))) merged.set(memory.id, memory);
  }
  return [...merged.values()].sort((a, b) => a.updated_at.localeCompare(b.updated_at) || a.id.localeCompare(b.id));
}

async function localExport(): Promise<SyncMemoryRecord[]> {
  const response = await apiFetch('/api/v1/memories-sync');
  if (!response.ok) throw new Error(`导出本地记忆失败（HTTP ${response.status}）`);
  return response.json();
}

function localSettings(): SyncBundle['settings'] {
  try {
    const value = JSON.parse(window.localStorage.getItem('lingshu_pro_retrieval_weights') || 'null') as SyncedSetting | null;
    return value?.updated_at ? { retrieval_weights: value } : {};
  } catch { return {}; }
}

function mergeSettings(local: SyncBundle['settings'], remote: SyncBundle['settings']): SyncBundle['settings'] {
  const left = local?.retrieval_weights;
  const right = remote?.retrieval_weights;
  const winner = !left ? right : !right ? left : left.updated_at >= right.updated_at ? left : right;
  if (winner) window.localStorage.setItem('lingshu_pro_retrieval_weights', JSON.stringify(winner));
  return winner ? { retrieval_weights: winner } : {};
}

async function localImport(memories: SyncMemoryRecord[]): Promise<number> {
  const response = await apiFetch('/api/v1/memories-sync', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ memories }) });
  if (!response.ok) throw new Error(`合并本地记忆失败（HTTP ${response.status}）`);
  return Number((await response.json()).changed || 0);
}

export async function synchronizeProMemories(passphrase: string, userId: string): Promise<SyncResult> {
  let local = await localExport();
  let settings = localSettings();
  let changed = 0;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const cloud = await cloudAccountClient.readEncryptedSync<CloudSyncState>(userId);
    let merged = local;
    if (cloud.snapshot) {
      const remote = await decryptSyncBundle(cloud.snapshot, passphrase, userId);
      merged = mergeSyncMemories(local, remote.memories);
      settings = mergeSettings(settings, remote.settings);
      changed += await localImport(merged);
      local = await localExport();
      if (JSON.stringify(local) === JSON.stringify(remote.memories) && JSON.stringify(settings) === JSON.stringify(remote.settings || {})) {
        return { revision: cloud.revision, changed, itemCount: local.length, uploaded: false, updatedAt: cloud.updated_at };
      }
    }
    const snapshot = await encryptSyncBundle({ version: 1, exported_at: new Date().toISOString(), memories: merged, settings }, passphrase, userId);
    try {
      const result = await cloudAccountClient.writeEncryptedSync<{ revision: number; updated_at: string }>({ base_revision: cloud.revision, snapshot }, userId);
      return { revision: result.revision, changed, itemCount: merged.length, uploaded: true, updatedAt: result.updated_at };
    } catch (error) {
      if (!(error instanceof CloudAccountError) || error.status !== 409) throw error;
    }
  }
  throw new Error('多台设备仍在同时写入，请稍后再次同步。');
}
