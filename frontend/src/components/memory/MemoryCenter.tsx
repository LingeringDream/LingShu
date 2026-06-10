import { useEffect, useState } from 'react';
import { useMemoryStore } from '../../stores/memoryStore';

const TYPE_LABELS: Record<string, string> = {
  preference: '偏好',
  fact: '事实',
  goal: '目标',
  context: '上下文',
};

export function MemoryCenter() {
  const {
    memories,
    loading,
    error,
    searchQuery,
    typeFilter,
    fetchMemories,
    searchMemories,
    createMemory,
    updateMemory,
    deleteMemory,
    setSearchQuery,
    setTypeFilter,
  } = useMemoryStore();

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');
  const [newContent, setNewContent] = useState('');
  const [newType, setNewType] = useState('fact');
  const [expanded, setExpanded] = useState(true);

  useEffect(() => {
    fetchMemories();
  }, [fetchMemories]);

  const filtered = typeFilter
    ? memories.filter((m) => m.memory_type === typeFilter)
    : memories;

  const handleSearch = () => {
    if (searchQuery.trim()) {
      searchMemories(searchQuery);
    } else {
      fetchMemories();
    }
  };

  const handleCreate = async () => {
    if (!newContent.trim()) return;
    try {
      await createMemory({ memory_type: newType, content: newContent, importance: 0.7 });
      setNewContent('');
    } catch (e) {
      // createMemory throws — show the error inline
      setNewContent(`错误: ${e instanceof Error ? e.message : '创建失败'}`);
    }
  };

  const handleSave = async (id: string) => {
    await updateMemory(id, { content: editContent });
    setEditingId(null);
  };

  const cardStyle: React.CSSProperties = {
    background: 'var(--bg-tertiary)',
    borderRadius: '8px',
    padding: '12px',
    fontSize: '13px',
    lineHeight: '1.5',
    border: '1px solid var(--border)',
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Header */}
      <div
        style={{
          padding: '12px 20px',
          borderBottom: '1px solid var(--border)',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          cursor: 'pointer',
        }}
        onClick={() => setExpanded(!expanded)}
      >
        <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-secondary)' }}>
          记忆中心 ({memories.length})
        </span>
        <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
          {expanded ? '▼' : '▶'}
        </span>
      </div>

      {!expanded && null}

      {expanded && (
        <div style={{ display: 'flex', flexDirection: 'column', flex: 1, overflow: 'hidden' }}>
          {/* Search + Filter */}
          <div style={{ padding: '12px 20px', display: 'flex', gap: '8px' }}>
            <input
              placeholder="搜索记忆..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              style={{
                flex: 1,
                background: 'var(--bg-tertiary)',
                border: '1px solid var(--border)',
                borderRadius: '6px',
                padding: '6px 10px',
                color: 'var(--text-primary)',
                fontSize: '13px',
                outline: 'none',
              }}
            />
            <select
              value={typeFilter}
              onChange={(e) => setTypeFilter(e.target.value)}
              style={{
                background: 'var(--bg-tertiary)',
                border: '1px solid var(--border)',
                borderRadius: '6px',
                color: 'var(--text-primary)',
                fontSize: '12px',
                padding: '4px 8px',
              }}
            >
              <option value="">全部</option>
              <option value="preference">偏好</option>
              <option value="fact">事实</option>
              <option value="goal">目标</option>
              <option value="context">上下文</option>
            </select>
          </div>

          {/* New memory input */}
          <div style={{ padding: '0 20px 12px', display: 'flex', gap: '8px' }}>
            <input
              placeholder="添加记忆..."
              value={newContent}
              onChange={(e) => setNewContent(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
              style={{
                flex: 1,
                background: 'var(--bg-tertiary)',
                border: '1px solid var(--border)',
                borderRadius: '6px',
                padding: '6px 10px',
                color: 'var(--text-primary)',
                fontSize: '13px',
                outline: 'none',
              }}
            />
            <select
              value={newType}
              onChange={(e) => setNewType(e.target.value)}
              style={{
                background: 'var(--bg-tertiary)',
                border: '1px solid var(--border)',
                borderRadius: '6px',
                color: 'var(--text-primary)',
                fontSize: '12px',
                padding: '4px 8px',
              }}
            >
              <option value="fact">事实</option>
              <option value="preference">偏好</option>
              <option value="goal">目标</option>
              <option value="context">上下文</option>
            </select>
          </div>

          {/* Memory list */}
          <div
            style={{
              flex: 1,
              overflow: 'auto',
              padding: '0 20px 16px',
              display: 'flex',
              flexDirection: 'column',
              gap: '8px',
            }}
          >
            {loading && <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>加载中...</span>}
            {error && <span style={{ fontSize: '12px', color: 'var(--error)' }}>{error}</span>}

            {!loading && filtered.length === 0 && (
              <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                暂无记忆。聊天时会自动提取，或手动添加。
              </span>
            )}

            {filtered.map((m) => (
              <div key={m.id} style={cardStyle}>
                {/* Header: type tag + importance + actions */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '6px' }}>
                  <span
                    style={{
                      fontSize: '10px',
                      background: 'var(--accent)',
                      color: '#fff',
                      borderRadius: '4px',
                      padding: '1px 6px',
                      opacity: 0.8,
                    }}
                  >
                    {TYPE_LABELS[m.memory_type] ?? m.memory_type}
                  </span>
                  <span style={{ fontSize: '10px', color: 'var(--text-secondary)' }}>
                    重要性 {m.importance.toFixed(1)}
                  </span>
                  <div style={{ marginLeft: 'auto', display: 'flex', gap: '6px' }}>
                    <button
                      onClick={() => {
                        setEditingId(m.id);
                        setEditContent(m.content);
                      }}
                      style={{
                        background: 'none',
                        border: 'none',
                        color: 'var(--text-secondary)',
                        cursor: 'pointer',
                        fontSize: '11px',
                      }}
                    >
                      编辑
                    </button>
                    <button
                      onClick={() => deleteMemory(m.id)}
                      style={{
                        background: 'none',
                        border: 'none',
                        color: 'var(--text-secondary)',
                        cursor: 'pointer',
                        fontSize: '11px',
                      }}
                    >
                      删除
                    </button>
                  </div>
                </div>

                {/* Content (edit mode or read mode) */}
                {editingId === m.id ? (
                  <div style={{ display: 'flex', gap: '6px' }}>
                    <textarea
                      value={editContent}
                      onChange={(e) => setEditContent(e.target.value)}
                      rows={3}
                      style={{
                        flex: 1,
                        background: 'var(--bg-primary)',
                        border: '1px solid var(--accent)',
                        borderRadius: '6px',
                        padding: '8px',
                        color: 'var(--text-primary)',
                        fontSize: '13px',
                        resize: 'none',
                        outline: 'none',
                        fontFamily: 'inherit',
                      }}
                    />
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      <button
                        onClick={() => handleSave(m.id)}
                        style={{
                          background: 'var(--accent)',
                          border: 'none',
                          color: '#fff',
                          borderRadius: '4px',
                          padding: '4px 8px',
                          fontSize: '11px',
                          cursor: 'pointer',
                        }}
                      >
                        保存
                      </button>
                      <button
                        onClick={() => setEditingId(null)}
                        style={{
                          background: 'var(--bg-tertiary)',
                          border: '1px solid var(--border)',
                          borderRadius: '4px',
                          padding: '4px 8px',
                          fontSize: '11px',
                          cursor: 'pointer',
                          color: 'var(--text-secondary)',
                        }}
                      >
                        取消
                      </button>
                    </div>
                  </div>
                ) : (
                  <div style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                    {m.content}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
