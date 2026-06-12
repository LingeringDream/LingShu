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
        background: 'radial-gradient(circle at 50% 48%, rgba(255,255,255,0.22), rgba(46,107,255,0.28) 38%, rgba(46,107,255,0.08) 62%, transparent 72%)',
        boxShadow: '0 18px 52px rgba(46, 107, 255, 0.18)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        position: 'relative',
        color: 'white',
        fontSize: '40px',
      }}>
        <div style={{
          position: 'absolute',
          width: '150px',
          height: '38px',
          border: '2px solid rgba(46,107,255,0.42)',
          borderRadius: '50%',
          transform: 'rotate(-18deg)',
        }} />
        <div style={{
          position: 'absolute',
          width: '72px',
          height: '72px',
          border: '1.5px solid rgba(255,255,255,0.38)',
          borderRadius: '50%',
          background: 'rgba(46,107,255,0.18)',
        }} />
        <div style={{
          width: 44,
          height: 44,
          borderRadius: '50%',
          background: 'radial-gradient(circle, rgba(255,255,255,0.92), rgba(46,107,255,0.34) 48%, rgba(46,107,255,0.14) 70%)',
          position: 'absolute',
        }} />
        <div style={{
          width: 0,
          height: 0,
          borderLeft: '10px solid transparent',
          borderRight: '10px solid transparent',
          borderBottom: '26px solid rgba(255,255,255,0.86)',
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
