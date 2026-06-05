export function AvatarPlaceholder() {
  return (
    <div style={{
      flex: 1,
      background: 'var(--bg-secondary)',
      borderRadius: '12px',
      border: '1px solid var(--border)',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '16px',
      padding: '24px',
    }}>
      <div style={{
        width: '120px',
        height: '120px',
        borderRadius: '50%',
        background: 'linear-gradient(135deg, var(--accent), var(--accent-light))',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        fontSize: '48px',
      }} />
      <div style={{
        fontSize: '16px',
        fontWeight: 500,
        color: 'var(--text-primary)',
      }}>
        灵枢
      </div>
      <div style={{
        fontSize: '12px',
        color: 'var(--text-secondary)',
        textAlign: 'center',
      }}>
        3D 虚拟形象将在 Phase 2 实现
      </div>
    </div>
  );
}
