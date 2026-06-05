import { useState, useEffect } from 'react';
import { useProjectStore, type Project } from '../../stores/projectStore';

export function ProjectManager() {
  const { projects, loading, error, fetchProjects, createProject, updateProject, deleteProject } =
    useProjectStore();
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [editDesc, setEditDesc] = useState('');

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  const handleCreate = async () => {
    if (!name.trim()) return;
    await createProject({ name: name.trim(), description: description.trim() || undefined });
    setName('');
    setDescription('');
    setShowCreate(false);
  };

  const handleUpdate = async (id: string) => {
    if (!editName.trim()) return;
    await updateProject(id, { name: editName.trim(), description: editDesc.trim() || undefined });
    setEditingId(null);
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm('确定要删除这个项目吗？')) return;
    await deleteProject(id);
  };

  const startEdit = (p: Project) => {
    setEditingId(p.id);
    setEditName(p.name);
    setEditDesc(p.description || '');
  };

  return (
    <div style={{ padding: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <h3 style={{ margin: 0 }}>项目管理</h3>
        <button
          type="button"
          onClick={() => setShowCreate(true)}
          style={{
            padding: '6px 16px',
            background: 'var(--color-accent, #0a73ff)',
            color: '#fff',
            border: 'none',
            borderRadius: 4,
            cursor: 'pointer',
            fontSize: 13,
          }}
        >
          新建项目
        </button>
      </div>

      {error && (
        <p style={{ color: '#e05555', fontSize: 13, marginBottom: 12 }}>{error}</p>
      )}

      {/* Create form */}
      {showCreate && (
        <div
          style={{
            padding: 14,
            marginBottom: 16,
            border: '1px solid var(--color-accent, #0a73ff)',
            borderRadius: 6,
            background: '#f8faff',
          }}
        >
          <h4 style={{ margin: '0 0 10px', fontSize: 14 }}>新建项目</h4>
          <input
            placeholder="项目名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
            style={{ display: 'block', width: '100%', padding: 6, marginBottom: 8, fontSize: 13, border: '1px solid #ddd', borderRadius: 4 }}
          />
          <input
            placeholder="描述 (可选)"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            style={{ display: 'block', width: '100%', padding: 6, marginBottom: 8, fontSize: 13, border: '1px solid #ddd', borderRadius: 4 }}
          />
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              type="button"
              onClick={handleCreate}
              disabled={loading}
              style={{
                padding: '4px 14px',
                background: 'var(--color-accent, #0a73ff)',
                color: '#fff',
                border: 'none',
                borderRadius: 4,
                cursor: 'pointer',
                fontSize: 13,
              }}
            >
              {loading ? '创建中...' : '创建'}
            </button>
            <button
              type="button"
              onClick={() => setShowCreate(false)}
              style={{ padding: '4px 14px', fontSize: 13, border: '1px solid #ddd', borderRadius: 4, background: 'transparent', cursor: 'pointer' }}
            >
              取消
            </button>
          </div>
        </div>
      )}

      {/* Project list */}
      {loading && projects.length === 0 ? (
        <p style={{ fontSize: 13 }}>加载中...</p>
      ) : projects.length === 0 ? (
        <p style={{ fontSize: 13, color: 'var(--color-secondary-text, #888)' }}>
          暂无项目。点击"新建项目"创建第一个。
        </p>
      ) : (
        <div>
          {projects.map((p) => (
            <div
              key={p.id}
              style={{
                padding: '10px 14px',
                marginBottom: 10,
                border: '1px solid var(--color-border, #ddd)',
                borderRadius: 6,
                fontSize: 13,
              }}
            >
              {editingId === p.id ? (
                <div>
                  <input
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    style={{ display: 'block', width: '100%', padding: 4, marginBottom: 6, fontSize: 13 }}
                  />
                  <input
                    value={editDesc}
                    onChange={(e) => setEditDesc(e.target.value)}
                    style={{ display: 'block', width: '100%', padding: 4, marginBottom: 6, fontSize: 13 }}
                  />
                  <div style={{ display: 'flex', gap: 8 }}>
                    <button
                      type="button"
                      onClick={() => handleUpdate(p.id)}
                      style={{ padding: '2px 10px', fontSize: 12, background: '#0a73ff', color: '#fff', border: 'none', borderRadius: 3, cursor: 'pointer' }}
                    >
                      保存
                    </button>
                    <button
                      type="button"
                      onClick={() => setEditingId(null)}
                      style={{ padding: '2px 10px', fontSize: 12, border: '1px solid #ddd', borderRadius: 3, background: 'transparent', cursor: 'pointer' }}
                    >
                      取消
                    </button>
                  </div>
                </div>
              ) : (
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div>
                    <strong>{p.name}</strong>
                    {p.description && (
                      <span style={{ marginLeft: 8, color: '#888' }}>{p.description}</span>
                    )}
                    <span
                      style={{
                        marginLeft: 8,
                        fontSize: 11,
                        padding: '1px 6px',
                        borderRadius: 8,
                        background: p.status === 'active' ? '#e6f4e6' : '#f0f0f0',
                      }}
                    >
                      {p.status}
                    </span>
                  </div>
                  <div style={{ display: 'flex', gap: 6 }}>
                    <button
                      type="button"
                      onClick={() => startEdit(p)}
                      style={{ padding: '2px 8px', fontSize: 12, border: '1px solid #ddd', borderRadius: 3, background: 'transparent', cursor: 'pointer' }}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      onClick={() => handleDelete(p.id)}
                      style={{ padding: '2px 8px', fontSize: 12, border: '1px solid #ecc', borderRadius: 3, background: 'transparent', color: '#c55', cursor: 'pointer' }}
                    >
                      删除
                    </button>
                  </div>
                </div>
              )}
              {p.health_score != null && (
                <div style={{ marginTop: 4, fontSize: 11, color: '#999' }}>
                  健康度: {(p.health_score * 100).toFixed(0)}%
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
