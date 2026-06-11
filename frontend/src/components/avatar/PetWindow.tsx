/* global WebSocket */
import { useEffect, useRef, useState } from 'react';
import { Application, Graphics, Text, Container, BlurFilter, Circle } from 'pixi.js';
import { isTauri, showMainWindow } from '../../lib/tauri';

const DRAG_THRESHOLD_PX = 2;

// Pet body center + interactive radius in window CSS coords (200×260).
const BODY_CX = 100;
const BODY_CY = 110;
const BODY_HIT_RADIUS = 64;

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
  private tx = 0; private ty = 0;
  private ex = 0; private ey = 0;
  private mood: Mood = 'idle';
  private sc = 1;
  private tsc = 1;

  constructor() {
    this.container.addChild(this.glow, this.face, this.blushL, this.blushR, this.leftEye, this.rightEye, this.mouth);
  }

  setMood(m: Mood) { this.mood = m; this.tsc = 1; }
  lookAt(x: number, y: number) { this.tx = ((x - 50) / 50) * 4; this.ty = ((y - 50) / 50) * 2; }
  bounce() { this.tsc = 1.2; }
  squish() { this.tsc = 0.85; }
  relax() { this.tsc = 1.15; }

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
    this.container.position.set(BODY_CX, BODY_CY);
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
  const bubbleRef = useRef<HTMLDivElement>(null);

  // Pre-load Tauri window handle for synchronous startDragging().
  const petWindowRef = useRef<Awaited<ReturnType<typeof import('@tauri-apps/api/window')['getCurrentWindow']>> | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      petWindowRef.current = getCurrentWindow();
    }).catch(() => {});
    import('@tauri-apps/api/core').then(({ invoke }) => {
      invoke('request_accessibility_permission', { prompt: false }).catch(() => {});
    }).catch(() => {});
  }, []);

  // PixiJS lifecycle — events bound to stage, not to a DOM wrapper.
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
        await app.init({ width: 200, height: 260, backgroundAlpha: 0, antialias: true, resolution: 2, autoDensity: true });
      } catch (err) {
        console.error('[pet] PixiJS init failed:', err);
        return;
      }
      if (disposed) { app.destroy(true); return; }
      const container = canvasRef.current;
      if (!container) { app.destroy(true); return; }
      initialized = true;
      container.appendChild(app.canvas);

      const stage = app.stage;
      stage.eventMode = 'static';
      stage.hitArea = new Circle(BODY_CX, BODY_CY, BODY_HIT_RADIUS);

      const pet = new PetCharacter();
      petRef.current = pet;
      pet.container.position.set(BODY_CX, BODY_CY);
      stage.addChild(pet.container);

      const name = new Text({ text: '灵枢', style: { fontSize: 12, fontWeight: '600', fill: 0xffffff, fontFamily: 'system-ui, sans-serif', align: 'center' } });
      name.anchor.set(0.5, 0); name.position.set(BODY_CX, BODY_CY + 58);
      stage.addChild(name);

      app.ticker.add((t) => pet.update(t.deltaTime));

      const moods: Mood[] = ['idle', 'thinking', 'idle', 'speaking', 'idle', 'happy', 'sleepy'];
      let i = 0;
      timer = setInterval(() => { i = (i + 1) % moods.length; pet.setMood(moods[i]); }, 5000);

      // ── Stage events (only the circular body area receives them) ──

      stage.on('pointermove', (e: { clientX: number; clientY: number }) => {
        const r = app.canvas.getBoundingClientRect();
        if (r.width > 0) pet.lookAt((e.clientX - r.left) * (100 / r.width), (e.clientY - r.top) * (100 / r.height));
      });

      stage.on('pointerdown', (e: { button: number; clientX: number; clientY: number }) => {
        if (e.button !== 0 || !isTauri()) return;
        const sx = e.clientX, sy = e.clientY; draggedRef.current = false;
        const onMove = (ev: MouseEvent) => {
          if (draggedRef.current) return;
          if (Math.hypot(ev.clientX - sx, ev.clientY - sy) > DRAG_THRESHOLD_PX) {
            draggedRef.current = true; cleanup();
            document.body.style.cursor = 'grabbing';
            pet.squish();
            const done = () => { document.body.style.cursor = ''; pet.relax(); };
            if (petWindowRef.current) {
              petWindowRef.current.startDragging().then(done).catch(done);
            } else {
              import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().startDragging()).then(done).catch(done);
            }
          }
        };
        const cleanup = () => { window.removeEventListener('mousemove', onMove); window.removeEventListener('mouseup', cleanup); };
        window.addEventListener('mousemove', onMove); window.addEventListener('mouseup', cleanup);
      });

      stage.on('pointerup', () => {
        if (draggedRef.current) { draggedRef.current = false; return; }
        pet.bounce(); setBubble('我在呢！'); setTimeout(() => setBubble(null), 2000);
        if (isTauri()) showMainWindow();
      });

      stage.on('pointerupoutside', () => { if (draggedRef.current) draggedRef.current = false; });

      stage.cursor = 'grab';
    })();

    return () => {
      disposed = true;
      if (timer) clearInterval(timer);
      if (initialized) { app.destroy(true, { children: true }); appRef.current = null; petRef.current = null; }
    };
  }, []);

  // WebSocket
  useEffect(() => {
    if (!inTauri) return;
    const ws = new WebSocket('ws://127.0.0.1:8080/ws');
    ws.onmessage = (e) => { try { const m = JSON.parse(e.data); if (m.type === 'calendar' || m.type === 'thought') { setBubble((m.type === 'calendar' ? '📅 ' : '💡 ') + m.title); petRef.current?.bounce(); setTimeout(() => setBubble(null), 4000); } } catch { /* */ } };
    return () => ws.close();
  }, [inTauri]);

  // Shaped click-through: poll cursor and toggle setIgnoreCursorEvents.
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    let timer: ReturnType<typeof setInterval> | null = null;
    let unlistenMoved: (() => void) | null = null;
    let win: Awaited<ReturnType<typeof import('@tauri-apps/api/window')['getCurrentWindow']>> | null = null;
    let ignoring = false;
    let originX = 0, originY = 0, scale = 1;

    (async () => {
      const { getCurrentWindow, cursorPosition } = await import('@tauri-apps/api/window');
      if (cancelled) return;
      win = getCurrentWindow();

      try { const p = await win.outerPosition(); originX = p.x; originY = p.y; } catch { /* */ }
      try { scale = await win.scaleFactor(); } catch { /* */ }
      if (cancelled) return;
      try { unlistenMoved = await win.onMoved(({ payload }) => { originX = payload.x; originY = payload.y; }); } catch { /* */ }

      timer = setInterval(async () => {
        const w = win;
        if (cancelled || !w) return;
        try {
          const c = await cursorPosition();
          const rx = (c.x - originX) / scale;
          const ry = (c.y - originY) / scale;
          const inside = Math.hypot(rx - BODY_CX, ry - BODY_CY) <= BODY_HIT_RADIUS;
          if (inside === ignoring) {
            ignoring = !inside;
            await w.setIgnoreCursorEvents(ignoring);
          }
        } catch { /* leave interactive on error */ }
      }, 33);
    })();

    return () => {
      cancelled = true;
      if (timer) clearInterval(timer);
      if (unlistenMoved) unlistenMoved();
      win?.setIgnoreCursorEvents(false).catch(() => {});
    };
  }, []);

  return (
    <>
      <div ref={canvasRef}
        style={{ position: 'fixed', inset: 0, width: '100vw', height: '100vh', pointerEvents: 'none' }} />
      {bubble && <div ref={bubbleRef}
        style={{ position: 'fixed', left: BODY_CX, top: BODY_CY + 68, transform: 'translateX(-50%)',
          padding: '4px 12px', borderRadius: 12, background: 'rgba(255,255,255,0.92)', color: '#333',
          fontSize: 12, maxWidth: 180, textAlign: 'center', boxShadow: '0 2px 12px rgba(0,0,0,0.12)',
          pointerEvents: 'none', animation: 'fadeIn 0.25s ease', zIndex: 1 }}>{bubble}</div>}
      {!inTauri && <div style={{ position: 'fixed', bottom: 8, left: '50%', transform: 'translateX(-50%)',
        padding: '2px 8px', borderRadius: 8, background: 'rgba(0,0,0,0.5)', color: '#fffa', fontSize: 10,
        pointerEvents: 'none' }}>Tauri 未连接</div>}
      <style>{`@keyframes fadeIn { from { opacity: 0; transform: translateX(-50%) translateY(4px); } to { opacity: 1; transform: translateX(-50%) translateY(0); } }`}</style>
    </>
  );
}
