import { useCallback, useState } from 'react';
import { cloudAccountClient, cloudErrorMessage } from '../../lib/cloudAccount';
import { startProAutoSync, stopProAutoSync, synchronizeProMemories, type SyncResult } from '../../lib/proSync';

const AUTO_SYNC_KEY = 'lingshu_pro_auto_sync';

interface HistoryItem { revision: number; item_count: number; updated_at: string; device_id: string }

export function ProSyncPanel({ userId }: { userId: string }) {
  const [passphrase, setPassphrase] = useState('');
  const [autoSync, setAutoSync] = useState(() => window.localStorage.getItem(AUTO_SYNC_KEY) === 'true');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SyncResult | null>(null);
  const [history, setHistory] = useState<HistoryItem[]>([]);

  const loadHistory = useCallback(async () => {
    const data = await cloudAccountClient.readSyncHistory<{ history: HistoryItem[] }>();
    setHistory(data.history);
  }, []);

  const sync = useCallback(async (quiet = false) => {
    if (busy || !passphrase) return;
    setBusy(true);
    if (!quiet) setError(null);
    try {
      setResult(await synchronizeProMemories(passphrase, userId));
      if (autoSync) startProAutoSync(passphrase, userId);
      await loadHistory();
    } catch (failure) {
      setError(cloudErrorMessage(failure));
    } finally { setBusy(false); }
  }, [autoSync, busy, loadHistory, passphrase, userId]);

  function toggleAuto(value: boolean) {
    setAutoSync(value);
    window.localStorage.setItem(AUTO_SYNC_KEY, String(value));
    if (!value) stopProAutoSync();
    else if (passphrase.length >= 12) startProAutoSync(passphrase, userId);
  }

  return <div className="cloud-account-note pro-sync-panel" aria-label="Pro 加密云同步">
    <div className="pro-sync-heading"><div><strong>Pro 端到端加密同步</strong><p>合并这台设备与云端的记忆、删除记录和修改时间；本机 MCP 会立即使用合并后的记忆。</p></div>
      <span className="cloud-account-verified">已开放</span></div>
    <label className="pro-sync-secret" htmlFor="pro-sync-passphrase">同步加密口令
      <input id="pro-sync-passphrase" type="password" autoComplete="off" minLength={12} maxLength={128}
        placeholder="至少 12 个字符；所有设备必须一致" value={passphrase} onChange={event => setPassphrase(event.target.value)} disabled={busy} />
      <span>口令只在本次运行期间保存在内存中，灵枢云端无法读取或找回。首次同步前请妥善备份。</span>
    </label>
    <label className="pro-sync-auto"><input type="checkbox" checked={autoSync} onChange={event => toggleAuto(event.target.checked)} />解锁后每 5 分钟及恢复联网时自动同步</label>
    {error && <p className="cloud-account-error" role="alert">{error}</p>}
    {result && <p className="pro-sync-result" role="status">修订 {result.revision} · {result.itemCount} 条记忆 · {result.changed ? `本机合并 ${result.changed} 项` : result.uploaded ? '云端已更新' : '已是最新'}</p>}
    <div className="cloud-account-actions">
      <button className="cloud-account-primary" type="button" disabled={busy || passphrase.length < 12} onClick={() => void sync()}>{busy ? '正在加密并合并…' : '立即双向同步'}</button>
      <button type="button" disabled={busy} onClick={() => void loadHistory().catch(failure => setError(cloudErrorMessage(failure)))}>查看同步历史</button>
    </div>
    {history.length > 0 && <details className="pro-sync-history"><summary>最近同步（{history.length}）</summary><ol>{history.slice(0, 10).map(item =>
      <li key={item.revision}><span>修订 {item.revision} · {item.item_count} 条</span><time>{new Date(item.updated_at).toLocaleString('zh-CN')}</time></li>)}</ol></details>}
  </div>;
}
