import { useCallback, useEffect, useMemo, useState } from 'react';
import { apiFetch } from '../../lib/api';
import './MemoryEvolutionLab.css';

interface Agent { agent_key: string; display_name: string; role_description: string }
interface Stats {
  total: number; episodes: number; facts: number; experiences: number;
  private_memories: number; shared_memories: number; global_memories: number;
  unresolved_conflicts: number; stale_or_filtered: number; retrieval_runs: number;
}
interface ScoreBreakdown { semantic: number; lexical: number; recency: number; trust: number; frequency: number; importance: number; total: number }
interface RankedMemory {
  id: string; content: string; memory_layer: string; owner_agent_id: string; memory_scope: string;
  scene_tags: string[]; source_trust: number; conflict_status: string; retention_status: string;
  rank: number; score: number; score_breakdown: ScoreBreakdown;
}
interface SearchRun {
  run_id: string; mode: 'static' | 'adaptive'; requested_k: number; candidate_count: number;
  filtered_count: number; latency_ms: number; estimated_tokens: number; results: RankedMemory[];
}
interface ThresholdCheck { metric: string; target: string; measured: number; passed: boolean }
interface Benchmark {
  dataset: string; dataset_description: string; turn_count: number; query_count: number;
  baseline: { top1_accuracy: number; top_k_precision: number; stale_redundant_filter_rate: number; retrieval_task_completion: number; estimated_token_count: number };
  adaptive: { top1_accuracy: number; top_k_precision: number; stale_redundant_filter_rate: number; retrieval_task_completion: number; estimated_token_count: number };
  accuracy_gain_percentage_points: number; task_completion_gain_percentage_points: number; token_reduction_rate: number;
  ablations: { name: string; top1_accuracy: number; top_k_precision: number }[];
  threshold_checks: ThresholdCheck[]; evidence_note: string;
}
interface Conflict {
  id: string; fact_key: string; resolution: string; reason: string; score_delta: number; created_at: string;
}

const INITIAL_STATS: Stats = {
  total: 0, episodes: 0, facts: 0, experiences: 0, private_memories: 0,
  shared_memories: 0, global_memories: 0, unresolved_conflicts: 0,
  stale_or_filtered: 0, retrieval_runs: 0,
};

const LAYER_LABEL: Record<string, string> = { episode: '原始交互片段', fact: '结构化事实', experience: '通用经验' };
const SCOPE_LABEL: Record<string, string> = { private: '私有', shared: '定向共享', global: '全局公共' };

async function readJson<T>(path: string, init?: Parameters<typeof apiFetch>[1]): Promise<T> {
  const response = await apiFetch(path, init);
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new Error(data?.error?.message ?? `HTTP ${response.status}`);
  return data as T;
}

export function MemoryEvolutionLab() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [agentId, setAgentId] = useState('lingshu');
  const [stats, setStats] = useState<Stats>(INITIAL_STATS);
  const [conflicts, setConflicts] = useState<Conflict[]>([]);
  const [query, setQuery] = useState('项目现在主要使用什么技术栈');
  const [topK, setTopK] = useState(5);
  const [staticRun, setStaticRun] = useState<SearchRun | null>(null);
  const [adaptiveRun, setAdaptiveRun] = useState<SearchRun | null>(null);
  const [benchmark, setBenchmark] = useState<Benchmark | null>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [newAgent, setNewAgent] = useState({ key: '', name: '' });
  const [shareTarget, setShareTarget] = useState('');
  const [lastCreatedId, setLastCreatedId] = useState<string | null>(null);
  const [lastCreatedOwner, setLastCreatedOwner] = useState('lingshu');
  const [weights, setWeights] = useState({ semantic: 34, lexical: 18, recency: 16, trust: 13, frequency: 8, importance: 11 });
  const [newMemory, setNewMemory] = useState({
    memory_layer: 'fact', memory_scope: 'private', content: '', fact_key: '', fact_value: '', scene_tags: 'project', source_trust: 90,
  });

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
    loadOverview().catch((error) => setMessage(error instanceof Error ? error.message : '加载实验台失败'));
  }, [loadOverview]);

  const weightTotal = useMemo(() => Object.values(weights).reduce((sum, value) => sum + value, 0), [weights]);

  const compareSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    setMessage(null);
    const normalized = Object.fromEntries(Object.entries(weights).map(([key, value]) => [key, value / Math.max(weightTotal, 1)]));
    const payload = { query, agent_id: agentId, top_k: topK, weights: normalized };
    try {
      const [baseline, adaptive] = await Promise.all([
        readJson<SearchRun>('/api/v1/memory-gateway/search', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ ...payload, mode: 'static' }) }),
        readJson<SearchRun>('/api/v1/memory-gateway/search', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ ...payload, mode: 'adaptive' }) }),
      ]);
      setStaticRun(baseline);
      setAdaptiveRun(adaptive);
      setStats((current) => ({ ...current, retrieval_runs: current.retrieval_runs + 2 }));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '检索失败');
    } finally { setLoading(false); }
  };

  const createMemory = async () => {
    if (!newMemory.content.trim()) return;
    setLoading(true);
    setMessage(null);
    try {
      const created = await readJson<{ id: string }>('/api/v1/memory-gateway/memories', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          agent_id: agentId,
          memory_layer: newMemory.memory_layer,
          memory_scope: newMemory.memory_scope,
          memory_type: newMemory.memory_layer === 'episode' ? 'context' : 'fact',
          content: newMemory.content,
          fact_key: newMemory.memory_layer === 'fact' ? newMemory.fact_key : null,
          fact_value: newMemory.memory_layer === 'fact' ? newMemory.fact_value : null,
          scene_tags: newMemory.scene_tags.split(',').map((tag) => tag.trim()).filter(Boolean),
          source_type: 'demo_console', source_trust: newMemory.source_trust / 100, importance: 0.8,
        }),
      });
      setLastCreatedId(created.id);
      setLastCreatedOwner(agentId);
      setNewMemory((current) => ({ ...current, content: '', fact_value: '' }));
      setMessage('记忆已写入；若 fact_key 与旧事实相同，冲突治理已自动执行。');
      await loadOverview();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : '写入失败');
    } finally { setLoading(false); }
  };

  const createAgent = async () => {
    if (!newAgent.key.trim() || !newAgent.name.trim()) return;
    setLoading(true); setMessage(null);
    try {
      await readJson('/api/v1/memory-gateway/agents', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ agent_key: newAgent.key.trim(), display_name: newAgent.name.trim(), role_description: '演示协作 Agent' }),
      });
      setNewAgent({ key: '', name: '' });
      setMessage('Agent 已注册，可切换身份验证私有记忆隔离。');
      await loadOverview();
    } catch (error) { setMessage(error instanceof Error ? error.message : 'Agent 创建失败'); }
    finally { setLoading(false); }
  };

  const shareLastMemory = async () => {
    if (!lastCreatedId || !shareTarget) return;
    setLoading(true); setMessage(null);
    try {
      await readJson(`/api/v1/memory-gateway/memories/${lastCreatedId}/grants`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ owner_agent_id: lastCreatedOwner, grantee_agent_id: shareTarget, can_read: true }),
      });
      setMessage(`最近写入的记忆已定向授权给 ${shareTarget}。`);
      await loadOverview();
    } catch (error) { setMessage(error instanceof Error ? error.message : '共享授权失败'); }
    finally { setLoading(false); }
  };

  const runBenchmark = async () => {
    setLoading(true); setMessage(null);
    try { setBenchmark(await readJson<Benchmark>('/api/v1/memory-gateway/benchmark')); }
    catch (error) { setMessage(error instanceof Error ? error.message : '评测失败'); }
    finally { setLoading(false); }
  };

  const runMaintenance = async () => {
    setLoading(true); setMessage(null);
    try {
      const result = await readJson<{ stale_marked: number; conflicts_filtered: number }>('/api/v1/memory-gateway/maintenance', { method: 'POST' });
      setMessage(`动态筛选完成：标记过期 ${result.stale_marked} 条，过滤被替代事实 ${result.conflicts_filtered} 条。`);
      await loadOverview();
    } catch (error) { setMessage(error instanceof Error ? error.message : '维护失败'); }
    finally { setLoading(false); }
  };

  return (
    <div className="evolution-lab">
      <header className="lab-hero">
        <div>
          <span className="lab-kicker">LINGSHU MEMORY EVOLUTION LAB</span>
          <h2>动态记忆沉淀与自适应检索系统</h2>
          <p>三层异构记忆 · 多 Agent 权限网关 · 时序冲突治理 · 静态 RAG 对照实验</p>
        </div>
        <div className="lab-status"><i /> 全链路原型在线</div>
      </header>

      {message && <div className="lab-message">{message}</div>}

      <section className="lab-stat-grid" aria-label="记忆系统统计">
        <Stat value={stats.episodes} label="原始片段" accent="cyan" />
        <Stat value={stats.facts} label="结构化事实" accent="violet" />
        <Stat value={stats.experiences} label="通用经验" accent="amber" />
        <Stat value={stats.unresolved_conflicts} label="待确认冲突" accent="red" />
        <Stat value={stats.stale_or_filtered} label="已动态过滤" accent="green" />
        <Stat value={stats.retrieval_runs} label="检索证据" accent="blue" />
      </section>

      <section className="lab-panel pipeline-panel">
        <div className="panel-heading"><div><span>01 / STORAGE</span><h3>分层异构情景记忆</h3></div><small>来源可追溯 · 时间有效区间 · 场景标签</small></div>
        <div className="memory-pipeline">
          {[
            ['episode', '原始交互片段层', '保留完整语境与时间线'],
            ['fact', '结构化事实层', 'fact_key / value 可冲突治理'],
            ['experience', '通用经验沉淀层', '由多条来源归纳并可复用'],
          ].map(([key, title, description], index) => (
            <div className={`pipeline-node layer-${key}`} key={key}><b>0{index + 1}</b><strong>{title}</strong><span>{description}</span>{index < 2 && <em>→</em>}</div>
          ))}
        </div>
        <div className="write-grid">
          <label>访问 Agent<select value={agentId} onChange={(event) => setAgentId(event.target.value)}>{agents.map((agent) => <option key={agent.agent_key} value={agent.agent_key}>{agent.display_name} / {agent.agent_key}</option>)}</select></label>
          <label>记忆层级<select value={newMemory.memory_layer} onChange={(event) => setNewMemory({ ...newMemory, memory_layer: event.target.value })}><option value="episode">原始片段</option><option value="fact">结构化事实</option><option value="experience">通用经验</option></select></label>
          <label>权限空间<select value={newMemory.memory_scope} onChange={(event) => setNewMemory({ ...newMemory, memory_scope: event.target.value })}><option value="private">私有</option><option value="shared">定向共享</option><option value="global">全局公共</option></select></label>
          <label>来源可信度 <b>{newMemory.source_trust}%</b><input type="range" min="0" max="100" value={newMemory.source_trust} onChange={(event) => setNewMemory({ ...newMemory, source_trust: Number(event.target.value) })} /></label>
          {newMemory.memory_layer === 'fact' && <><label>事实键<input value={newMemory.fact_key} placeholder="project.stack" onChange={(event) => setNewMemory({ ...newMemory, fact_key: event.target.value })} /></label><label>事实值<input value={newMemory.fact_value} placeholder="Rust" onChange={(event) => setNewMemory({ ...newMemory, fact_value: event.target.value })} /></label></>}
          <label className="wide-field">场景标签<input value={newMemory.scene_tags} placeholder="project, coding" onChange={(event) => setNewMemory({ ...newMemory, scene_tags: event.target.value })} /></label>
          <label className="wide-field">记忆内容<textarea value={newMemory.content} placeholder="输入可演示的事实；使用相同 fact_key 写入不同值可触发冲突治理。" onChange={(event) => setNewMemory({ ...newMemory, content: event.target.value })} /></label>
        </div>
        <div className="panel-actions"><button className="primary-action" disabled={loading} onClick={createMemory}>写入三层记忆</button><button disabled={loading} onClick={runMaintenance}>执行动态筛选</button><span>空间：私有 {stats.private_memories} · 共享 {stats.shared_memories} · 公共 {stats.global_memories}</span></div>
        <div className="gateway-controls">
          <div><strong>注册协作 Agent</strong><input value={newAgent.key} placeholder="planner" onChange={(event) => setNewAgent({ ...newAgent, key: event.target.value })} /><input value={newAgent.name} placeholder="规划 Agent" onChange={(event) => setNewAgent({ ...newAgent, name: event.target.value })} /><button onClick={createAgent} disabled={loading}>注册</button></div>
          <div><strong>定向共享最近写入</strong><code>{lastCreatedId ? lastCreatedId.slice(0, 8) : '尚未写入'}</code><select value={shareTarget} onChange={(event) => setShareTarget(event.target.value)}><option value="">选择接收 Agent</option>{agents.filter((agent) => agent.agent_key !== agentId).map((agent) => <option key={agent.agent_key} value={agent.agent_key}>{agent.display_name}</option>)}</select><button onClick={shareLastMemory} disabled={loading || !lastCreatedId || !shareTarget}>授权读取</button></div>
        </div>
      </section>

      <section className="lab-panel">
        <div className="panel-heading"><div><span>02 / RETRIEVAL</span><h3>静态 RAG 与自适应检索同屏对照</h3></div><small>语义 · 词法 · 时序 · 可信度 · 频次 · 重要性</small></div>
        <div className="search-controls"><input value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.key === 'Enter' && compareSearch()} /><label>Top-K<input type="number" min="1" max="20" value={topK} onChange={(event) => setTopK(Number(event.target.value))} /></label><button className="primary-action" onClick={compareSearch} disabled={loading}>运行双路检索</button></div>
        <div className="weight-grid">
          {Object.entries(weights).map(([key, value]) => <label key={key}><span>{({ semantic: '语义', lexical: '词法', recency: '时序', trust: '可信度', frequency: '频次', importance: '重要性' } as Record<string, string>)[key]} <b>{value}%</b></span><input type="range" min="0" max="60" value={value} onChange={(event) => setWeights({ ...weights, [key]: Number(event.target.value) })} /></label>)}
        </div>
        <div className="comparison-grid">
          <RunColumn title="传统静态 RAG" subtitle="向量相似度 + 固定重要性" run={staticRun} tone="baseline" />
          <RunColumn title="灵枢自适应检索" subtitle="动态路由 + 冲突/过期过滤 + 多特征重排" run={adaptiveRun} tone="adaptive" />
        </div>
      </section>

      <section className="lab-panel">
        <div className="panel-heading"><div><span>03 / EVIDENCE</span><h3>60 轮可复现实验与消融</h3></div><button className="primary-action" onClick={runBenchmark} disabled={loading}>重跑标准评测</button></div>
        {!benchmark ? <div className="benchmark-empty">点击“重跑标准评测”，由后端现场计算基线、完整模型、消融与阈值结果。</div> : <>
          <div className="benchmark-meta"><strong>{benchmark.dataset}</strong><span>{benchmark.turn_count} 轮 · {benchmark.query_count} 个时序查询</span><p>{benchmark.dataset_description}</p></div>
          <div className="benchmark-compare">
            <MetricBar label="长时序 Top-1" baseline={benchmark.baseline.top1_accuracy} adaptive={benchmark.adaptive.top1_accuracy} suffix="%" />
            <MetricBar label="Top-K（Hit@5）" baseline={benchmark.baseline.top_k_precision} adaptive={benchmark.adaptive.top_k_precision} suffix="%" />
            <MetricBar label="Token 消耗" baseline={benchmark.baseline.estimated_token_count} adaptive={benchmark.adaptive.estimated_token_count} suffix="" lowerBetter />
          </div>
          <div className="threshold-grid">{benchmark.threshold_checks.map((check) => <div className={`threshold-card ${check.passed ? 'passed' : 'failed'}`} key={check.metric}><span>{check.passed ? 'PASS' : 'PENDING'}</span><strong>{check.metric}</strong><b>{check.measured.toFixed(2)}%</b><small>目标 {check.target}</small></div>)}</div>
          <div className="ablation-table"><div><strong>消融项</strong><strong>Top-1</strong><strong>Hit@5</strong></div>{benchmark.ablations.map((row) => <div key={row.name}><span>{row.name}</span><span>{row.top1_accuracy.toFixed(2)}%</span><span>{row.top_k_precision.toFixed(2)}%</span></div>)}</div>
          <p className="evidence-note">证据边界：{benchmark.evidence_note}</p>
        </>}
      </section>

      <section className="lab-panel conflict-panel">
        <div className="panel-heading"><div><span>04 / GOVERNANCE</span><h3>记忆冲突审计</h3></div><small>{conflicts.length} 条决策记录</small></div>
        {conflicts.length === 0 ? <div className="benchmark-empty">暂无冲突。用相同 fact_key 写入不同事实值即可现场演示。</div> : <div className="conflict-list">{conflicts.slice(0, 8).map((item) => <div key={item.id}><b>{item.fact_key}</b><span>{item.resolution}</span><p>{item.reason}</p><small>证据差值 {item.score_delta.toFixed(3)} · {new Date(item.created_at).toLocaleString('zh-CN')}</small></div>)}</div>}
      </section>
    </div>
  );
}

function Stat({ value, label, accent }: { value: number; label: string; accent: string }) {
  return <div className={`lab-stat stat-${accent}`}><b>{value}</b><span>{label}</span></div>;
}

function RunColumn({ title, subtitle, run, tone }: { title: string; subtitle: string; run: SearchRun | null; tone: string }) {
  return <div className={`run-column ${tone}`}><header><div><strong>{title}</strong><span>{subtitle}</span></div>{run && <b>{run.latency_ms.toFixed(1)} ms</b>}</header>{!run ? <div className="run-empty">等待运行检索</div> : <><div className="run-summary"><span>候选 {run.candidate_count}</span><span>过滤 {run.filtered_count}</span><span>Token ≈ {run.estimated_tokens}</span></div><div className="result-list">{run.results.map((result) => <article key={result.id}><b>#{result.rank}</b><div><strong>{result.content}</strong><span>{LAYER_LABEL[result.memory_layer] ?? result.memory_layer} · {SCOPE_LABEL[result.memory_scope] ?? result.memory_scope} · {result.owner_agent_id}</span><small>{run.mode === 'static' ? `语义 ${result.score_breakdown.semantic.toFixed(2)} / 重要性 ${result.score_breakdown.importance.toFixed(2)}` : `语义 ${result.score_breakdown.semantic.toFixed(2)} / 时序 ${result.score_breakdown.recency.toFixed(2)} / 可信 ${result.score_breakdown.trust.toFixed(2)}`}</small></div><em>{result.score.toFixed(3)}</em></article>)}</div></>}</div>;
}

function MetricBar({ label, baseline, adaptive, suffix, lowerBetter = false }: { label: string; baseline: number; adaptive: number; suffix: string; lowerBetter?: boolean }) {
  const maximum = Math.max(baseline, adaptive, 1);
  return <div className="metric-row"><strong>{label}</strong><div><span>静态</span><i><em style={{ width: `${baseline / maximum * 100}%` }} /></i><b>{baseline.toFixed(suffix ? 2 : 0)}{suffix}</b></div><div className="adaptive-bar"><span>自适应</span><i><em style={{ width: `${adaptive / maximum * 100}%` }} /></i><b>{adaptive.toFixed(suffix ? 2 : 0)}{suffix}{lowerBetter ? ' ↓' : ''}</b></div></div>;
}
