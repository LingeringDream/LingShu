/* global WebSocket */
import { useEffect, useRef, useState, useCallback } from 'react';
import { Application, Graphics, Text, Container, BlurFilter } from 'pixi.js';
import { isTauri, showMainWindow } from '../../lib/tauri';

const DRAG_THRESHOLD_PX = 4;

type Mood = 'idle' | 'thinking' | 'speaking' | 'happy' | 'sleepy';

const MOOD_COLORS: Record<Mood, number> = {
  idle: 0x6c9cff, thinking: 0x9b8cff, speaking: 0x6ce0a0,
  happy: 0xffc060, sleepy: 0x8899bb,
};

// ── Pet Character ──────────────────────────────────────────────────

class PetCharacter {
  container = new Container();
  private face = new Graphics();
  private leftEye = new Graphics();
  private rightEye = new Graphics();
  private mouth = new Graphics();
  private blushL = new Graphics();
  private blushR = new Graphics();
  private glow = new Graphics();
  private animTime = 0;
  private tx = 0; private ty = 0; // eye target
  private ex = 0; private ey = 0; // eye current
  private mood: Mood = 'idle';
  private sc = 1;
  private tsc = 1;

  constructor() {
    this.container.addChild(this.glow, this.face, this.blushL, this.blushR, this.leftEye, this.rightEye, this.mouth);
  }

  setMood(m: Mood) { this.mood = m; this.tsc = 1; }
  lookAt(x: number, y: number) { this.tx = ((x - 50) / 50) * 4; this.ty = ((y - 50) / 50) * 2; }
  bounce() { this.tsc = 1.2; }

  update(dt: number) {
    this.animTime += dt * 0.05;
    this.ex += (this.tx - this.ex) * 0.08;
    this.ey += (this.ty - this.ey) * 0.08;
    this.sc += (this.tsc - this.sc) * 0.12;
    if (Math.abs(this.tsc - this.sc) < 0.002) this.tsc = 1;

    const c = MOOD_COLORS[this.mood];
    const b = Math.sin(this.animTime) * 3;

    this.glow.clear();
    this.glow.circle(50, 50, 52);
    this.glow.fill({ color: c, alpha: 0.15 });
    this.glow.filters = [new BlurFilter({ strength: 16 })];

    this.face.clear();
    this.face.circle(50, 50 + b, 42);
    this.face.fill({ color: c, alpha: 0.88 });

    this.blushL.clear(); this.blushL.ellipse(32, 58 + b, 8, 4); this.blushL.fill({ color: 0xff8899, alpha: 0.25 });
    this.blushR.clear(); this.blushR.ellipse(68, 58 + b, 8, 4); this.blushR.fill({ color: 0xff8899, alpha: 0.25 });

    const es = this.mood === 'sleepy' ? 0.3 : this.mood === 'thinking' ? 0.5 : this.mood === 'speaking' ? 1.1 : this.mood === 'happy' ? 0.85 : 1;
    const lx = 36 + this.ex, ly = 44 + b + this.ey, rx = 64 + this.ex, ry = 44 + b + this.ey;

    this.leftEye.clear(); this.leftEye.ellipse(lx, ly, 8, 10 * es); this.leftEye.fill(0xffffff); this.leftEye.circle(lx + this.ex * 1.5, ly + this.ey * 1.5, 4); this.leftEye.fill(0x334466);
    this.rightEye.clear(); this.rightEye.ellipse(rx, ry, 8, 10 * es); this.rightEye.fill(0xffffff); this.rightEye.circle(rx + this.ex * 1.5, ry + this.ey * 1.5, 4); this.rightEye.fill(0x334466);

    this.mouth.clear();
    if (this.mood === 'thinking') { this.mouth.ellipse(50, 64 + b, 5, 3); this.mouth.fill({ color: 0x445566, alpha: 0.6 }); }
    else if (this.mood === 'speaking' || this.mood === 'happy') { this.mouth.ellipse(50, 62 + b, 7, 5); this.mouth.fill({ color: 0xff6688, alpha: 0.7 }); this.mouth.ellipse(50, 65 + b, 5, 3); this.mouth.fill({ color: 0x442233, alpha: 0.4 }); }
    else if (this.mood === 'sleepy') { this.mouth.ellipse(50, 66 + b, 4, 2); this.mouth.fill({ color: 0x445566, alpha: 0.4 }); }
    else { this.mouth.arc(50, 60 + b, 5, 0.1, Math.PI - 0.1); this.mouth.stroke({ color: 0x445566, alpha: 0.5, width: 1.5 }); }

    this.container.scale.set(this.sc);
    this.container.pivot.set(50, 50);
    this.container.position.set(50, 50);
  }
}

// ── Component ──────────────────────────────────────────────────────

export function PetWindow() {
  const canvasRef = useRef<HTMLDivElement>(null);
  const petRef = useRef<PetCharacter | null>(null);
  const appRef = useRef<Application | null>(null);
  const [bubble, setBubble] = useState<string | null>(null);
  const [inTauri, setInTauri] = useState(false);
  const draggedRef = useRef(false);

  // Pre-load the Tauri window handle so startDragging() can be called
  // synchronously inside the mousemove handler — macOS requires the drag to be
  // initiated within the live mouse event, so an async import there is too late.
  const petWindowRef = useRef<Awaited<ReturnType<typeof import('@tauri-apps/api/window')['getCurrentWindow']>> | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      petWindowRef.current = getCurrentWindow();
    }).catch(() => {});
    // Silently request accessibility permission for screen reading (L3)
    import('@tauri-apps/api/core').then(({ invoke }) => {
      invoke('request_accessibility_permission').catch(() => {});
    }).catch(() => {});
  }, []);

  // PixiJS lifecycle — cleanup is returned from useEffect (NOT from the async
  // IIFE), and an `initialized` flag guards against the StrictMode init/destroy
  // race (app.destroy() before app.init() resolves throws in PixiJS v8).
  useEffect(() => {
    setInTauri(isTauri());
    if (!canvasRef.current) return;
    const app = new Application();
    appRef.current = app;
    let disposed = false;
    let initialized = false;
    let timer: ReturnType<typeof setInterval> | null = null;

    (async () => {
      try {
        await app.init({ width: 200, height: 260, backgroundAlpha: 0.005, antialias: true, resolution: 2, autoDensity: true });
      } catch (err) {
        console.error('[pet] PixiJS init failed:', err);
        return;
      }
      if (disposed) { app.destroy(true); return; }
      const container = canvasRef.current;
      if (!container) { app.destroy(true); return; }
      initialized = true;
      container.appendChild(app.canvas);
      const pet = new PetCharacter();
      petRef.current = pet;
      pet.container.position.set(50, 80);
      app.stage.addChild(pet.container);
      const name = new Text({ text: '灵枢', style: { fontSize: 12, fontWeight: '600', fill: 0xffffff, fontFamily: 'system-ui, sans-serif', align: 'center' } });
      name.anchor.set(0.5, 0); name.position.set(100, 140);
      app.stage.addChild(name);
      app.ticker.add((t) => pet.update(t.deltaTime));
      const moods: Mood[] = ['idle', 'thinking', 'idle', 'speaking', 'idle', 'happy', 'sleepy'];
      let i = 0;
      timer = setInterval(() => { i = (i + 1) % moods.length; pet.setMood(moods[i]); }, 5000);
    })();

    return () => {
      disposed = true;
      if (timer) clearInterval(timer);
      if (initialized) { app.destroy(true, { children: true }); appRef.current = null; petRef.current = null; }
    };
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const r = canvasRef.current?.getBoundingClientRect();
    if (r) petRef.current?.lookAt((e.clientX - r.left) * (100 / r.width), (e.clientY - r.top) * (100 / r.height));
  }, []);

  const handleClick = useCallback(async () => {
    if (draggedRef.current) { draggedRef.current = false; return; }
    petRef.current?.bounce(); setBubble('我在呢！'); setTimeout(() => setBubble(null), 2000);
    if (inTauri) await showMainWindow();
  }, [inTauri]);

  const handleDoubleClick = useCallback(() => { if (!draggedRef.current) { petRef.current?.bounce(); setBubble('你好！'); setTimeout(() => setBubble(null), 3000); } }, []);

  // Drag — synchronous startDragging() via the pre-loaded window handle.
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0 || !inTauri) return;
    const sx = e.clientX, sy = e.clientY; draggedRef.current = false;
    const onMove = (ev: MouseEvent) => {
      if (draggedRef.current) return;
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) > DRAG_THRESHOLD_PX) {
        draggedRef.current = true; cleanup();
        if (petWindowRef.current) {
          petWindowRef.current.startDragging().catch(() => {});
        } else {
          import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().startDragging()).catch(() => {});
        }
      }
    };
    const cleanup = () => { window.removeEventListener('mousemove', onMove); window.removeEventListener('mouseup', cleanup); };
    window.addEventListener('mousemove', onMove); window.addEventListener('mouseup', cleanup);
  }, [inTauri]);

  // WebSocket
  useEffect(() => {
    if (!inTauri) return;
    const ws = new WebSocket('ws://127.0.0.1:8080/ws');
    ws.onmessage = (e) => { try { const m = JSON.parse(e.data); if (m.type === 'calendar' || m.type === 'thought') { setBubble((m.type === 'calendar' ? '📅 ' : '💡 ') + m.title); petRef.current?.bounce(); setTimeout(() => setBubble(null), 4000); } } catch { /* */ } };
    return () => ws.close();
  }, [inTauri]);

  return (
    <div onMouseDown={handleMouseDown} onMouseMove={handleMouseMove} onClick={handleClick} onDoubleClick={handleDoubleClick}
      style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', background: 'rgba(255,255,255,0.01)', userSelect: 'none', WebkitUserSelect: 'none', cursor: 'grab' }}>
      <div ref={canvasRef} />
      {bubble && <div style={{ marginTop: 2, padding: '4px 12px', borderRadius: 12, background: 'rgba(255,255,255,0.92)', color: '#333', fontSize: 12, maxWidth: 180, textAlign: 'center', boxShadow: '0 2px 12px rgba(0,0,0,0.12)', animation: 'fadeIn 0.25s ease', pointerEvents: 'none' }}>{bubble}</div>}
      {!inTauri && <div style={{ marginTop: 6, padding: '2px 8px', borderRadius: 8, background: 'rgba(0,0,0,0.5)', color: '#fffa', fontSize: 10 }}>Tauri 未连接</div>}
      <style>{`@keyframes fadeIn { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: translateY(0); } }`}</style>
    </div>
  );
}
