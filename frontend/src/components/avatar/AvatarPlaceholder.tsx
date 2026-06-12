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
        width: '124px',
        height: '124px',
        borderRadius: '50%',
        background: 'radial-gradient(circle at 50% 44%, rgba(255,255,255,0.28), rgba(46,107,255,0.92) 46%, rgba(88,72,245,0.82) 76%)',
        boxShadow: '0 18px 48px rgba(46, 107, 255, 0.26)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        position: 'relative',
        color: 'white',
        fontSize: '40px',
      }}>
        <div style={{
          position: 'absolute',
          width: '148px',
          height: '42px',
          border: '2px solid rgba(255,255,255,0.56)',
          borderRadius: '50%',
          transform: 'rotate(-18deg)',
        }} />
        <div style={{
          width: 0,
          height: 0,
          borderLeft: '16px solid transparent',
          borderRight: '16px solid transparent',
          borderBottom: '38px solid rgba(255,255,255,0.82)',
          filter: 'drop-shadow(0 4px 10px rgba(31,42,68,0.16))',
          transform: 'rotate(45deg)',
        }} />
      </div>
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
        星核形象 · 点击桌宠显示对话框
      </div>
    </div>
  );
}
