import { useCallback, useEffect, useRef, useState } from 'react';
import { cloudAccountClient } from '../../lib/cloudAccount';
import { useCloudAccountStore } from '../../stores/cloudAccountStore';
import './TeamWorkspace.css';

type Role = 'owner' | 'admin' | 'editor' | 'viewer';
interface Team { id: string; name: string; role: Role }
interface Member { cloud_user_id: string; email: string; role: Role }
interface AgentKey { id: string; agent_key: string; name: string; permission: string; expires_at: string; revoked_at: string | null }
interface Invite { id: string; email: string; role: Role; expires_at: string; used_at: string | null; revoked_at: string | null }
interface Audit { id: string; actor: string; action: string; target: string; created_at: string }
interface Memory { id: string; content: string; memory_layer: string; memory_scope: string; owner_agent_id: string }
const roles: Record<Role, string> = { owner: '所有者', admin: '管理员', editor: '编辑者', viewer: '只读成员' };
async function request<T>(path: string, body?: unknown, method = body === undefined ? 'GET' : 'POST'): Promise<T> {
  const response = await cloudAccountClient.teamRequest(path, { method, ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  if (!response.ok) { const data = await response.json().catch(() => null); throw new Error(data?.error?.message ?? `请求失败 (${response.status})`); }
  return response.status === 204 ? undefined as T : response.json();
}
export function TeamWorkspace() {
  const identity = useCloudAccountStore(s => s.account?.user.id);
  return <TeamWorkspaceSession key={identity ?? 'signed-out'} />;
}
function TeamWorkspaceSession() {
  const { account, initialized, initialize } = useCloudAccountStore();
  const [teams, setTeams] = useState<Team[]>([]);
  const [selected, setSelected] = useState('');
  const [members, setMembers] = useState<Member[]>([]);
  const [keys, setKeys] = useState<AgentKey[]>([]);
  const [invites, setInvites] = useState<Invite[]>([]);
  const [audit, setAudit] = useState<Audit[]>([]);
  const [memories, setMemories] = useState<Memory[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [issued, setIssued] = useState<{ label: string; token: string } | null>(null);
  const [name, setName] = useState('');
  const [join, setJoin] = useState('');
  const [email, setEmail] = useState('');
  const [role, setRole] = useState<Role>('editor');
  const [agent, setAgent] = useState('');
  const [permission, setPermission] = useState('read');
  const [days, setDays] = useState(30);
  const [content, setContent] = useState('');
  const [scope, setScope] = useState('global');
  const [query, setQuery] = useState('');
  const [target, setTarget] = useState('');
  const team = teams.find(t => t.id === selected);
  const admin = team?.role === 'owner' || team?.role === 'admin';
  const version = useRef(0);
  const identity = account?.user.id;
  const base = `/api/v1/teams/${selected}`;
  const loadTeams = useCallback(async () => {
    const rows = await request<Team[]>('/api/v1/teams');
    setTeams(rows); setSelected(current => rows.some(t => t.id === current) ? current : rows[0]?.id ?? '');
  }, []);
  const load = useCallback(async () => {
    if (!selected) return;
    const epoch = ++version.current;
    const [m, mem, k, i, a] = await Promise.all([
      request<Member[]>(`${base}/members`), request<Memory[]>(`${base}/memories`),
      admin ? request<AgentKey[]>(`${base}/keys`) : Promise.resolve([]),
      admin ? request<Invite[]>(`${base}/invites`) : Promise.resolve([]),
      admin ? request<Audit[]>(`${base}/audit`) : Promise.resolve([]),
    ]);
    if (epoch !== version.current) return;
    setMembers(m); setMemories(mem); setKeys(k); setInvites(i); setAudit(a);
  }, [base, selected, admin]);
  useEffect(() => { void initialize(); }, [initialize]);
  useEffect(() => {
    ++version.current; setTeams([]); setSelected(''); setIssued(null); setMembers([]); setMemories([]); setKeys([]); setInvites([]); setAudit([]);
    if (identity) void loadTeams().catch(e => setMessage(e.message));
  }, [identity, loadTeams]);
  useEffect(() => {
    setIssued(null); setMembers([]); setMemories([]); setKeys([]); setInvites([]); setAudit([]); setTarget(''); setMessage('');
    void load().catch(e => setMessage(e.message));
    return () => { ++version.current; };
  }, [load]);
  async function act(operation: () => Promise<void>) {
    if (busy) return;
    setBusy(true); setMessage('');
    try { await operation(); } catch (e) { setMessage(e instanceof Error ? e.message : '操作失败'); }
    finally { setBusy(false); }
  }
  if (!initialized) return <section className="team-workspace"><p>正在恢复云账户…</p></section>;
  if (!account) return <section className="team-workspace"><h3>团队工作区</h3><p>先在侧栏「云账户」登录，即可创建团队或接受邀请。团队成员使用各自已验证的邮箱登录。</p></section>;
  const canCreate = account.entitlements?.features.some(feature => feature.code === 'team_create' && feature.enabled) ?? false;
  return <section className="team-workspace" aria-label="团队工作区">
    <header><div><h3>团队工作区</h3><p>成员协作、独立 Agent 凭证与共享记忆</p></div><span>{account.user.email}</span></header>
    {message && <p className="team-notice" role="status">{message}</p>}
    {issued && <div className="team-secret" role="status"><strong>{issued.label} · 仅本次显示</strong><p>请保存后交给对应成员或 Agent。关闭后无法再次查看。</p><code>{issued.token}</code><button onClick={() => setIssued(null)}>已保存，关闭</button></div>}
    <fieldset disabled={busy} className="team-setup">
      <form onSubmit={e => { e.preventDefault(); if (!canCreate) return; void act(async () => { const t = await request<Team>('/api/v1/teams', { name }); await loadTeams(); setSelected(t.id); setName(''); setMessage('团队已创建'); }); }}>
        <label>新团队名称<input required maxLength={100} value={name} onChange={e => setName(e.target.value)} placeholder="例如：研发知识团队" /></label><button disabled={!canCreate}>创建团队</button>
        {!canCreate && <p>{account.entitlements?.access_source === 'team_membership' ? '当前为受邀团队成员；创建新的团队需要单独开通团队版订阅。' : account.entitlements ? '创建团队需要有效的 Team 权益；免费账号可通过邀请加入。' : '版本权益尚未获取，请到云账户刷新账户信息。'}</p>}
      </form>
      <form onSubmit={e => { e.preventDefault(); void act(async () => { const t = await request<Team>('/api/v1/teams/join', { token: join.trim() }); await loadTeams(); setSelected(t.id); setJoin(''); setMessage('已加入团队'); }); }}>
        <label>邀请码<input required value={join} onChange={e => setJoin(e.target.value)} placeholder="使用受邀邮箱登录后输入" autoComplete="off" /></label><button>加入团队</button>
      </form>
    </fieldset>
    {teams.length > 0 ? <label className="team-switch">当前团队<select value={selected} disabled={busy} onChange={e => setSelected(e.target.value)}>{teams.map(t => <option key={t.id} value={t.id}>{t.name} · {roles[t.role]}</option>)}</select></label> : <p>还没有团队。创建一个工作区，或输入管理员提供的邀请码。</p>}
    {team && <fieldset disabled={busy} className="team-content">
      <div className="team-grid">
        <article><h4>成员与角色 <small>{members.length}</small></h4><p>所有者管理角色；管理员管理普通成员和 Agent；编辑者写入记忆；只读成员检索记忆。</p>
          {members.map(m => <div className="team-row" key={m.cloud_user_id}><div><strong>{m.email}</strong><small>{roles[m.role]}</small></div>
            {team.role === 'owner' && m.role !== 'owner' && <select aria-label={`调整 ${m.email} 的角色`} value={m.role} onChange={e => void act(async () => { await request(`${base}/members/${m.cloud_user_id}`, { role: e.target.value }, 'PATCH'); await load(); })}>{(['admin','editor','viewer'] as Role[]).map(r => <option key={r} value={r}>{roles[r]}</option>)}</select>}
            {admin && m.role !== 'owner' && (team.role === 'owner' || m.role !== 'admin') && <button onClick={() => void act(async () => { await request(`${base}/members/${m.cloud_user_id}`, undefined, 'DELETE'); await load(); setMessage('成员已移除，后续请求将被拒绝'); })}>移除</button>}
          </div>)}
          {admin && <form onSubmit={e => { e.preventDefault(); void act(async () => { const s = await request<{token:string}>(`${base}/invites`, { email, role }); setIssued({label:`邀请 ${email}（7 天内有效）`,token:s.token}); setEmail(''); await load(); }); }}><label>受邀邮箱<input type="email" required value={email} onChange={e => setEmail(e.target.value)} /></label><label>邀请角色<select value={role} onChange={e => setRole(e.target.value as Role)}>{(['editor','viewer', ...(team.role === 'owner' ? ['admin'] : [])] as Role[]).map(r => <option key={r} value={r}>{roles[r]}</option>)}</select></label><button>生成邀请码</button></form>}
          {admin && invites.map(i => <div className="team-row" key={i.id}><span>{i.email}<small>{roles[i.role]} · {i.used_at ? '已接受' : i.revoked_at ? '已撤销' : Date.parse(i.expires_at) < Date.now() ? '已过期' : '待接受'}</small></span>{!i.used_at && !i.revoked_at && <button onClick={() => void act(async () => { await request(`${base}/invites/${i.id}`, undefined, 'DELETE'); await load(); })}>撤销邀请</button>}</div>)}
        </article>
        {admin && <article><h4>Agent 接入凭证</h4><p>凭证固定绑定本团队和 Agent 身份。相同 Agent 可签发多个凭证以轮换，撤销旧凭证后立即拒绝新请求。</p>
          <form onSubmit={e => { e.preventDefault(); void act(async () => { const s = await request<{token:string}>(`${base}/keys`, { agent_key: agent, name: agent, permission, expires_in_days: days }); setIssued({label:`Agent ${agent} 凭证`,token:s.token}); setAgent(''); await load(); }); }}>
            <label>Agent 标识<input required pattern="[A-Za-z0-9_\-]{1,64}" maxLength={64} value={agent} onChange={e => setAgent(e.target.value)} placeholder="research-agent" /></label>
            <label>权限<select value={permission} onChange={e => setPermission(e.target.value)}><option value="read">只读</option><option value="write">读写及定向授权</option></select></label>
            <label>有效天数<input type="number" min={1} max={365} required value={days} onChange={e => setDays(Number(e.target.value))} /></label><button>签发凭证</button>
          </form>
          {keys.map(k => <div className="team-row" key={k.id}><div><strong>{k.agent_key}</strong><small>{k.permission === 'read' ? '只读' : '读写'} · {k.revoked_at ? '已撤销' : Date.parse(k.expires_at) < Date.now() ? '已过期' : `有效至 ${new Date(k.expires_at).toLocaleDateString()}`}</small></div>{!k.revoked_at && <button onClick={() => void act(async () => { await request(`${base}/keys/${k.id}`, undefined, 'DELETE'); await load(); setIssued(null); })}>撤销凭证</button>}</div>)}
          <details><summary>API 接入示例</summary><p>将下面路径拼接到团队服务器地址，Bearer 使用上方签发的 Agent 凭证。团队成员必须连接同一服务器。</p><pre>{`POST ${base}/search\nAuthorization: Bearer <Agent 凭证>\nContent-Type: application/json\n\n{"query":"项目约定","mode":"adaptive","top_k":5}`}</pre><p>写入使用 /memories；memory_scope 为 private、shared 或 global（仅本团队公共）。agent_id 由服务端根据凭证确定。</p></details>
        </article>}
      </div>
      <article><h4>团队记忆</h4><p>公共记忆仅在当前团队可见；私有记忆仅归属身份可见。管理员也不能冒用其他 Agent 读取私有内容。</p>
        {team.role !== 'viewer' && <form onSubmit={e => { e.preventDefault(); void act(async () => { await request(`${base}/memories`, { content, memory_layer:'episode', memory_scope:scope }); setContent(''); await load(); setMessage('记忆已写入团队空间'); }); }}>
          <label>记忆内容<textarea required maxLength={10000} value={content} onChange={e => setContent(e.target.value)} /></label><label>可见范围<select value={scope} onChange={e => setScope(e.target.value)}><option value="global">团队公共</option><option value="private">仅自己</option><option value="shared">定向共享（写入后授权）</option></select></label><button>写入记忆</button>
        </form>}
        <form onSubmit={e => { e.preventDefault(); void act(async () => { const result = await request<{results:Memory[]}>(`${base}/search`, {query,mode:'adaptive',top_k:20}); setMemories(result.results); setMessage(`检索到 ${result.results.length} 条记忆`); }); }}><label>检索团队记忆<input required value={query} onChange={e => setQuery(e.target.value)} /></label><button>检索</button><button type="button" onClick={() => void act(load)}>刷新列表</button></form>
        {team.role !== 'viewer' && <label>定向授权目标<select value={target} onChange={e => setTarget(e.target.value)}><option value="">选择成员或输入 Agent 标识</option>{members.filter(m => m.cloud_user_id !== identity).map(m => <option key={m.cloud_user_id} value={`member-${m.cloud_user_id}`}>{m.email}</option>)}</select><input aria-label="授权 Agent 标识" value={target} onChange={e => setTarget(e.target.value)} placeholder="research-agent" /></label>}
        {memories.length === 0 && <p>暂无可见记忆。写入团队公共内容后，其他成员即可检索。</p>}
        {memories.map(m => <div className="team-memory" key={m.id}><small>{m.memory_scope === 'global' ? '团队公共' : m.memory_scope === 'private' ? '私有' : '定向共享'} · {m.memory_layer}</small><p>{m.content}</p>{team.role !== 'viewer' && m.owner_agent_id === `member-${identity}` && <div className="team-actions"><button disabled={!target} onClick={() => void act(async () => { await request(`${base}/memories/${m.id}/grants`, {owner_agent_id:'ignored',grantee_agent_id:target,can_read:true}); await load(); setMessage('已改为定向共享，并授予目标读取权限'); })}>授权给目标</button><button disabled={!target} onClick={() => void act(async () => { await request(`${base}/memories/${m.id}/grants`, {owner_agent_id:'ignored',grantee_agent_id:target,can_read:false}); await load(); setMessage('已撤销目标读取授权'); })}>撤销目标授权</button></div>}</div>)}
      </article>
      {admin && <article><h4>团队审计 · 最近 100 条</h4><p>记录成员、邀请、凭证和记忆访问操作。requested 表示请求已进入处理，不代表操作成功。</p>{audit.map(a => <div className="team-audit" key={a.id}><time>{new Date(a.created_at).toLocaleString()}</time><strong>{a.action}</strong><code>{a.target || '—'}</code><small>操作身份：{members.find(m => m.cloud_user_id === a.actor)?.email ?? a.actor}</small></div>)}</article>}
    </fieldset>}
  </section>;
}
