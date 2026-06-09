// ── Desktop Pet Window ─────────────────────────────────────────────────

import { useEffect, useRef, useState } from 'react';
import { isTauri, showMainWindow } from '../../lib/tauri';

type Mood = 'idle' | 'thinking' | 'speaking';

const MOOD_EMOJI: Record<Mood, string> = {
  idle: '😊',
  thinking: '🤔',
  speaking: '💬',
};

// Pointer must travel this many px (with the button held) before we treat the
// gesture as a window-drag rather than a click. Keeps a clean tap → open main.
const DRAG_THRESHOLD_PX = 4;

export function PetWindow() {
  const [mood, setMood] = useState<Mood>('idle');
  const [bubble, setBubble] = useState<string | null>(null);
  const [inTauri, setInTauri] = useState(false);
  // True once the current press has crossed the drag threshold, so the trailing
  // click event can be ignored (a drag should not also open the main window).
  const draggedRef = useRef(false);

  useEffect(() => {
    setInTauri(isTauri());
  }, []);

  useEffect(() => {
    const moods: Mood[] = ['idle', 'thinking', 'idle', 'speaking'];
    let i = 0;
    const timer = setInterval(() => {
      i = (i + 1) % moods.length;
      setMood(moods[i]);
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  // Native Tauri window drag. `-webkit-app-region: drag` (Electron) is NOT
  // honoured by Tauri's WKWebView, so we drive the OS drag ourselves: on
  // press-and-move we call `startDragging()` (granted via the
  // `core:window:allow-start-dragging` capability). A press without movement
  // falls through to the click handler.
  const handleDragMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0 || !inTauri) return;
    const startX = e.clientX;
    const startY = e.clientY;
    draggedRef.current = false;

    const onMove = (ev: MouseEvent) => {
      if (draggedRef.current) return;
      if (Math.hypot(ev.clientX - startX, ev.clientY - startY) > DRAG_THRESHOLD_PX) {
        draggedRef.current = true;
        cleanup();
        // The OS takes over the drag loop here; the webview will not see the
        // trailing mouseup, which is why listeners are removed first.
        import('@tauri-apps/api/window')
          .then(({ getCurrentWindow }) => getCurrentWindow().startDragging())
          .catch((err) => console.error('[pet] startDragging failed:', err));
      }
    };
    const cleanup = () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', cleanup);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', cleanup);
  };

  const handleAvatarClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    // Suppress the click that trails a drag gesture.
    if (draggedRef.current) {
      draggedRef.current = false;
      return;
    }
    if (inTauri) await showMainWindow();
    setBubble('我在呢！');
    setTimeout(() => setBubble(null), 2000);
  };

  const handleAvatarDoubleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (draggedRef.current) return;
    setBubble('你好！有什么需要帮助的吗？');
    setTimeout(() => setBubble(null), 3000);
  };

  return (
    <div
      // The whole transparent window is a drag surface; dragging the avatar
      // works too because the press bubbles up to this handler.
      onMouseDown={handleDragMouseDown}
      style={{
        width: '100vw', height: '100vh',
        display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center',
        background: 'transparent', userSelect: 'none',
        WebkitUserSelect: 'none',
      }}
    >
      {/* Avatar — draggable (press + move) AND clickable (clean tap) */}
      <div
        onClick={handleAvatarClick}
        onDoubleClick={handleAvatarDoubleClick}
        style={{
          width: 100, height: 100, borderRadius: '50%',
          background: 'linear-gradient(135deg, rgba(100,140,255,0.85) 0%, rgba(140,180,255,0.85) 100%)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 44, cursor: 'grab',
          boxShadow: '0 4px 24px rgba(100,140,255,0.35)',
          backdropFilter: 'blur(8px)', WebkitBackdropFilter: 'blur(8px)',
          transition: 'transform 0.3s ease',
          transform: mood === 'thinking' ? 'scale(0.95)' : 'scale(1)',
        }}
      >
        {MOOD_EMOJI[mood]}
      </div>

      {/* Name */}
      <div style={{
        marginTop: 8, fontSize: 13, fontWeight: 600,
        color: 'rgba(255,255,255,0.9)',
        textShadow: '0 1px 4px rgba(0,0,0,0.3)',
      }}>灵枢</div>

      {/* Bubble */}
      {bubble && (
        <div style={{
          marginTop: 6, padding: '4px 12px', borderRadius: 12,
          background: 'rgba(255,255,255,0.92)', color: '#333',
          fontSize: 12, maxWidth: 160, textAlign: 'center',
          boxShadow: '0 2px 12px rgba(0,0,0,0.12)',
          animation: 'fadeIn 0.25s ease', pointerEvents: 'none',
        }}>{bubble}</div>
      )}

      {!inTauri && (
        <div style={{
          marginTop: 8, padding: '2px 8px', borderRadius: 8,
          background: 'rgba(0,0,0,0.5)', color: '#fffa', fontSize: 10,
        }}>Tauri 未连接</div>
      )}

      <style>{`
        @keyframes fadeIn {
          from { opacity: 0; transform: translateY(4px); }
          to   { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}
