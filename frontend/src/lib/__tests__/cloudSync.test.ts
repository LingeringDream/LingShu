// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { decryptRecord, direction, encryptRecord, fingerprint, generateRecoveryKey, importRecoveryKey, MemorySync, type LocalRecord, type RemoteRecord, type SyncData, type SyncIO } from '../cloudSync';

const user = 'user-a';
const id = '11111111-1111-4111-8111-111111111111';
const data = (content = '私人记忆'): SyncData => ({ id, content, deleted_at: null });

describe('end-to-end encryption', () => {
  it('uses random authenticated encryption and binds account, record and revision', async () => {
    const key = await importRecoveryKey(generateRecoveryKey());
    const envelope = await encryptRecord(key, user, data(), 1);
    const record: RemoteRecord = { id, revision: 1, envelope };
    expect(await decryptRecord(key, user, record)).toEqual(data());
    expect(envelope.ciphertext).not.toContain('私人记忆');
    expect(await encryptRecord(key, user, data(), 1)).not.toEqual(envelope);
    for (const altered of [{ ...record, id: 'other' }, { ...record, revision: 2 }, { ...record, envelope: { ...envelope, ciphertext: 'A' + envelope.ciphertext.slice(1, -4) + 'AAAA' } }]) {
      await expect(decryptRecord(key, user, altered)).rejects.toThrow('密文验证失败');
    }
    await expect(decryptRecord(key, 'user-b', record)).rejects.toThrow('密文验证失败');
    await expect(decryptRecord(await importRecoveryKey(generateRecoveryKey()), user, record)).rejects.toThrow('密文验证失败');
    await expect(importRecoveryKey('weak password')).rejects.toThrow('64');
    await expect(encryptRecord(key, user, data('a'.repeat(35000)), 1)).rejects.toThrow('35 KB');
  });
  it('fingerprints JSON independently of property order', async () => {
    expect(await fingerprint(data())).toBe(await fingerprint({ deleted_at: null, content: '私人记忆', id }));
  });
  it('detects edit/edit and edit/delete conflicts without using wall clocks', () => {
    const base = { hash: 'old', revision: 1 };
    expect(direction('edited', 'deleted', 2, base)).toBe('conflict');
    expect(direction('old', 'deleted', 2, base)).toBe('pull');
    expect(direction('edited', 'old', 1, base)).toBe('push');
    expect(direction('edited', 'other', 1)).toBe('conflict');
    expect(direction('old', undefined, 0, base)).toBe('conflict');
  });
});

async function fixture() {
  const key = await importRecoveryKey(generateRecoveryKey());
  const cloud = new Map<string, RemoteRecord>();
  let transfers = 0;
  const device = (initial?: SyncData) => {
    const records = new Map<string, LocalRecord>();
    if (initial) records.set(initial.id, { data: structuredClone(initial), token: 'initial' });
    const values = new Map<string, string>();
    const controller = new AbortController();
    const io: SyncIO = {
      storage: { getItem: k => values.get(k) || null, setItem: (k, v) => { values.set(k, v); } },
      async cloud<T>(path: string, body?: unknown): Promise<T> {
        if (path === '/sync/vault') return { vault: { key_id: key.id } } as T;
        if (path === '/sync/records') return { records: [...cloud.values()].map(r => ({ id: r.id, revision: r.revision })), next: null } as T;
        const id = path.split('/').pop()!;
        if (body) {
          const b = body as {base_revision: number; envelope: RemoteRecord['envelope']};
          if ((cloud.get(id)?.revision || 0) !== b.base_revision) throw new Error('SYNC_CONFLICT');
          cloud.set(id, { id, revision: b.base_revision + 1, envelope: b.envelope });
          transfers++;
          return { revision: b.base_revision + 1 } as T;
        }
        transfers++;
        return structuredClone(cloud.get(id)) as T;
      },
      async local<T>(path: string, body?: unknown): Promise<T> {
        if (!path) return { user_id: 'local-owner', records: [...records.values()].map(r => structuredClone(r)), next: null } as T;
        const b = body as { data: SyncData; expected_token: string | null; user_id: string };
        expect(b.user_id).toBe('local-owner');
        if ((records.get(b.data.id)?.token || null) !== b.expected_token) throw new Error('LOCAL_CONFLICT');
        records.set(b.data.id, { data: structuredClone(b.data), token: crypto.randomUUID() });
        return {} as T;
      },
    };
    return { records, io, controller, engine: new MemorySync(user, key, controller.signal, io),
      edit(content: string, deleted = false) {
        records.set(id, { data: { ...data(content), deleted_at: deleted ? '2026-09-07T00:00:00Z' : null }, token: crypto.randomUUID() });
      } };
  };
  return { cloud, device, transfers: () => transfers };
}

describe('multi-device sync', () => {
  it('round-trips additions, incrementally skips unchanged ciphertext and propagates tombstones', async () => {
    const f = await fixture(), a = f.device(data()), b = f.device();
    expect((await a.engine.run()).uploaded).toBe(1);
    expect((await b.engine.run()).downloaded).toBe(1);
    expect(b.records.get(id)?.data).toEqual(data());
    const transfers = f.transfers();
    await a.engine.run(); await b.engine.run();
    expect(f.transfers()).toBe(transfers);
    a.edit('私人记忆', true);
    await a.engine.run(); await b.engine.run();
    expect(b.records.get(id)?.data.deleted_at).not.toBeNull();
  });
  it('preserves both sides of a conflict, rejects stale local writes, and resolves with CAS', async () => {
    const f = await fixture(), a = f.device(data()), b = f.device();
    await a.engine.run(); await b.engine.run();
    a.edit('from A'); b.edit('from B');
    await a.engine.run();
    const result = await b.engine.run();
    expect(result.conflicts).toHaveLength(1);
    expect(result.conflicts[0].data.content).toBe('from A');
    expect(b.records.get(id)?.data.content).toBe('from B');
    b.edit('B newer');
    await expect(b.engine.resolve(result.conflicts[0], 'cloud')).rejects.toThrow('LOCAL_CONFLICT');
    const fresh = await b.engine.run();
    await b.engine.resolve(fresh.conflicts[0], 'local');
    await a.engine.run();
    expect(a.records.get(id)?.data.content).toBe('B newer');
  });
  it('recovers after an acknowledged upload loses its response', async () => {
    const f = await fixture(), a = f.device(data());
    const send = a.io.cloud;
    let once = true;
    a.io.cloud = async (path, body) => {
      const result = await send(path, body);
      if (body && once) { once = false; throw new Error('network disconnected'); }
      return result as never;
    };
    await expect(a.engine.run()).rejects.toThrow('network disconnected');
    expect((await a.engine.run()).conflicts).toHaveLength(0);
    expect(f.cloud.get(id)?.revision).toBe(1);
  });
  it('stops on rollback, corruption or cancellation without overwriting local data', async () => {
    const f = await fixture(), a = f.device(data()), b = f.device();
    await a.engine.run(); await b.engine.run();
    a.edit('new'); await a.engine.run();
    const latest = f.cloud.get(id)!;
    f.cloud.set(id, { ...latest, envelope: { ...latest.envelope, iv: 'AAAAAAAAAAAAAAAA' } });
    await expect(b.engine.run()).rejects.toThrow('密文验证失败');
    expect(b.records.get(id)?.data.content).toBe('私人记忆');
    f.cloud.set(id, { ...latest, revision: 1 });
    await expect(a.engine.run()).rejects.toThrow('版本回退');
    b.controller.abort();
    await expect(b.engine.run()).rejects.toThrow();
  });
});
