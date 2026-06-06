// ── Desktop Pet Window ─────────────────────────────────────────────────
// Rendered inside the "pet" Tauri window (frameless, transparent, always-on-top).
// Falls back to a static preview when running in the browser.

import { useEffect, useState } from 'react';
import { isTauri, showMainWindow } from '../../lib/tauri';

type Mood = 'idle' | 'thinking' | 'speaking';

const MOOD_EMOJI: Record<Mood, string> = {
  idle: '😊',
  thinking: '🤔',
  speaking: '💬',
};

export function PetWindow() {
  const [mood, setMood] = useState<Mood>('idle');
  const [bubble, setBubble] = useState<string | null>(null);
  const [inTauri, setInTauri] = useState(false);

  useEffect(() => {
    setInTauri(isTauri());
  }, []);

  // Cycle moods for visual interest
  useEffect(() => {
    const moods: Mood[] = ['idle', 'thinking', 'idle', 'speaking'];
    let i = 0;
    const timer = setInterval(() => {
      i = (i + 1) % moods.length;
      setMood(moods[i]);
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  const handleClick = async () => {
    if (inTauri) {
      await showMainWindow();
    }
    // Show a brief bubble
    setBubble('我在呢！');
    setTimeout(() => setBubble(null), 2000);
  };

  const handleDoubleClick = () => {
    setBubble('你好！有什么需要帮助的吗？');
    setTimeout(() => setBubble(null), 3000);
  };

  return (
    <div
      data-tauri-drag-region
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        // Transparent background — Tauri makes the window see-through
        background: 'transparent',
        cursor: 'pointer',
        position: 'relative',
      }}
    >
      {/* Avatar circle with subtle shadow for depth */}
      <div
        style={{
          width: 100,
          height: 100,
          borderRadius: '50%',
          background:
            'linear-gradient(135deg, rgba(100,140,255,0.85) 0%, rgba(140,180,255,0.85) 100%)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: 44,
          boxShadow: '0 4px 24px rgba(100,140,255,0.35), 0 1px 4px rgba(0,0,0,0.1)',
          backdropFilter: 'blur(8px)',
          WebkitBackdropFilter: 'blur(8px)',
          transition: 'transform 0.3s ease',
          transform: mood === 'thinking' ? 'scale(0.95)' : 'scale(1)',
        }}
      >
        {MOOD_EMOJI[mood]}
      </div>

      {/* Name label */}
      <div
        style={{
          marginTop: 8,
          fontSize: 13,
          fontWeight: 600,
          color: 'rgba(255,255,255,0.9)',
          textShadow: '0 1px 4px rgba(0,0,0,0.3)',
          letterSpacing: '0.02em',
        }}
      >
        灵枢
      </div>

      {/* Bubble */}
      {bubble && (
        <div
          style={{
            marginTop: 6,
            padding: '4px 12px',
            borderRadius: 12,
            background: 'rgba(255,255,255,0.92)',
            color: '#333',
            fontSize: 12,
            maxWidth: 160,
            textAlign: 'center',
            boxShadow: '0 2px 12px rgba(0,0,0,0.12)',
            animation: 'fadeIn 0.25s ease',
            pointerEvents: 'none',
          }}
        >
          {bubble}
        </div>
      )}

      {/* Non-Tauri badge */}
      {!inTauri && (
        <div
          style={{
            marginTop: 8,
            padding: '2px 8px',
            borderRadius: 8,
            background: 'rgba(0,0,0,0.5)',
            color: '#fffa',
            fontSize: 10,
          }}
        >
          Tauri 未连接
        </div>
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
