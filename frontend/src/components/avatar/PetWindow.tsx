/* global WebSocket */
import { useEffect, useRef, useState, useCallback } from 'react';
import { Application, Graphics, Text, Container, BlurFilter } from 'pixi.js';
import { isTauri, showMainWindow } from '../../lib/tauri';
import { getMoodPresentation, getReplyDisplayTarget, type Mood } from './petPresentation';

const DRAG_THRESHOLD_PX = 2;

// Pet body position + interactive radius, in window CSS coords (the window is
// 200×260). The PixiJS character is drawn centered on (BODY_CX, BODY_CY); the
// shaped click-through hit-test uses the same circle so only the body grabs the
// mouse and the transparent rest of the window passes clicks through.
const BODY_CX = 100;
const BODY_CY = 110;
const BODY_HIT_RADIUS = 64;
const DIALOG_HIT_RECT = { x: 6, y: 134, width: 188, height: 108 };

// ── Pet Character ──────────────────────────────────────────────────

class PetCharacter {
  container = new Container();
  private face = new Graphics();
  private core = new Graphics();
  private star = new Graphics();
  private orbitBack = new Graphics();
  private orbitFront = new Graphics();
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
    this.container.addChild(
      this.glow,
      this.orbitBack,
      this.face,
      this.core,
      this.star,
      this.blushL,
      this.blushR,
      this.leftEye,
      this.rightEye,
      this.mouth,
      this.orbitFront,
    );
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

    const visual = getMoodPresentation(this.mood);
    const c = visual.color;
    const b = Math.sin(this.animTime) * 3;
    const pulse = 1 + Math.sin(this.animTime * 1.4) * 0.025 * visual.pulse;
    const orbitRotation = this.animTime * visual.orbitSpeed;

    this.glow.clear();
    this.glow.circle(50, 50 + b, 54 * pulse);
    this.glow.fill({ color: visual.glowColor, alpha: 0.18 });
    this.glow.filters = [new BlurFilter({ strength: 18 })];

    this.orbitBack.clear();
    this.drawOrbit(this.orbitBack, 50, 50 + b, 52, 16, orbitRotation, c, 0.28, true);

    this.face.clear();
    this.face.circle(50, 50 + b, 41 * pulse);
    this.face.fill({ color: c, alpha: 0.84 });

    this.core.clear();
    this.core.circle(50, 50 + b, 24 + Math.sin(this.animTime * 1.8) * 1.2);
    this.core.fill({ color: 0xffffff, alpha: 0.14 });

    this.star.clear();
    this.drawStar(this.star, 50, 50 + b, this.mood === 'thinking' ? 16 : 13, 6, 0xffffff, 0.68);

    this.blushL.clear(); this.blushL.ellipse(32, 58 + b, 8, 4); this.blushL.fill({ color: 0xff8899, alpha: 0.25 });
    this.blushR.clear(); this.blushR.ellipse(68, 58 + b, 8, 4); this.blushR.fill({ color: 0xff8899, alpha: 0.25 });

    const es = visual.eyeShape === 'sleepy' ? 0.28 : visual.eyeShape === 'focused' ? 0.58 : visual.eyeShape === 'smiling' ? 0.72 : 1;
    const lx = 36 + this.ex, ly = 44 + b + this.ey, rx = 64 + this.ex, ry = 44 + b + this.ey;

    this.leftEye.clear(); this.drawEye(this.leftEye, lx, ly, es);
    this.rightEye.clear(); this.drawEye(this.rightEye, rx, ry, es);

    this.mouth.clear();
    if (this.mood === 'thinking') { this.mouth.ellipse(50, 64 + b, 4, 3); this.mouth.fill({ color: 0x334466, alpha: 0.6 }); }
    else if (this.mood === 'speaking') { this.mouth.ellipse(50, 62 + b, 7, 5 + Math.sin(this.animTime * 7) * 1.8); this.mouth.fill({ color: 0xff6688, alpha: 0.7 }); this.mouth.ellipse(50, 65 + b, 5, 3); this.mouth.fill({ color: 0x442233, alpha: 0.35 }); }
    else if (this.mood === 'happy') { this.mouth.arc(50, 59 + b, 9, 0.05, Math.PI - 0.05); this.mouth.stroke({ color: 0x445566, alpha: 0.55, width: 2 }); }
    else if (this.mood === 'sleepy') { this.mouth.ellipse(50, 66 + b, 4, 2); this.mouth.fill({ color: 0x445566, alpha: 0.4 }); }
    else { this.mouth.arc(50, 60 + b, 5, 0.1, Math.PI - 0.1); this.mouth.stroke({ color: 0x445566, alpha: 0.5, width: 1.5 }); }

    this.orbitFront.clear();
    this.drawOrbit(this.orbitFront, 50, 50 + b, 52, 16, orbitRotation + Math.PI, c, 0.55, false);

    this.container.scale.set(this.sc);
    this.container.pivot.set(50, 50);
    this.container.position.set(BODY_CX, BODY_CY);
  }

  private drawEye(target: Graphics, x: number, y: number, scaleY: number) {
    target.ellipse(x, y, 7.5, 9.5 * scaleY);
    target.fill(0xffffff);
    if (scaleY > 0.35) {
      target.circle(x + this.ex * 1.3, y + this.ey * 1.2, 3.7);
      target.fill(0x24344f);
      target.circle(x + 1.5 + this.ex * 1.1, y - 2 + this.ey, 1.2);
      target.fill({ color: 0xffffff, alpha: 0.7 });
    }
  }

  private drawStar(target: Graphics, x: number, y: number, outer: number, inner: number, color: number, alpha: number) {
    target.moveTo(x, y - outer);
    target.lineTo(x + inner, y - inner);
    target.lineTo(x + outer, y);
    target.lineTo(x + inner, y + inner);
    target.lineTo(x, y + outer);
    target.lineTo(x - inner, y + inner);
    target.lineTo(x - outer, y);
    target.lineTo(x - inner, y - inner);
    target.closePath();
    target.fill({ color, alpha });
  }

  private drawOrbit(target: Graphics, x: number, y: number, rx: number, ry: number, rotation: number, color: number, alpha: number, backHalf: boolean) {
    target.ellipse(x, y, rx, ry);
    target.rotation = rotation;
    target.pivot.set(x, y);
    target.position.set(x, y);
    target.stroke({ color, alpha, width: backHalf ? 2 : 2.5 });
  }
}

// ── Component ──────────────────────────────────────────────────────

export function PetWindow() {
  const canvasRef = useRef<HTMLDivElement>(null);
  const petRef = useRef<PetCharacter | null>(null);
  const appRef = useRef<Application | null>(null);
  const [bubble, setBubble] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [draft, setDraft] = useState('');
  const [dialogReply, setDialogReply] = useState('点击灵枢后，我会在这里保留较长回复；短提示会从旁边气泡出现。');
  const [inTauri, setInTauri] = useState(false);
  const draggedRef = useRef(false);
  const dialogOpenRef = useRef(false);

  useEffect(() => { dialogOpenRef.current = dialogOpen; }, [dialogOpen]);

  // Pre-load the Tauri window handle so startDragging() can be called
  // synchronously inside the mousemove handler — macOS requires the drag to be
  // initiated within the live mouse event, so an async import there is too late.
  const petWindowRef = useRef<Awaited<ReturnType<typeof import('@tauri-apps/api/window')['getCurrentWindow']>> | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      petWindowRef.current = getCurrentWindow();
    }).catch(() => {});
    // Silent accessibility status check (no system dialog at startup — the
    // real prompt fires from the chat's one-click grant button at point of
    // need, via request_accessibility_permission with prompt: true).
    import('@tauri-apps/api/core').then(({ invoke }) => {
      invoke('request_accessibility_permission', { prompt: false }).catch(() => {});
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
      const pet = new PetCharacter();
      petRef.current = pet;
      pet.container.position.set(BODY_CX, BODY_CY);
      app.stage.addChild(pet.container);
      const name = new Text({ text: '灵枢', style: { fontSize: 12, fontWeight: '600', fill: 0xffffff, fontFamily: 'system-ui, sans-serif', align: 'center' } });
      name.anchor.set(0.5, 0); name.position.set(BODY_CX, BODY_CY + 58);
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

  const handleClick = useCallback(() => {
    if (draggedRef.current) { draggedRef.current = false; return; }
    petRef.current?.bounce();
    setDialogOpen((open) => !open);
    if (!dialogOpenRef.current) setBubble(null);
  }, []);

  const handleDoubleClick = useCallback(async () => {
    if (!draggedRef.current) {
      petRef.current?.bounce();
      setDialogOpen(true);
      if (inTauri) await showMainWindow();
    }
  }, [inTauri]);

  const submitMiniPrompt = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    const text = draft.trim();
    if (!text) return;
    const reply = `收到：${text}`;
    setDraft('');
    setDialogReply(reply);
    petRef.current?.setMood('speaking');
    if (getReplyDisplayTarget(reply) === 'bubble') {
      setBubble(reply);
      setTimeout(() => setBubble(null), 3500);
    }
    setTimeout(() => petRef.current?.setMood('idle'), 1800);
  }, [draft]);

  // Drag — synchronous startDragging() via the pre-loaded window handle.
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0 || !inTauri) return;
    const sx = e.clientX, sy = e.clientY; draggedRef.current = false;
    const onMove = (ev: MouseEvent) => {
      if (draggedRef.current) return;
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) > DRAG_THRESHOLD_PX) {
        draggedRef.current = true; cleanup();
        document.body.style.cursor = 'grabbing';
        petRef.current?.squish();
        const done = () => { document.body.style.cursor = ''; petRef.current?.relax(); };
        if (petWindowRef.current) {
          petWindowRef.current.startDragging().then(done).catch(done);
        } else {
          import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().startDragging()).then(done).catch(done);
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
    ws.onmessage = (e) => { try { const m = JSON.parse(e.data); if (m.type === 'calendar' || m.type === 'thought') { const reply = (m.type === 'calendar' ? '日程：' : '想法：') + m.title; setDialogReply(reply); if (getReplyDisplayTarget(reply) === 'bubble') setBubble(reply); else setDialogOpen(true); petRef.current?.bounce(); setTimeout(() => setBubble(null), 4000); } } catch { /* */ } };
    return () => ws.close();
  }, [inTauri]);

  // Shaped click-through: only the pet body captures the mouse; the transparent
  // rest of the window passes clicks through to whatever is behind it. We poll
  // the GLOBAL cursor (it works even while the window ignores events — which is
  // exactly when the DOM receives no mousemove) and toggle setIgnoreCursorEvents
  // on body enter/leave. Any failure leaves the window interactive so the pet
  // stays draggable.
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    let timer: ReturnType<typeof setInterval> | null = null;
    let unlistenMoved: (() => void) | null = null;
    let win: Awaited<ReturnType<typeof import('@tauri-apps/api/window')['getCurrentWindow']>> | null = null;
    let ignoring = false;            // current OS ignore-cursor state
    let originX = 0, originY = 0, scale = 1; // window top-left (physical) + scale

    (async () => {
      const { getCurrentWindow, cursorPosition } = await import('@tauri-apps/api/window');
      if (cancelled) return;
      win = getCurrentWindow();

      try { const p = await win.outerPosition(); originX = p.x; originY = p.y; } catch { /* */ }
      try { scale = await win.scaleFactor(); } catch { /* */ }
      if (cancelled) return;
      // The window only moves while dragging the body (interactive); keep the
      // cached origin fresh so the hit-test stays accurate after a move.
      try { unlistenMoved = await win.onMoved(({ payload }) => { originX = payload.x; originY = payload.y; }); } catch { /* */ }

      timer = setInterval(async () => {
        const w = win;
        if (cancelled || !w) return;
        try {
          const c = await cursorPosition();              // global physical px
          const rx = (c.x - originX) / scale;            // → window CSS px
          const ry = (c.y - originY) / scale;
          const insideBody = Math.hypot(rx - BODY_CX, ry - BODY_CY) <= BODY_HIT_RADIUS;
          const insideDialog = dialogOpenRef.current
            && rx >= DIALOG_HIT_RECT.x
            && rx <= DIALOG_HIT_RECT.x + DIALOG_HIT_RECT.width
            && ry >= DIALOG_HIT_RECT.y
            && ry <= DIALOG_HIT_RECT.y + DIALOG_HIT_RECT.height;
          const inside = insideBody || insideDialog;
          if (inside === ignoring) {                      // state needs to flip
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
    <div onMouseDown={handleMouseDown} onMouseMove={handleMouseMove} onClick={handleClick} onDoubleClick={handleDoubleClick}
      style={{ width: '100vw', height: '100vh', position: 'relative', background: 'transparent', userSelect: 'none', WebkitUserSelect: 'none', cursor: 'grab', overflow: 'hidden' }}>
      <div ref={canvasRef} style={{ position: 'absolute', left: '50%', top: '50%', width: 200, height: 260, transform: 'translate(-50%, -50%)' }} />
      {bubble && <div style={{ position: 'absolute', left: '50%', top: dialogOpen ? 'calc(50% - 42px)' : 'calc(50% - 6px)', transform: 'translateX(-50%)', zIndex: 2, padding: '5px 12px', borderRadius: 12, background: 'rgba(255,255,255,0.94)', color: '#24344f', fontSize: 12, lineHeight: 1.35, maxWidth: 180, textAlign: 'center', boxShadow: '0 8px 24px rgba(21,43,92,0.18)', border: '1px solid rgba(46,107,255,0.16)', animation: 'fadeIn 0.25s ease', pointerEvents: 'none' }}>{bubble}</div>}
      {dialogOpen && (
        <form onSubmit={submitMiniPrompt} style={{ position: 'absolute', left: '50%', top: 'calc(50% + 4px)', transform: 'translateX(-50%)', zIndex: 1, width: 172, padding: 8, borderRadius: 12, background: 'rgba(255,255,255,0.95)', color: '#1f2a44', boxShadow: '0 12px 36px rgba(21,43,92,0.22)', border: '1px solid rgba(46,107,255,0.18)', backdropFilter: 'blur(12px)', animation: 'fadeIn 0.2s ease', cursor: 'default' }} onClick={(e) => e.stopPropagation()}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
            <strong style={{ fontSize: 12, fontWeight: 700 }}>灵枢</strong>
            <button type="button" aria-label="关闭对话框" onClick={() => setDialogOpen(false)} style={{ width: 18, height: 18, border: 0, borderRadius: 9, background: 'rgba(46,107,255,0.1)', color: '#2e6bff', fontSize: 12, lineHeight: '18px', padding: 0, cursor: 'pointer' }}>×</button>
          </div>
          <div style={{ minHeight: 30, maxHeight: 44, overflow: 'hidden', fontSize: 11, lineHeight: 1.35, color: '#40516f', marginBottom: 7 }}>{dialogReply}</div>
          <div style={{ display: 'flex', gap: 5 }}>
            <input value={draft} onChange={(e) => setDraft(e.target.value)} placeholder="和灵枢说一句..." style={{ flex: 1, minWidth: 0, height: 24, borderRadius: 8, border: '1px solid rgba(46,107,255,0.2)', padding: '0 7px', fontSize: 11, outline: 'none', color: '#1f2a44' }} />
            <button type="submit" aria-label="发送" style={{ width: 32, height: 24, border: 0, borderRadius: 8, background: '#2e6bff', color: '#fff', fontSize: 11, fontWeight: 700, cursor: 'pointer' }}>发</button>
          </div>
        </form>
      )}
      {!inTauri && !dialogOpen && <div style={{ position: 'absolute', left: '50%', top: 'calc(50% + 122px)', transform: 'translateX(-50%)', padding: '2px 8px', borderRadius: 8, background: 'rgba(0,0,0,0.5)', color: '#fffa', fontSize: 10 }}>Tauri 未连接</div>}
      <style>{`@keyframes fadeIn { from { opacity: 0; transform: translateX(-50%) translateY(4px); } to { opacity: 1; transform: translateX(-50%) translateY(0); } }`}</style>
    </div>
  );
}
