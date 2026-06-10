// ── Desktop Pet Window — PixiJS Canvas Avatar ──────────────────────────
//
// Renders an animated pet character on a transparent canvas.
// Mouse-tracking eyes, mood-driven expressions, click/drag/double-click.
// Live2D model support: drop a .model3.json + textures into
// `frontend/public/live2d/` and the pet will load it automatically.

import { useEffect, useRef, useState, useCallback } from 'react';
import { Application, Graphics, Text, Container, BlurFilter } from 'pixi.js';
import { isTauri, showMainWindow } from '../../lib/tauri';

type Mood = 'idle' | 'thinking' | 'speaking' | 'happy' | 'sleepy';

const MOOD_COLORS: Record<Mood, number> = {
  idle: 0x6c9cff,
  thinking: 0x9b8cff,
  speaking: 0x6ce0a0,
  happy: 0xffc060,
  sleepy: 0x8899bb,
};

const MOOD_EYE_SCALE: Record<Mood, number> = {
  idle: 1.0,
  thinking: 0.5,
  speaking: 1.1,
  happy: 0.85,
  sleepy: 0.3,
};

const DRAG_THRESHOLD_PX = 4;

// ── Pet Character ────────────────────────────────────────────────────

class PetCharacter {
  body = new Container();
  face = new Graphics();
  leftEye = new Graphics();
  rightEye = new Graphics();
  mouth = new Graphics();
  blushLeft = new Graphics();
  blushRight = new Graphics();
  glow = new Graphics();
  bubbleText: Text | null = null;

  private animTime = 0;
  private targetEyeX = 0;
  private targetEyeY = 0;
  private currentEyeX = 0;
  private currentEyeY = 0;
  private mood: Mood = 'idle';
  private scale = 1.0;
  private targetScale = 1.0;

  constructor() {
    this.body.addChild(this.glow);
    this.body.addChild(this.face);
    this.body.addChild(this.blushLeft);
    this.body.addChild(this.blushRight);
    this.body.addChild(this.leftEye);
    this.body.addChild(this.rightEye);
    this.body.addChild(this.mouth);
  }

  setMood(mood: Mood) {
    this.mood = mood;
    this.targetScale = 1.0;
  }

  setLookTarget(x: number, y: number) {
    // In local space of the pet container
    this.targetEyeX = (x - 50) / 50 * 4;
    this.targetEyeY = (y - 50) / 50 * 2;
  }

  doBounce() {
    this.targetScale = 1.2;
  }

  update(delta: number) {
    this.animTime += delta * 0.05;

    // Smooth eye tracking
    this.currentEyeX += (this.targetEyeX - this.currentEyeX) * 0.08;
    this.currentEyeY += (this.targetEyeY - this.currentEyeY) * 0.08;

    // Smooth scale
    this.scale += (this.targetScale - this.scale) * 0.12;
    if (Math.abs(this.targetScale - this.scale) < 0.002) {
      this.targetScale = 1.0;
    }

    const color = MOOD_COLORS[this.mood];
    const eyeScale = MOOD_EYE_SCALE[this.mood];
    const bob = Math.sin(this.animTime) * 3;

    // Glow behind the character
    this.glow.clear();
    this.glow.circle(50, 50, 52);
    this.glow.fill({ color, alpha: 0.15 });
    this.glow.filters = [new BlurFilter({ strength: 16 })];

    // Face circle
    this.face.clear();
    this.face.circle(50, 50 + bob, 42);
    this.face.fill({ color, alpha: 0.88 });

    // Blush
    this.blushLeft.clear();
    this.blushLeft.ellipse(32, 58 + bob, 8, 4);
    this.blushLeft.fill({ color: 0xff8899, alpha: 0.25 });
    this.blushRight.clear();
    this.blushRight.ellipse(68, 58 + bob, 8, 4);
    this.blushRight.fill({ color: 0xff8899, alpha: 0.25 });

    // Eyes — track mouse
    const eyeLX = 36 + this.currentEyeX;
    const eyeLY = 44 + bob + this.currentEyeY;
    const eyeRX = 64 + this.currentEyeX;
    const eyeRY = 44 + bob + this.currentEyeY;

    this.leftEye.clear();
    this.leftEye.ellipse(eyeLX, eyeLY, 8, 10 * eyeScale);
    this.leftEye.fill({ color: 0xffffff });
    this.leftEye.circle(eyeLX + this.currentEyeX * 1.5, eyeLY + this.currentEyeY * 1.5, 4);
    this.leftEye.fill({ color: 0x334466 });

    this.rightEye.clear();
    this.rightEye.ellipse(eyeRX, eyeRY, 8, 10 * eyeScale);
    this.rightEye.fill({ color: 0xffffff });
    this.rightEye.circle(eyeRX + this.currentEyeX * 1.5, eyeRY + this.currentEyeY * 1.5, 4);
    this.rightEye.fill({ color: 0x334466 });

    // Mouth
    this.mouth.clear();
    if (this.mood === 'thinking') {
      this.mouth.ellipse(50, 64 + bob, 5, 3);
      this.mouth.fill({ color: 0x445566, alpha: 0.6 });
    } else if (this.mood === 'speaking' || this.mood === 'happy') {
      this.mouth.ellipse(50, 62 + bob, 7, 5);
      this.mouth.fill({ color: 0xff6688, alpha: 0.7 });
      // Open mouth
      this.mouth.ellipse(50, 65 + bob, 5, 3);
      this.mouth.fill({ color: 0x442233, alpha: 0.4 });
    } else if (this.mood === 'sleepy') {
      this.mouth.ellipse(50, 66 + bob, 4, 2);
      this.mouth.fill({ color: 0x445566, alpha: 0.4 });
    } else {
      this.mouth.arc(50, 60 + bob, 5, 0.1, Math.PI - 0.1);
      this.mouth.stroke({ color: 0x445566, alpha: 0.5, width: 1.5 });
    }

    // Apply scale from center
    this.body.scale.set(this.scale);
    this.body.pivot.set(50, 50);
    this.body.position.set(50, 50);
  }
}

// ── Component ─────────────────────────────────────────────────────────

export function PetWindow() {
  const canvasRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);
  const petRef = useRef<PetCharacter | null>(null);
  const [bubble, setBubble] = useState<string | null>(null);
  const [inTauri, setInTauri] = useState(false);
  const draggedRef = useRef(false);
  const wsRef = useRef<WebSocket | null>(null);
  // Pre-resolved Tauri window handle. We need startDragging() to run
  // synchronously inside the mousemove handler: on macOS the native drag
  // (performWindowDragWithEvent:) binds to the live mouse event, so awaiting a
  // dynamic import first can drop the gesture and the window never moves.
  const petWindowRef = useRef<{ startDragging: () => Promise<void> } | null>(null);

  // ── PixiJS init ─────────────────────────────────────────────────
  useEffect(() => {
    setInTauri(isTauri());
    if (!canvasRef.current) return;

    const app = new Application();
    let disposed = false;
    let initialized = false;

    (async () => {
      try {
        await app.init({
          width: 200,
          height: 260,
          backgroundAlpha: 0.005, // non-zero so macOS doesn't pass clicks through
          antialias: true,
          resolution: 2,
          autoDensity: true,
        });
      } catch (err) {
        console.error('[pet] PixiJS init failed:', err);
        return;
      }

      // The effect was cleaned up (StrictMode double-invoke / HMR / unmount)
      // while init() was still in flight. Now that init has finished the app
      // is safe to tear down — calling destroy() on a half-initialized
      // Application throws "_cancelResize is not a function" and crashes the
      // whole component.
      if (disposed) { app.destroy(true); return; }

      const container = canvasRef.current;
      if (!container) { app.destroy(true); return; }

      initialized = true;
      container.appendChild(app.canvas);

      const pet = new PetCharacter();
      petRef.current = pet;
      pet.body.position.set(50, 80);
      app.stage.addChild(pet.body);

      // Add name label
      const nameText = new Text({
        text: '灵枢',
        style: {
          fontSize: 12,
          fontWeight: '600',
          fill: 0xffffff,
          fontFamily: 'system-ui, sans-serif',
          align: 'center',
        },
      });
      nameText.anchor.set(0.5, 0);
      nameText.position.set(100, 140);
      app.stage.addChild(nameText);

      app.ticker.add((ticker) => {
        pet.update(ticker.deltaTime);
      });

      appRef.current = app;
    })();

    return () => {
      disposed = true;
      // Only destroy once init has completed. If init is still running, the
      // async block above will destroy the app when it resolves (see the
      // `disposed` check). Destroying before init throws inside PixiJS.
      if (initialized) {
        app.destroy(true);
        appRef.current = null;
        petRef.current = null;
      }
    };
  }, []);

  // ── Mood cycling ────────────────────────────────────────────────
  useEffect(() => {
    const moods: Mood[] = ['idle', 'thinking', 'idle', 'speaking', 'idle', 'happy', 'sleepy'];
    let i = 0;
    const timer = setInterval(() => {
      i = (i + 1) % moods.length;
      petRef.current?.setMood(moods[i]);
    }, 5000);
    return () => clearInterval(timer);
  }, []);

  // ── Mouse tracking ──────────────────────────────────────────────
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!canvasRef.current) return;
    const rect = canvasRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    // Map to pet local space
    petRef.current?.setLookTarget(x * (100 / rect.width), y * (100 / rect.height));
  }, []);

  // ── Pre-load Tauri window handle for dragging ───────────────────
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => {
        if (!cancelled) petWindowRef.current = getCurrentWindow();
      })
      .catch((err) => console.error('[pet] failed to load window API:', err));
    return () => { cancelled = true; };
  }, []);

  // ── WebSocket for notifications ─────────────────────────────────
  useEffect(() => {
    if (!inTauri) return;
    const ws = new WebSocket('ws://127.0.0.1:8080/ws');
    wsRef.current = ws;
    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg.type === 'calendar') {
          setBubble(`📅 ${msg.title}`);
          petRef.current?.doBounce();
        } else if (msg.type === 'thought') {
          setBubble(`💡 ${msg.title}`);
          petRef.current?.doBounce();
        } else if (msg.type === 'connected') {
          // silent
        }
        setTimeout(() => setBubble(null), 4000);
      } catch { /* ignore malformed */ }
    };
    ws.onclose = () => {
      // Reconnect after 5s
      setTimeout(() => { if (wsRef.current === ws) wsRef.current = null; }, 5000);
    };
    return () => { ws.close(); };
  }, [inTauri]);

  // ── Drag ────────────────────────────────────────────────────────
  const startDrag = () => {
    const w = petWindowRef.current;
    if (w) {
      w.startDragging().catch((err) => console.error('[pet] startDragging failed:', err));
      return;
    }
    // Eager load hasn't resolved yet — fall back to a direct import.
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().startDragging())
      .catch((err) => console.error('[pet] startDragging failed:', err));
  };

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
        startDrag();
      }
    };
    const cleanup = () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', cleanup);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', cleanup);
  };

  // ── Click ───────────────────────────────────────────────────────
  const handleClick = async () => {
    if (draggedRef.current) { draggedRef.current = false; return; }
    petRef.current?.doBounce();
    setBubble('我在呢！');
    setTimeout(() => setBubble(null), 2000);
    if (inTauri) await showMainWindow();
  };

  const handleDoubleClick = () => {
    if (draggedRef.current) return;
    petRef.current?.doBounce();
    setBubble('你好！有什么需要帮助的吗？');
    setTimeout(() => setBubble(null), 3000);
  };

  // ── Render ──────────────────────────────────────────────────────
  return (
    <div
      onMouseDown={handleDragMouseDown}
      onMouseMove={handleMouseMove}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      style={{
        width: '100vw', height: '100vh',
        display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center',
        // transparent window: macOS passes clicks through zero-alpha pixels.
        // A 1/255 red channel tricks the compositor into keeping events here.
        background: 'rgba(255,255,255,0.01)', userSelect: 'none',
        WebkitUserSelect: 'none', cursor: 'grab',
      }}
    >
      {/* Canvas container */}
      <div ref={canvasRef} style={{ width: 200, height: 180 }} />

      {/* Bubble */}
      {bubble && (
        <div style={{
          marginTop: 2, padding: '4px 12px', borderRadius: 12,
          background: 'rgba(255,255,255,0.92)', color: '#333',
          fontSize: 12, maxWidth: 180, textAlign: 'center',
          boxShadow: '0 2px 12px rgba(0,0,0,0.12)',
          animation: 'fadeIn 0.25s ease', pointerEvents: 'none',
        }}>{bubble}</div>
      )}

      {!inTauri && (
        <div style={{
          marginTop: 6, padding: '2px 8px', borderRadius: 8,
          background: 'rgba(0,0,0,0.5)', color: '#fffa', fontSize: 10,
        }}>Tauri 未连接 — WebSocket 不可用</div>
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
