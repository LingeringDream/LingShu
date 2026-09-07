import { describe, expect, it } from 'vitest';
import { decryptSyncBundle, encryptSyncBundle, mergeSyncMemories, type SyncMemoryRecord } from '../proSync';

const record = (id: string, updated: string, content: string, deleted_at: string | null = null): SyncMemoryRecord => ({
  id, memory_type: 'fact', content, importance: 0.7, metadata: {}, deleted_at,
  created_at: '2026-09-01T00:00:00Z', updated_at: updated,
});

describe('Pro encrypted sync', () => {
  it('round trips an encrypted bundle and rejects a different passphrase', async () => {
    const bundle = { version: 1 as const, exported_at: '2026-09-07T00:00:00Z', memories: [record('a', '2026-09-07T00:00:00Z', 'secret')] };
    const encrypted = await encryptSyncBundle(bundle, 'correct horse battery', 'user-1');
    expect(encrypted.ciphertext).not.toContain('secret');
    await expect(decryptSyncBundle(encrypted, 'correct horse battery', 'user-1')).resolves.toEqual(bundle);
    await expect(decryptSyncBundle(encrypted, 'different passphrase', 'user-1')).rejects.toThrow('无法解密');
  });

  it('uses last-write-wins and preserves deletion tombstones', () => {
    const merged = mergeSyncMemories(
      [record('same', '2026-09-07T02:00:00Z', 'deleted', '2026-09-07T02:00:00Z')],
      [record('same', '2026-09-07T01:00:00Z', 'old'), record('remote', '2026-09-07T01:00:00Z', 'remote')],
    );
    expect(merged).toHaveLength(2);
    expect(merged.find(item => item.id === 'same')?.deleted_at).not.toBeNull();
  });
});
