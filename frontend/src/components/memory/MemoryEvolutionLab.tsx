import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { apiFetch } from '../../lib/api';
import './MemoryEvolutionLab.css';
import { TeamWorkspace } from './TeamWorkspace';

interface Agent {
  agent_key: string;
  display_name: string;
  role_description: string;
}

interface Stats {
  total: number;
  episodes: number;
  facts: number;
  experiences: number;
  private_memories: number;
  shared_memories: number;
  global_memories: number;
  unresolved_conflicts: number;
  stale_or_filtered: number;
  retrieval_runs: number;
}

interface ScoreBreakdown {
  semantic: number;
  lexical: number;
  recency: number;
  trust: number;
  frequency: number;
  importance: number;
  total: number;
}

interface RankedMemory {
  id: string;
  content: string;
  memory_layer: string;
  owner_agent_id: string;
  memory_scope: string;
  scene_tags: string[];
  source_trust: number;
  conflict_status: string;
  retention_status: string;
  rank: number;
  score: number;
  score_breakdown: ScoreBreakdown;
}

interface SearchRun {
  run_id: string;
  mode: 'static' | 'adaptive';
  requested_k: number;
  candidate_count: number;
  filtered_count: number;
  latency_ms: number;
  estimated_tokens: number;
  results: RankedMemory[];
}

interface ThresholdCheck {
  metric: string;
  target: string;
  measured: number;
  passed: boolean;
}

interface Benchmark {
  dataset: string;
  dataset_description: string;
  turn_count: number;
  query_count: number;
  baseline: {
    top1_accuracy: number;
    top_k_precision: number;
    stale_redundant_filter_rate: number;
    retrieval_task_completion: number;
    estimated_token_count: number;
  };
  adaptive: {
    top1_accuracy: number;
    top_k_precision: number;
    stale_redundant_filter_rate: number;
    retrieval_task_completion: number;
    estimated_token_count: number;
  };
  accuracy_gain_percentage_points: number;
  task_completion_gain_percentage_points: number;
  token_reduction_rate: number;
  ablations: { name: string; top1_accuracy: number; top_k_precision: number }[];
  threshold_checks: ThresholdCheck[];
  evidence_note: string;
}

interface Conflict {
  id: string;
  fact_key: string;
  incumbent_memory_id: string;
  challenger_memory_id: string;
  resolution: string;
  reason: string;
  score_delta: number;
  incumbent_value: string | null;
  challenger_value: string | null;
  review_status: 'not_required' | 'pending' | 'resolved';
  reviewer_agent_id: string | null;
  review_note: string | null;
  reviewed_at: string | null;
  experience_memory_id: string | null;
  created_at: string;
}

interface MemoryDraft {
  memory_layer: string;
  memory_scope: string;
  content: string;
  fact_key: string;
  fact_value: string;
  scene_tags: string;
  source_trust: number;
}

type LabView = 'console' | 'governance' | 'evidence' | 'team';
type WeightKey = keyof Omit<ScoreBreakdown, 'total'>;

const INITIAL_STATS: Stats = {
  total: 0,
  episodes: 0,
  facts: 0,
  experiences: 0,
  private_memories: 0,
  shared_memories: 0,
  global_memories: 0,
  unresolved_conflicts: 0,
  stale_or_filtered: 0,
  retrieval_runs: 0,
};

const INITIAL_WEIGHTS: Record<WeightKey, number> = {
  semantic: 34,
  lexical: 18,
  recency: 16,
  trust: 13,
  frequency: 8,
  importance: 11,
};
export const RETRIEVAL_WEIGHTS_KEY = 'lingshu_pro_retrieval_weights';

function loadRetrievalWeights(): Record<WeightKey, number> {
  try {
    const stored = JSON.parse(window.localStorage.getItem(RETRIEVAL_WEIGHTS_KEY) || 'null');
    const value = stored?.value;
    if (value && Object.keys(INITIAL_WEIGHTS).every(key => Number.isFinite(value[key]))) return value;
  } catch { /* keep safe defaults */ }
  return INITIAL_WEIGHTS;
}

const INITIAL_MEMORY: MemoryDraft = {
  memory_layer: 'fact',
  memory_scope: 'private',
  content: '',
  fact_key: '',
  fact_value: '',
  scene_tags: 'project',
  source_trust: 90,
};

const LAYER_LABEL: Record<string, string> = {
  episode: '原始交互',
  fact: '结构化事实',
  experience: '通用经验',
};

const SCOPE_LABEL: Record<string, string> = {
  private: '私有',
  shared: '定向共享',
  global: '全局公共',
};

const WEIGHT_LABEL: Record<WeightKey, string> = {
  semantic: '语义',
  lexical: '词法',
  recency: '时序',
  trust: '可信度',
  frequency: '频次',
  importance: '重要性',
};

const LAB_VIEWS: { key: LabView; label: string; description: string }[] = [
  { key: 'team', label: '团队工作区', description: '成员、凭证与共享记忆' },
  { key: 'console', label: 'Memory Console', description: '召回、过滤与解释' },
  { key: 'governance', label: 'Agent Governance', description: '权限、冲突与生命周期' },
  { key: 'evidence', label: 'Benchmark & Integration', description: '实验、消融与接入' },
];

const API_ENDPOINTS = [
  ['POST', '/api/v1/memory-gateway/memories', '写入分层记忆'],
  ['POST', '/api/v1/memory-gateway/search', '静态或自适应检索'],
  ['POST', '/api/v1/memory-gateway/memories/{id}/grants', '定向授权'],
  ['POST', '/api/v1/memory-gateway/conflicts/{id}/resolve', '裁决争议并沉淀经验'],
  ['POST', '/api/v1/memory-gateway/maintenance', '执行生命周期维护'],
  ['GET', '/api/v1/memory-gateway/benchmark', '重算基线与消融'],
] as const;

const PYTHON_EXAMPLE = `import requests

result = requests.post(
    "http://127.0.0.1:8080/api/v1/memory-gateway/search",
    headers={"Authorization": "Bearer <token>"},
    json={
        "query": "项目现在主要使用什么技术栈？",
        "agent_id": "lingshu",
        "mode": "adaptive",
        "top_k": 5
    }
).json()`;

async function readJson<T>(path: string, init?: Parameters<typeof apiFetch>[1]): Promise<T> {
  const response = await apiFetch(path, init);
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new Error(data?.error?.message ?? `HTTP ${response.status}`);
  return data as T;
}

export function MemoryEvolutionLab() {
  const [activeView, setActiveView] = useState<LabView>('console');
  const [agents, setAgents] = useState<Agent[]>([]);
  const [agentId, setAgentId] = useState('lingshu');
  const [stats, setStats] = useState<Stats>(INITIAL_STATS);
  const [conflicts, setConflicts] = useState<Conflict[]>([]);
  const [query, setQuery] = useState('项目现在主要使用什么技术栈');
  const [topK, setTopK] = useState(5);
  const [staticRun, setStaticRun] = useState<SearchRun | null>(null);
  const [adaptiveRun, setAdaptiveRun] = useState<SearchRun | null>(null);
  const [selectedResultId, setSelectedResultId] = useState<string | null>(null);
  const [benchmark, setBenchmark] = useState<Benchmark | null>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [newAgent, setNewAgent] = useState({ key: '', name: '' });
  const [shareTarget, setShareTarget] = useState('');
  const [lastCreatedId, setLastCreatedId] = useState<string | null>(null);
  const [lastCreatedOwner, setLastCreatedOwner] = useState('lingshu');
  const [weights, setWeights] = useState<Record<WeightKey, number>>(loadRetrievalWeights);
  const didInitializeWeights = useRef(false);
  const [newMemory, setNewMemory] = useState<MemoryDraft>(INITIAL_MEMORY);

  const loadOverview = useCallback(async () => {
    const [agentRows, statRows, conflictRows] = await Promise.all([
      readJson<Agent[]>('/api/v1/memory-gateway/agents'),
      readJson<Stats>('/api/v1/memory-gateway/stats'),
      readJson<Conflict[]>('/api/v1/memory-gateway/conflicts'),
    ]);
    setAgents(agentRows);
    setStats(statRows);
    setConflicts(conflictRows);
  }, []);

  useEffect(() => {
    loadOverview().catch((error) =>
      setMessage(error instanceof Error ? error.message : '加载记忆引擎失败'),
    );
  }, [loadOverview]);

  const weightTotal = useMemo(
    () => Object.values(weights).reduce((sum, value) => sum + value, 0),
    [weights],
  );

  useEffect(() => {
    if (!didInitializeWeights.current) {
      didInitializeWeights.current = true;
      return;
    }
    window.localStorage.setItem(RETRIEVAL_WEIGHTS_KEY, JSON.stringify({ value: weights, updated_at: new Date().toISOString() }));
  }, [weights]);

  const selectedMemory = useMemo(() => {
    if (!adaptiveRun?.results.length) return null;
    return (
      adaptiveRun.results.find((result) => result.id === selectedResultId) ??
      adaptiveRun.results[0]
    );
  }, [adaptiveRun, selectedResultId]);

  const runComparison = useCallback(
    async (searchQuery: string) => {
      const normalized = Object.fromEntries(
        Object.entries(weights).map(([key, value]) => [key, value / Math.max(weightTotal, 1)]),
      );
      const payload = { query: searchQuery, agent_id: agentId, top_k: topK, weights: normalized };
      const [baseline, adaptive] = await Promise.all([
        readJson<SearchRun>('/api/v1/memory-gateway/search', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ ...payload, mode: 'static' }),
        }),
        readJson<SearchRun>('/api/v1/memory-gateway/search', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ ...payload, mode: 'adaptive' }),
        }),
      ]);
      setStaticRun(baseline);
      setAdaptiveRun(adaptive);
      setSelectedResultId(adaptive.results[0]?.id ?? null);
      setStats((current) => ({ ...current, retrieval_runs: current.retrieval_runs + 2 }));
    },
    [agentId, topK, weightTotal, weights],
  );

  const compareSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    setMessage(null);
    try {
      await runComparison(query.trim());
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '检索失败');
    } finally {
      setLoading(false);
    }
  };

  const postMemory = useCallback(
    (memory: MemoryDraft, ownerAgentId: string) =>
      readJson<{ id: string }>('/api/v1/memory-gateway/memories', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          agent_id: ownerAgentId,
          memory_layer: memory.memory_layer,
          memory_scope: memory.memory_scope,
          memory_type: memory.memory_layer === 'episode' ? 'context' : 'fact',
          content: memory.content,
          fact_key: memory.memory_layer === 'fact' ? memory.fact_key : null,
          fact_value: memory.memory_layer === 'fact' ? memory.fact_value : null,
          scene_tags: memory.scene_tags.split(',').map((tag) => tag.trim()).filter(Boolean),
          source_type: 'demo_console',
          source_trust: memory.source_trust / 100,
          importance: 0.8,
        }),
      }),
    [],
  );

  const createMemory = async () => {
    if (!newMemory.content.trim()) return;
    setLoading(true);
    setMessage(null);
    try {
      const created = await postMemory(newMemory, agentId);
      setLastCreatedId(created.id);
      setLastCreatedOwner(agentId);
      setNewMemory((current) => ({ ...current, content: '', fact_value: '' }));
      setMessage('记忆已写入；相同事实键的不同值会自动进入冲突治理。');
      await loadOverview();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '写入失败');
    } finally {
      setLoading(false);
    }
  };

  const loadConflictDemo = async () => {
    const demoQuery = '我现在住在哪里？';
    setLoading(true);
    setMessage(null);
    try {
      await postMemory({
        ...INITIAL_MEMORY,
        content: '用户此前居住在北京。',
        fact_key: 'user.location',
        fact_value: '北京',
        scene_tags: 'profile, travel',
        source_trust: 92,
      }, agentId);
      const latest = await postMemory({
        ...INITIAL_MEMORY,
        content: '用户已于八月搬到深圳，目前居住在深圳。',
        fact_key: 'user.location',
        fact_value: '深圳',
        scene_tags: 'profile, travel',
        source_trust: 98,
      }, agentId);
      setLastCreatedId(latest.id);
      setLastCreatedOwner(agentId);
      setQuery(demoQuery);
      await Promise.all([loadOverview(), runComparison(demoQuery)]);
      setMessage('演示已载入：旧事实“北京”与新事实“深圳”已进入双路检索。');
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '载入演示失败');
    } finally {
      setLoading(false);
    }
  };

  const createAgent = async () => {
    if (!newAgent.key.trim() || !newAgent.name.trim()) return;
    setLoading(true);
    setMessage(null);
    try {
      await readJson('/api/v1/memory-gateway/agents', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          agent_key: newAgent.key.trim(),
          display_name: newAgent.name.trim(),
          role_description: '演示协作 Agent',
        }),
      });
      setNewAgent({ key: '', name: '' });
      setMessage('Agent 已注册，可切换身份验证私有记忆隔离。');
      await loadOverview();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Agent 创建失败');
    } finally {
      setLoading(false);
    }
  };

  const shareLastMemory = async () => {
    if (!lastCreatedId || !shareTarget) return;
    setLoading(true);
    setMessage(null);
    try {
      await readJson(`/api/v1/memory-gateway/memories/${lastCreatedId}/grants`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          owner_agent_id: lastCreatedOwner,
          grantee_agent_id: shareTarget,
          can_read: true,
        }),
      });
      setMessage(`最近写入的记忆已定向授权给 ${shareTarget}。`);
      await loadOverview();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '共享授权失败');
    } finally {
      setLoading(false);
    }
  };

  const runBenchmark = async () => {
    setLoading(true);
    setMessage(null);
    try {
      setBenchmark(await readJson<Benchmark>('/api/v1/memory-gateway/benchmark'));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '评测失败');
    } finally {
      setLoading(false);
    }
  };

  const runMaintenance = async () => {
    setLoading(true);
    setMessage(null);
    try {
      const result = await readJson<{ stale_marked: number; conflicts_filtered: number }>(
        '/api/v1/memory-gateway/maintenance',
        { method: 'POST' },
      );
      setMessage(`生命周期维护完成：标记过期 ${result.stale_marked} 条，过滤被替代事实 ${result.conflicts_filtered} 条。`);
      await loadOverview();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '维护失败');
    } finally {
      setLoading(false);
    }
  };

  const resolveConflict = async (conflictId: string, winner: 'incumbent' | 'challenger') => {
    setLoading(true);
    setMessage(null);
    try {
      const result = await readJson<{ experience_memory_id: string | null }>(
        `/api/v1/memory-gateway/conflicts/${conflictId}/resolve`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            winner,
            reviewer_agent_id: agentId,
            promote_to_experience: true,
          }),
        },
      );
      setMessage(result.experience_memory_id
        ? '冲突已裁决，并沉淀为可追溯的经验记忆。'
        : '冲突已裁决。');
      await loadOverview();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '冲突裁决失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="evolution-lab">
      <header className="lab-header">
        <div className="lab-title-block">
          <h2>动态记忆控制台</h2>
          <p>看见每一次召回、过滤与事实更新</p>
        </div>
        <div className="lab-header-meta">
          <span className="engine-status"><i /> Memory Engine 在线</span>
          <span className="engine-version">API v1 · Local-first</span>
        </div>
      </header>

      <nav className="lab-view-nav" aria-label="动态记忆工作区">
        {LAB_VIEWS.map((view) => (
          <button key={view.key} type="button" className={activeView === view.key ? 'is-active' : ''} aria-current={activeView === view.key ? 'page' : undefined} onClick={() => setActiveView(view.key)}>
            <strong>{view.label}</strong>
            <span>{view.description}</span>
          </button>
        ))}
      </nav>

      {activeView === 'team' ? <TeamWorkspace /> : null}

      {message ? <div className="lab-message" role="status">{message}</div> : null}

      {activeView === 'console' ? (
        <MemoryConsole
          agents={agents}
          agentId={agentId}
          setAgentId={setAgentId}
          stats={stats}
          query={query}
          setQuery={setQuery}
          topK={topK}
          setTopK={setTopK}
          loading={loading}
          staticRun={staticRun}
          adaptiveRun={adaptiveRun}
          selectedResultId={selectedMemory?.id ?? null}
          selectedMemory={selectedMemory}
          weights={weights}
          setWeights={setWeights}
          conflicts={conflicts}
          onCompare={compareSearch}
          onLoadDemo={loadConflictDemo}
          onSelectResult={setSelectedResultId}
        />
      ) : null}

      {activeView === 'governance' ? (
        <GovernanceView
          agents={agents}
          agentId={agentId}
          setAgentId={setAgentId}
          stats={stats}
          conflicts={conflicts}
          loading={loading}
          newAgent={newAgent}
          setNewAgent={setNewAgent}
          newMemory={newMemory}
          setNewMemory={setNewMemory}
          shareTarget={shareTarget}
          setShareTarget={setShareTarget}
          lastCreatedId={lastCreatedId}
          onCreateAgent={createAgent}
          onCreateMemory={createMemory}
          onShare={shareLastMemory}
          onMaintenance={runMaintenance}
          onResolveConflict={resolveConflict}
        />
      ) : null}

      {activeView === 'evidence' ? (
        <EvidenceView benchmark={benchmark} loading={loading} onRunBenchmark={runBenchmark} />
      ) : null}
    </div>
  );
}

interface MemoryConsoleProps {
  agents: Agent[];
  agentId: string;
  setAgentId: (value: string) => void;
  stats: Stats;
  query: string;
  setQuery: (value: string) => void;
  topK: number;
  setTopK: (value: number) => void;
  loading: boolean;
  staticRun: SearchRun | null;
  adaptiveRun: SearchRun | null;
  selectedResultId: string | null;
  selectedMemory: RankedMemory | null;
  weights: Record<WeightKey, number>;
  setWeights: Dispatch<SetStateAction<Record<WeightKey, number>>>;
  conflicts: Conflict[];
  onCompare: () => void;
  onLoadDemo: () => void;
  onSelectResult: (id: string) => void;
}

function MemoryConsole({ agents, agentId, setAgentId, stats, query, setQuery, topK, setTopK, loading, staticRun, adaptiveRun, selectedResultId, selectedMemory, weights, setWeights, conflicts, onCompare, onLoadDemo, onSelectResult }: MemoryConsoleProps) {
  return (
    <main className="lab-view console-view">
      <section className="command-surface" aria-label="检索命令">
        <div className="command-query">
          <label htmlFor="memory-query">本轮 Query</label>
          <input id="memory-query" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') onCompare(); }} placeholder="输入需要长期记忆回答的问题" />
        </div>
        <label className="command-select">访问 Agent<select value={agentId} onChange={(event) => setAgentId(event.target.value)}>{agents.map((agent) => <option key={agent.agent_key} value={agent.agent_key}>{agent.display_name}</option>)}</select></label>
        <label className="command-select command-top-k">Top-K<input type="number" min="1" max="20" value={topK} onChange={(event) => setTopK(Number(event.target.value))} /></label>
        <button type="button" className="lab-button lab-button-quiet" onClick={onLoadDemo} disabled={loading}>载入冲突演示</button>
        <button type="button" className="lab-button lab-button-primary" onClick={onCompare} disabled={loading}>{loading ? '正在检索…' : '运行双路检索'}</button>
      </section>

      <section className="console-stat-rail" aria-label="记忆引擎统计">
        <ConsoleStat label="候选记忆" value={adaptiveRun?.candidate_count ?? stats.total} />
        <ConsoleStat label="本轮过滤" value={adaptiveRun?.filtered_count ?? 0} tone="green" />
        <ConsoleStat label="事实冲突" value={stats.unresolved_conflicts} tone="amber" />
        <ConsoleStat label="检索证据" value={stats.retrieval_runs} tone="violet" />
      </section>

      <section className="comparison-workbench" aria-label="Static RAG 与 Dynamic Memory 对照">
        <RunColumn title="Static RAG" subtitle="相似度召回，不过滤失效事实" run={staticRun} tone="baseline" />
        <RunColumn title="Dynamic Memory" subtitle="权限校验、冲突过滤与六因素重排" run={adaptiveRun} tone="adaptive" selectedResultId={selectedResultId} onSelect={onSelectResult} />
        <ScoreInspector memory={selectedMemory} weights={weights} setWeights={setWeights} />
      </section>

      <section className="memory-trace-band">
        <div className="trace-heading"><div><h3>三层记忆链路</h3><p>从可追溯原始交互，到当前事实，再沉淀为可复用经验。</p></div><span>{stats.total} 条有效记录</span></div>
        <div className="memory-lineage">
          <TraceNode index="01" title="原始交互" count={stats.episodes} description="完整语境与时间线" tone="cyan" />
          <TraceNode index="02" title="结构化事实" count={stats.facts} description="事实键、有效期与来源" tone="violet" />
          <TraceNode index="03" title="通用经验" count={stats.experiences} description="跨任务复用的成功模式" tone="amber" />
        </div>
        <ConflictSnapshot conflict={conflicts[0] ?? null} filteredCount={stats.stale_or_filtered} />
      </section>
    </main>
  );
}

interface GovernanceViewProps {
  agents: Agent[];
  agentId: string;
  setAgentId: (value: string) => void;
  stats: Stats;
  conflicts: Conflict[];
  loading: boolean;
  newAgent: { key: string; name: string };
  setNewAgent: Dispatch<SetStateAction<{ key: string; name: string }>>;
  newMemory: MemoryDraft;
  setNewMemory: Dispatch<SetStateAction<MemoryDraft>>;
  shareTarget: string;
  setShareTarget: (value: string) => void;
  lastCreatedId: string | null;
  onCreateAgent: () => void;
  onCreateMemory: () => void;
  onShare: () => void;
  onMaintenance: () => void;
  onResolveConflict: (conflictId: string, winner: 'incumbent' | 'challenger') => void;
}

function GovernanceView({ agents, agentId, setAgentId, stats, conflicts, loading, newAgent, setNewAgent, newMemory, setNewMemory, shareTarget, setShareTarget, lastCreatedId, onCreateAgent, onCreateMemory, onShare, onMaintenance, onResolveConflict }: GovernanceViewProps) {
  const confirmResolution = (item: Conflict, winner: 'incumbent' | 'challenger') => {
    const selectedValue = winner === 'incumbent' ? item.incumbent_value : item.challenger_value;
    const accepted = window.confirm(
      `确认将“${selectedValue ?? '未记录值'}”设为 ${item.fact_key} 的有效事实？另一候选将被过滤，并生成可追溯经验。`,
    );
    if (accepted) onResolveConflict(item.id, winner);
  };

  return (
    <main className="lab-view governance-view">
      <section className="governance-overview lab-surface">
        <div className="section-heading"><div><h3>Agent 与记忆空间</h3><p>向量召回之前先执行身份、空间和定向授权校验。</p></div><span>{agents.length} 个 Agent</span></div>
        <div className="agent-space-grid">
          <div className="agent-list">
            {agents.map((agent) => (
              <button type="button" key={agent.agent_key} className={agentId === agent.agent_key ? 'is-active' : ''} onClick={() => setAgentId(agent.agent_key)}>
                <span>{agent.display_name.slice(0, 1).toUpperCase()}</span>
                <div><strong>{agent.display_name}</strong><small>{agent.agent_key}</small></div>
                <em>{agentId === agent.agent_key ? '当前身份' : '可切换'}</em>
              </button>
            ))}
          </div>
          <div className="scope-map">
            <ScopeMetric label="Private" value={stats.private_memories} description="仅所有者可见" tone="cyan" />
            <ScopeMetric label="Shared" value={stats.shared_memories} description="按记忆定向授权" tone="violet" />
            <ScopeMetric label="Global" value={stats.global_memories} description="所有 Agent 可检索" tone="green" />
          </div>
        </div>
      </section>

      <section className="governance-forms">
        <div className="lab-surface memory-writer">
          <div className="section-heading"><div><h3>写入分层记忆</h3><p>使用相同事实键写入新值，可现场触发时序冲突。</p></div></div>
          <div className="field-grid">
            <label>记忆层级<select value={newMemory.memory_layer} onChange={(event) => setNewMemory((current) => ({ ...current, memory_layer: event.target.value }))}><option value="episode">原始交互</option><option value="fact">结构化事实</option><option value="experience">通用经验</option></select></label>
            <label>权限空间<select value={newMemory.memory_scope} onChange={(event) => setNewMemory((current) => ({ ...current, memory_scope: event.target.value }))}><option value="private">私有</option><option value="shared">定向共享</option><option value="global">全局公共</option></select></label>
            {newMemory.memory_layer === 'fact' ? <><label>事实键<input value={newMemory.fact_key} placeholder="user.location" onChange={(event) => setNewMemory((current) => ({ ...current, fact_key: event.target.value }))} /></label><label>事实值<input value={newMemory.fact_value} placeholder="深圳" onChange={(event) => setNewMemory((current) => ({ ...current, fact_value: event.target.value }))} /></label></> : null}
            <label className="field-wide">场景标签<input value={newMemory.scene_tags} placeholder="profile, travel" onChange={(event) => setNewMemory((current) => ({ ...current, scene_tags: event.target.value }))} /></label>
            <label className="field-wide">记忆内容<textarea value={newMemory.content} placeholder="输入原始交互、事实或经验内容" onChange={(event) => setNewMemory((current) => ({ ...current, content: event.target.value }))} /></label>
            <label className="field-wide range-field"><span>来源可信度 <b>{newMemory.source_trust}%</b></span><input type="range" min="0" max="100" value={newMemory.source_trust} onChange={(event) => setNewMemory((current) => ({ ...current, source_trust: Number(event.target.value) }))} /></label>
          </div>
          <button type="button" className="lab-button lab-button-primary" onClick={onCreateMemory} disabled={loading || !newMemory.content.trim()}>写入记忆</button>
        </div>

        <div className="lab-surface agent-onboarding">
          <div className="section-heading"><div><h3>注册协作 Agent</h3><p>建立独立身份与私有记忆边界。</p></div></div>
          <label>Agent Key<input value={newAgent.key} placeholder="travel_agent" onChange={(event) => setNewAgent((current) => ({ ...current, key: event.target.value }))} /></label>
          <label>显示名称<input value={newAgent.name} placeholder="旅行 Agent" onChange={(event) => setNewAgent((current) => ({ ...current, name: event.target.value }))} /></label>
          <button type="button" className="lab-button lab-button-quiet" onClick={onCreateAgent} disabled={loading || !newAgent.key.trim() || !newAgent.name.trim()}>注册 Agent</button>
          <div className="share-divider" />
          <div className="section-heading"><div><h3>定向授权</h3><p>将最近写入的记忆授权给指定 Agent。</p></div></div>
          <code className="memory-id">{lastCreatedId ?? '尚未写入记忆'}</code>
          <select value={shareTarget} onChange={(event) => setShareTarget(event.target.value)}><option value="">选择接收 Agent</option>{agents.filter((agent) => agent.agent_key !== agentId).map((agent) => <option key={agent.agent_key} value={agent.agent_key}>{agent.display_name}</option>)}</select>
          <button type="button" className="lab-button lab-button-quiet" onClick={onShare} disabled={loading || !lastCreatedId || !shareTarget}>授权读取</button>
        </div>
      </section>

      <section className="lab-surface conflict-ledger">
        <div className="section-heading"><div><h3>冲突与生命周期账本</h3><p>旧事实不直接删除，而是保留裁决理由和历史状态。</p></div><button type="button" className="lab-button lab-button-quiet" onClick={onMaintenance} disabled={loading}>执行生命周期维护</button></div>
        {conflicts.length ? (
          <div className="conflict-table">
            <div className="conflict-table-head"><span>事实键</span><span>候选值</span><span>状态</span><span>证据</span><span>操作</span></div>
            {conflicts.slice(0, 8).map((item) => (
              <div className="conflict-table-row" key={item.id}>
                <strong>{item.fact_key}</strong>
                <div className="conflict-values">
                  <span title={item.incumbent_value ?? ''}>原：{item.incumbent_value ?? '未记录'}</span>
                  <span title={item.challenger_value ?? ''}>新：{item.challenger_value ?? '未记录'}</span>
                </div>
                <span>{item.review_status === 'pending' ? '待人工裁决' : item.resolution}</span>
                <p>{item.reason}</p>
                <div className="conflict-actions">
                  {item.review_status === 'pending' ? <>
                    <button type="button" className="lab-button lab-button-quiet" disabled={loading} onClick={() => confirmResolution(item, 'incumbent')}>保留原值</button>
                    <button type="button" className="lab-button lab-button-primary" disabled={loading} onClick={() => confirmResolution(item, 'challenger')}>采用新值</button>
                  </> : <time>{new Date(item.reviewed_at ?? item.created_at).toLocaleString('zh-CN')}</time>}
                </div>
              </div>
            ))}
          </div>
        ) : <EmptyState title="暂无冲突记录" description="在上方使用同一事实键写入不同值，即可生成可审计的冲突裁决。" />}
      </section>
    </main>
  );
}

function EvidenceView({ benchmark, loading, onRunBenchmark }: { benchmark: Benchmark | null; loading: boolean; onRunBenchmark: () => void }) {
  return (
    <main className="lab-view evidence-view">
      <section className="benchmark-header lab-surface"><div><h3>让指标从后端现场重算</h3><p>同一数据、同一候选池，对照静态基线、完整模型与消融条件。</p></div><button type="button" className="lab-button lab-button-primary" onClick={onRunBenchmark} disabled={loading}>{loading ? '正在计算…' : '重跑标准评测'}</button></section>
      {benchmark ? <>
        <section className="benchmark-summary lab-surface">
          <div className="benchmark-dataset"><span>当前数据集</span><strong>{benchmark.dataset}</strong><p>{benchmark.dataset_description}</p><small>{benchmark.turn_count} 轮记忆 · {benchmark.query_count} 个时序查询</small></div>
          <div className="metric-stack"><MetricBar label="长时序 Top-1" baseline={benchmark.baseline.top1_accuracy} adaptive={benchmark.adaptive.top1_accuracy} suffix="%" /><MetricBar label="Top-K（当前口径 Hit@5）" baseline={benchmark.baseline.top_k_precision} adaptive={benchmark.adaptive.top_k_precision} suffix="%" /><MetricBar label="估算上下文 Token" baseline={benchmark.baseline.estimated_token_count} adaptive={benchmark.adaptive.estimated_token_count} suffix="" lowerBetter /></div>
        </section>
        <section className="threshold-strip">{benchmark.threshold_checks.map((check) => <ThresholdCard key={check.metric} check={check} />)}</section>
        <section className="ablation-surface lab-surface">
          <div className="section-heading"><div><h3>消融实验</h3><p>移除单一能力后，观察准确率与召回覆盖变化。</p></div></div>
          <div className="ablation-table"><div className="ablation-head"><span>实验条件</span><span>Top-1</span><span>Hit@5</span></div>{benchmark.ablations.map((row) => <div className="ablation-row" key={row.name}><strong>{row.name}</strong><span>{row.top1_accuracy.toFixed(2)}%</span><span>{row.top_k_precision.toFixed(2)}%</span></div>)}</div>
          <p className="evidence-boundary"><strong>证据边界</strong>{benchmark.evidence_note}</p>
        </section>
      </> : <section className="benchmark-empty-state lab-surface"><EmptyState title="评测结果尚未载入" description="点击“重跑标准评测”，后端将读取标准化数据集并返回基线、完整模型、消融和阈值证据。" /></section>}

      <section className="integration-grid">
        <div className="lab-surface api-contract"><div className="section-heading"><div><h3>REST API / OpenAPI</h3><p>核心 Memory Engine 与展示界面解耦，可由第三方 Agent 调用。</p></div><span>v1</span></div><div className="endpoint-list">{API_ENDPOINTS.map(([method, path, description]) => <div key={path}><strong className={`method method-${method.toLowerCase()}`}>{method}</strong><code>{path}</code><span>{description}</span></div>)}</div></div>
        <div className="lab-surface sdk-preview"><div className="section-heading"><div><h3>接入示例</h3><p>当前提供 REST API；可基于 OpenAPI 继续封装 Python / TypeScript SDK。</p></div></div><pre><code>{PYTHON_EXAMPLE}</code></pre></div>
      </section>
    </main>
  );
}

function ConsoleStat({ label, value, tone = 'cyan' }: { label: string; value: number; tone?: string }) {
  return <div className={`console-stat tone-${tone}`}><span>{label}</span><strong>{value}</strong></div>;
}

function RunColumn({ title, subtitle, run, tone, selectedResultId, onSelect }: { title: string; subtitle: string; run: SearchRun | null; tone: 'baseline' | 'adaptive'; selectedResultId?: string | null; onSelect?: (id: string) => void }) {
  return (
    <div className={`run-column ${tone}`}>
      <header><div><strong>{title}</strong><span>{subtitle}</span></div>{run ? <b>{run.latency_ms.toFixed(1)} ms</b> : <span className="waiting-mark">WAITING</span>}</header>
      {run ? <><div className="run-summary"><span>候选 {run.candidate_count}</span><span>过滤 {run.filtered_count}</span><span>Token ≈ {run.estimated_tokens}</span></div><div className="result-list">{run.results.map((result) => <button type="button" key={result.id} className={selectedResultId === result.id ? 'is-selected' : ''} onClick={() => onSelect?.(result.id)}><span className="result-rank">{String(result.rank).padStart(2, '0')}</span><div className="result-copy"><strong>{result.content}</strong><span>{LAYER_LABEL[result.memory_layer] ?? result.memory_layer} · {SCOPE_LABEL[result.memory_scope] ?? result.memory_scope} · {result.owner_agent_id}</span><small>{result.conflict_status.toUpperCase()} · {result.retention_status.toUpperCase()}</small></div><em>{result.score.toFixed(3)}</em></button>)}</div></> : <EmptyState title="等待双路检索" description={tone === 'baseline' ? '展示未经动态过滤的相似结果。' : '展示权限校验、状态过滤和六因素重排后的最终上下文。'} />}
    </div>
  );
}

function ScoreInspector({ memory, weights, setWeights }: { memory: RankedMemory | null; weights: Record<WeightKey, number>; setWeights: Dispatch<SetStateAction<Record<WeightKey, number>>> }) {
  const scoreRows = (Object.keys(WEIGHT_LABEL) as WeightKey[]).map((key) => ({ key, label: WEIGHT_LABEL[key], score: memory?.score_breakdown[key] ?? 0 }));
  return (
    <aside className="score-inspector">
      <header><div><strong>评分解释器</strong><span>六因素自适应重排</span></div>{memory ? <b>{memory.score.toFixed(3)}</b> : <span>—</span>}</header>
      <div className="score-bars">{scoreRows.map((row) => <div key={row.key} className="score-row"><span>{row.label}</span><i><em style={{ width: `${Math.min(Math.max(row.score, 0), 1) * 100}%` }} /></i><b>{row.score.toFixed(2)}</b></div>)}</div>
      <div className="inspector-divider" />
      <strong className="weight-title">检索权重</strong>
      <div className="weight-controls">{(Object.keys(WEIGHT_LABEL) as WeightKey[]).map((key) => <label key={key}><span>{WEIGHT_LABEL[key]} <b>{weights[key]}%</b></span><input type="range" min="0" max="60" value={weights[key]} onChange={(event) => setWeights((current) => ({ ...current, [key]: Number(event.target.value) }))} /></label>)}</div>
      {memory ? <div className="selected-memory-state"><span>{memory.conflict_status}</span><span>{memory.retention_status}</span></div> : null}
    </aside>
  );
}

function TraceNode({ index, title, count, description, tone }: { index: string; title: string; count: number; description: string; tone: string }) {
  return <div className={`trace-node tone-${tone}`}><span>{index}</span><div><strong>{title}</strong><small>{description}</small></div><b>{count}</b></div>;
}

function ConflictSnapshot({ conflict, filteredCount }: { conflict: Conflict | null; filteredCount: number }) {
  return <div className="conflict-snapshot"><div className="snapshot-label"><span>CONFLICT TRACE</span><strong>事实版本演进</strong></div>{conflict ? <div className="snapshot-flow"><div><small>事实键</small><strong>{conflict.fact_key}</strong></div><i /><div><small>治理决策</small><strong>{conflict.resolution}</strong></div><i /><div className="snapshot-final"><small>已过滤历史事实</small><strong>{filteredCount} 条</strong></div></div> : <p>尚无冲突记录。载入冲突演示即可观察旧事实如何转为历史版本。</p>}</div>;
}

function ScopeMetric({ label, value, description, tone }: { label: string; value: number; description: string; tone: string }) {
  return <div className={`scope-metric tone-${tone}`}><span>{label}</span><strong>{value}</strong><small>{description}</small></div>;
}

function MetricBar({ label, baseline, adaptive, suffix, lowerBetter = false }: { label: string; baseline: number; adaptive: number; suffix: string; lowerBetter?: boolean }) {
  const maximum = Math.max(baseline, adaptive, 1);
  return <div className="metric-row"><strong>{label}</strong><div><span>Static</span><i><em style={{ width: `${(baseline / maximum) * 100}%` }} /></i><b>{baseline.toFixed(suffix ? 2 : 0)}{suffix}</b></div><div className="adaptive-bar"><span>Dynamic</span><i><em style={{ width: `${(adaptive / maximum) * 100}%` }} /></i><b>{adaptive.toFixed(suffix ? 2 : 0)}{suffix}{lowerBetter ? ' ↓' : ''}</b></div></div>;
}

function ThresholdCard({ check }: { check: ThresholdCheck }) {
  return <div className={`threshold-card ${check.passed ? 'passed' : 'pending'}`}><span>{check.passed ? 'PASS' : 'PENDING'}</span><strong>{check.metric}</strong><b>{check.measured.toFixed(2)}%</b><small>目标 {check.target}</small></div>;
}

function EmptyState({ title, description }: { title: string; description: string }) {
  return <div className="lab-empty"><i /><strong>{title}</strong><p>{description}</p></div>;
}
