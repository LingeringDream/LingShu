import { useEffect, useRef, useState } from 'react';
import { cloudAccountClient, cloudErrorMessage } from '../../lib/cloudAccount';
import { decryptRecord, generateRecoveryKey, importRecoveryKey, MemorySync, type RemoteRecord, type SyncConflict, type SyncKey } from '../../lib/cloudSync';
import { isTauri } from '../../lib/tauri';

async function storedKey(userId: string, action: 'load' | 'save' | 'clear', value?: string): Promise<string | null> {
  if (!isTauri()) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke(`sync_key_${action}`, { userId, ...(value ? { value } : {}) });
}

export function CloudSyncPanel({ userId, enabled }: { userId: string; enabled: boolean }) {
  const [recovery, setRecovery] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [unlocked, setUnlocked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [conflicts, setConflicts] = useState<SyncConflict[]>([]);
  const [history, setHistory] = useState<string[]>([]);
  const engine = useRef<MemorySync>();
  const key = useRef<SyncKey>();
  const lifetime = useRef(new AbortController());

  useEffect(() => {
    const controller = new AbortController();
    lifetime.current = controller;
    if (enabled) void storedKey(userId, 'load').then(async value => {
      if (!value || controller.signal.aborted) return;
      const loaded = await importRecoveryKey(value);
      if (controller.signal.aborted) return;
      key.current = loaded;
      engine.current = new MemorySync(userId, loaded, controller.signal);
      setUnlocked(true);
    }).catch(failure => { if (!controller.signal.aborted) setError(cloudErrorMessage(failure)); });
    return () => { controller.abort(); engine.current = undefined; key.current = undefined; };
  }, [userId, enabled]);

  async function perform(operation: () => Promise<void>) {
    setBusy(true); setError(''); setHistory([]);
    try { await operation(); }
    catch (failure) { if (!lifetime.current.signal.aborted) setError(cloudErrorMessage(failure)); }
    finally { if (!lifetime.current.signal.aborted) setBusy(false); }
  }

  async function unlock() {
    const loaded = await importRecoveryKey(recovery);
    const result = await cloudAccountClient.syncRequest<{vault: {key_id: string} | null}>(userId, '/sync/vault');
    if (!result.vault && !confirmed) throw new Error('首次开启前，请先保存恢复密钥并勾选确认。');
    if (result.vault && result.vault.key_id !== loaded.id) throw new Error('恢复密钥不匹配，请输入原设备上的恢复密钥。');
    await cloudAccountClient.syncRequest(userId, '/sync/vault', { key_id: loaded.id });
    lifetime.current.signal.throwIfAborted();
    await storedKey(userId, 'save', recovery.trim().toLowerCase());
    lifetime.current.signal.throwIfAborted();
    key.current = loaded;
    engine.current = new MemorySync(userId, loaded, lifetime.current.signal);
    setRecovery(''); setUnlocked(true); setStatus('已解锁，点击「立即同步」同步个人私有记忆。');
  }

  async function sync() {
    const result = await engine.current!.run();
    lifetime.current.signal.throwIfAborted();
    setConflicts(result.conflicts);
    setStatus(`已上传 ${result.uploaded} 条、下载 ${result.downloaded} 条；${result.conflicts.length ? `${result.conflicts.length} 条冲突等待处理` : '同步完成'}。`);
  }

  async function resolve(conflict: SyncConflict, choice: 'local' | 'cloud') {
    await engine.current!.resolve(conflict, choice);
    lifetime.current.signal.throwIfAborted();
    setConflicts(items => items.filter(item => item.remote.id !== conflict.remote.id));
    setStatus('冲突已处理。可再次同步以检查其他设备的新修改。');
  }

  return <section className="cloud-sync-panel" aria-labelledby="cloud-sync-title">
    <h3 id="cloud-sync-title">加密云同步</h3>
    <p className="cloud-account-muted">手动同步个人私有记忆及删除状态，支持多设备增量传输。聊天、项目与团队数据不在此同步范围内。</p>
    {!enabled ? <p>需要有效的 Pro 或 Team 权益，刷新账户信息后可查看是否可用。</p> : <>
      {!unlocked ? <div className="cloud-account-form">
        <label htmlFor="sync-recovery">恢复密钥<input id="sync-recovery" type="password" autoComplete="off" spellCheck={false} value={recovery} onChange={event => { setRecovery(event.target.value); setConfirmed(false); }} disabled={busy} /></label>
        <div className="cloud-account-actions"><button type="button" disabled={busy} onClick={() => { setRecovery(generateRecoveryKey()); setConfirmed(false); }}>首次使用：生成密钥</button></div>
        {recovery && <details><summary>查看并备份恢复密钥</summary><code className="cloud-sync-key">{recovery}</code></details>}
        <label className="cloud-sync-confirm"><input type="checkbox" checked={confirmed} onChange={event => setConfirmed(event.target.checked)} disabled={busy} />我已将恢复密钥保存在安全位置</label>
        <p className="cloud-account-muted">新设备需输入同一密钥。云端无法恢复丢失的密钥；登录密码不能代替它。桌面端将密钥保存在系统凭据保险库，浏览器模式只在当前页面保留。</p>
        <div className="cloud-account-actions"><button type="button" disabled={busy || !recovery} onClick={() => void perform(unlock)}>解锁 / 开启同步</button></div>
      </div> : <div className="cloud-account-actions">
        <button className="cloud-account-primary" type="button" disabled={busy || conflicts.length > 0} onClick={() => void perform(sync)}>{busy ? '正在处理…' : '立即同步'}</button>
        {conflicts.length > 0 && <button type="button" disabled={busy} onClick={() => void perform(sync)}>重新检查冲突</button>}
        <button type="button" disabled={busy} onClick={() => void perform(async () => {
          await storedKey(userId, 'clear'); key.current = undefined; engine.current = undefined;
          setUnlocked(false); setConflicts([]); setStatus('本机同步密钥已移除；本地记忆和云端密文保留。');
        })}>移除本机同步密钥</button>
      </div>}
      {status && <p role="status">{status}</p>}
      {error && <p role="alert" className="cloud-account-error">{error}</p>}
      {conflicts.map(conflict => <article className="cloud-sync-conflict" key={conflict.remote.id}>
        <h4>记忆冲突</h4>
        <p><strong>本机{conflict.local.data.deleted_at ? '（已删除）' : ''}</strong>：{conflict.local.data.content}</p>
        <p><strong>云端{conflict.data.deleted_at ? '（已删除）' : ''}</strong>：{conflict.data.content}</p>
        <details><summary>查看完整记忆属性</summary>
          <pre className="cloud-sync-properties">本机：{JSON.stringify(conflict.local.data, null, 2)}</pre>
          <pre className="cloud-sync-properties">云端：{JSON.stringify(conflict.data, null, 2)}</pre>
        </details>
        <p className="cloud-account-muted">选择后将用所选版本覆盖另一版本。云端保留最近 10 个已同步的旧版本。</p>
        <div className="cloud-account-actions">
          <button type="button" disabled={busy} onClick={() => void perform(() => resolve(conflict, 'local'))}>保留本机版本</button>
          <button type="button" disabled={busy} onClick={() => void perform(() => resolve(conflict, 'cloud'))}>使用云端版本</button>
          <button type="button" disabled={busy} onClick={() => void perform(async () => {
            const result = await cloudAccountClient.syncRequest<{records: RemoteRecord[]}>(userId, `/sync/records/${conflict.remote.id}/history`);
            const entries = await Promise.all(result.records.map(async record => `版本 ${record.revision}：${(await decryptRecord(key.current!, userId, record)).content}`));
            setHistory(entries.length ? entries : ['暂无历史版本。']);
          })}>查看云端历史</button>
        </div>
      </article>)}
      {history.length > 0 && <ul className="cloud-sync-history">{history.map((entry, index) => <li key={index}>{entry}</li>)}</ul>}
    </>}
  </section>;
}
