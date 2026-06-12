/* global WebSocket */
import { useEffect, useRef, useState, useCallback } from 'react';
import { Application, Graphics, Text, Container, BlurFilter } from 'pixi.js';
import { isTauri, showMainWindow } from '../../lib/tauri';
import { getMoodPresentation, getReplyDisplayTarget, lerpPresentation, traitsToModifiers, defaultModifiers, type Mood, type EyeShape, type MoodPresentation, type PersonalityTraits, type PersonalityModifiers } from './petPresentation';

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

interface Particle { x: number; y: number; vx: number; vy: number; life: number; maxLife: number }

class PetCharacter {
  container = new Container();
  private shell = new Graphics();
  private ring = new Graphics();
  private core = new Graphics();
  private star = new Graphics();
  private orbitBack = new Graphics();
  private orbitFront = new Graphics();
  private innerOrbitLayer = new Graphics();
  private sensor = new Graphics();
  private wake = new Graphics();
  private glow = new Graphics();
  private particleLayer = new Graphics();

  private animTime = 0;
  private tx = 0; private ty = 0;
  private ex = 0; private ey = 0;
  private mood: Mood = 'idle';
  private sc = 1;
  private tsc = 1;

  // Live (eased) presentation + integrated orbit angle. Mood switches glide
  // toward the new preset instead of jumping; the orbit angle is integrated
  // incrementally because `animTime * speed` would re-scale the accumulated
  // angle whenever the eased speed changes, spinning the ring wildly.
  private visual: MoodPresentation = { ...getMoodPresentation('idle') };
  private orbitAngle = 0;

  // Blink
  private blinkCooldown = 180 + Math.random() * 200;
  private blinkPhase = 0;
  private blinking = false;

  // Idle look-around (triggers when mouse has been still)
  private noMouseFrames = 0;
  private idleLookCooldown = 0;

  // thinking scan
  private scanPhase = 0;

  private particles: Particle[] = [];
  private mods: PersonalityModifiers = defaultModifiers();

  constructor() {
    this.container.addChild(
      this.glow,
      this.orbitBack,
      this.innerOrbitLayer,
      this.shell,
      this.ring,
      this.core,
      this.star,
      this.sensor,
      this.wake,
      this.orbitFront,
      this.particleLayer,
    );
  }

  setMood(m: Mood) {
    if (m !== this.mood) this.scanPhase = 0;
    this.mood = m;
    this.tsc = 1;
  }

  lookAt(x: number, y: number) {
    this.tx = ((x - 50) / 50) * 4;
    this.ty = ((y - 50) / 50) * 2;
    this.noMouseFrames = 0;
  }

  bounce() { this.tsc = 1 + this.mods.bounceMagnitude * 0.2; }
  applyPersonality(traits: PersonalityTraits) { this.mods = traitsToModifiers(traits); }
  squish() { this.tsc = 0.85; }
  relax() { this.tsc = 1.15; }

  burst() {
    for (let i = 0; i < 10; i++) {
      const angle = (i / 10) * Math.PI * 2 + Math.random() * 0.62;
      const speed = 1.5 + Math.random() * 2;
      this.particles.push({
        x: 50, y: 50,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed - 1,
        life: 30 + Math.random() * 15,
        maxLife: 45,
      });
    }
  }

  update(dt: number) {
    this.animTime += dt * 0.05;

    // Idle look-around
    this.noMouseFrames += dt;
    if (this.noMouseFrames > 90 && this.idleLookCooldown <= 0 && this.mood !== 'thinking') {
      this.tx = (Math.random() - 0.5) * 6;
      this.ty = (Math.random() - 0.5) * 3;
      this.idleLookCooldown = this.mods.idleLookFreq + Math.random() * 100;
    }
    if (this.idleLookCooldown > 0) this.idleLookCooldown -= dt;

    this.ex += (this.tx - this.ex) * 0.08;
    this.ey += (this.ty - this.ey) * 0.08;
    this.sc += (this.tsc - this.sc) * 0.12;
    if (Math.abs(this.tsc - this.sc) < 0.002) this.tsc = 1;

    // Blink
    if (this.blinking) {
      this.blinkPhase = Math.min(this.blinkPhase + dt * 0.12, 1);
      if (this.blinkPhase >= 1) {
        this.blinking = false;
        this.blinkPhase = 0;
        this.blinkCooldown = this.mods.blinkInterval + Math.random() * (this.mods.blinkInterval * 0.8);
      }
    } else {
      this.blinkCooldown -= dt;
      if (this.blinkCooldown <= 0) { this.blinking = true; this.blinkPhase = 0; }
    }
    const blinkScale = this.blinking ? Math.abs(Math.cos(this.blinkPhase * Math.PI)) : 1;

    // thinking: eye scans left-right
    if (this.mood === 'thinking') {
      this.scanPhase += dt * 0.04;
      this.tx = Math.sin(this.scanPhase) * 3.5;
      this.ty = Math.sin(this.scanPhase * 0.7) * 1.5;
    }

    this.visual = lerpPresentation(this.visual, getMoodPresentation(this.mood), Math.min(0.08 * dt, 1));
    const visual = this.visual;
    const c = visual.color;
    const b = Math.sin(this.animTime) * 3;
    const pulse = 1 + Math.sin(this.animTime * 1.4) * 0.025 * visual.pulse * this.mods.pulseMult;
    this.orbitAngle += dt * 0.05 * visual.orbitSpeed * this.mods.orbitSpeedMult;

    this.glow.clear();
    this.glow.circle(50, 50 + b, 54 * pulse);
    this.glow.fill({ color: visual.glowColor, alpha: 0.24 });
    this.glow.filters = [new BlurFilter({ strength: 18 })];

    this.orbitBack.clear();
    this.drawOrbit(this.orbitBack, 50, 50 + b, 56, 15, this.orbitAngle, c, 0.22, 1.5);

    // Inner orbiting dots (depth cue via alpha)
    this.innerOrbitLayer.clear();
    for (let d = 0; d < 2; d++) {
      const phase = this.orbitAngle * 1.8 + d * Math.PI;
      const dotX = 50 + Math.cos(phase) * 20;
      const dotY = 50 + b + Math.sin(phase) * 6;
      const dotAlpha = (Math.sin(phase) * 0.5 + 0.5) * 0.18 + 0.1;
      this.innerOrbitLayer.circle(dotX, dotY, 2);
      this.innerOrbitLayer.fill({ color: 0xffffff, alpha: dotAlpha });
    }

    this.shell.clear();
    this.shell.circle(50, 50 + b, 38 * pulse);
    this.shell.fill({ color: c, alpha: 0.3 });
    this.shell.circle(50, 50 + b, 34 * pulse);
    this.shell.stroke({ color: 0xffffff, alpha: 0.38, width: 1.5 });

    this.ring.clear();
    this.ring.circle(50, 50 + b, 29 + Math.sin(this.animTime * 1.1) * 1.5);
    this.ring.stroke({ color: c, alpha: 0.42, width: 2 });

    this.core.clear();
    this.core.circle(50, 50 + b, 21 + Math.sin(this.animTime * 1.8) * 1.1);
    this.core.fill({ color: 0xffffff, alpha: 0.28 });
    this.core.circle(50, 50 + b, 13 + Math.sin(this.animTime * 2.2) * 0.8);
    this.core.fill({ color: visual.glowColor, alpha: 0.34 });

    this.star.clear();
    this.drawStar(this.star, 50, 50 + b, this.mood === 'thinking' ? 17 : 14, 5, 0xffffff, 0.8);

    this.sensor.clear();
    const sensorX = 50 + this.ex * 1.6;
    const sensorY = 50 + b + this.ey * 1.4;
    this.drawEye(sensorX, sensorY, visual.eyeShape, blinkScale);

    this.wake.clear();
    if (this.mood === 'speaking' || this.mood === 'happy') {
      this.wake.arc(50, 50 + b, 44, -0.45, 0.45);
      this.wake.stroke({ color: 0xffffff, alpha: 0.28, width: 2 });
      this.wake.arc(50, 50 + b, 47, Math.PI - 0.35, Math.PI + 0.35);
      this.wake.stroke({ color: 0xffffff, alpha: 0.18, width: 2 });
    }

    this.orbitFront.clear();
    this.drawOrbit(this.orbitFront, 50, 50 + b, 56, 15, this.orbitAngle + Math.PI, c, 0.52, 2.2);

    // Particles
    this.particleLayer.clear();
    for (let i = this.particles.length - 1; i >= 0; i--) {
      const p = this.particles[i];
      p.x += p.vx * dt; p.y += p.vy * dt; p.vy += 0.05 * dt; p.life -= dt;
      if (p.life <= 0) { this.particles.splice(i, 1); continue; }
      const a = (p.life / p.maxLife) * 0.8;
      const s = (p.life / p.maxLife) * 3.5 + 0.5;
      this.particleLayer.circle(p.x, p.y, s);
      this.particleLayer.fill({ color: visual.glowColor, alpha: a });
    }

    this.container.scale.set(this.sc);
    this.container.pivot.set(50, 50);
    this.container.position.set(BODY_CX, BODY_CY);
  }

  private drawEye(x: number, y: number, shape: EyeShape, blinkScale: number) {
    switch (shape) {
      case 'open': {
        const r = 4.5 * blinkScale;
        if (r > 0.2) { this.sensor.circle(x, y, r); this.sensor.fill({ color: 0xffffff, alpha: 0.9 }); }
        this.sensor.circle(x, y, 8 + Math.sin(this.animTime * 2.8) * 1.5);
        this.sensor.stroke({ color: 0xffffff, alpha: 0.24, width: 1 });
        break;
      }
      case 'focused': {
        // smaller bright dot + crosshair
        const r = 3.5 * blinkScale;
        if (r > 0.2) { this.sensor.circle(x, y, r); this.sensor.fill({ color: 0xffffff, alpha: 0.95 }); }
        this.sensor.moveTo(x - 7, y); this.sensor.lineTo(x + 7, y);
        this.sensor.stroke({ color: 0xffffff, alpha: 0.22, width: 0.8 });
        this.sensor.moveTo(x, y - 6); this.sensor.lineTo(x, y + 6);
        this.sensor.stroke({ color: 0xffffff, alpha: 0.22, width: 0.8 });
        break;
      }
      case 'smiling': {
        // arc squint (happy)
        const hw = 6 * blinkScale;
        if (hw > 0.5) {
          this.sensor.arc(x, y + 1, hw, Math.PI, 0, false);
          this.sensor.stroke({ color: 0xffffff, alpha: 0.9, width: 2.5 });
        }
        break;
      }
      case 'sleepy': {
        // small dim dot + droop arc (r=3 per design doc §3.3)
        const r = 3 * blinkScale;
        if (r > 0.2) { this.sensor.circle(x, y, r); this.sensor.fill({ color: 0xffffff, alpha: 0.45 }); }
        this.sensor.arc(x, y + 2, 5.5, 0.3, Math.PI - 0.3);
        this.sensor.stroke({ color: 0xffffff, alpha: 0.22, width: 1 });
        break;
      }
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

  private drawOrbit(target: Graphics, x: number, y: number, rx: number, ry: number, rotation: number, color: number, alpha: number, width: number) {
    target.ellipse(x, y, rx, ry);
    target.rotation = rotation;
    target.pivot.set(x, y);
    target.position.set(x, y);
    target.stroke({ color, alpha, width });
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
      name.style.fill = 0x2e6bff;
      name.anchor.set(0.5, 0); name.position.set(BODY_CX, BODY_CY + 58);
      app.stage.addChild(name);
      app.ticker.add((t) => pet.update(t.deltaTime));
      const moods: Mood[] = ['idle', 'thinking', 'idle', 'speaking', 'idle', 'happy', 'sleepy'];
      let i = 0;
      if (!isTauri()) timer = setInterval(() => { i = (i + 1) % moods.length; pet.setMood(moods[i]); }, 5000);
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
    petRef.current?.burst();
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

  // Press feedback + drag — squish() fires on body mousedown (design doc §6),
  // and only body presses arm the window drag, so text selection inside the
  // dialog input is not hijacked. startDragging() is called synchronously via
  // the pre-loaded window handle.
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const inBody = Math.hypot(e.clientX - rect.left - BODY_CX, e.clientY - rect.top - BODY_CY) <= BODY_HIT_RADIUS;
    if (!inBody) return;
    petRef.current?.squish();
    if (!inTauri) return;
    const sx = e.clientX, sy = e.clientY; draggedRef.current = false;
    const onMove = (ev: MouseEvent) => {
      if (draggedRef.current) return;
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) > DRAG_THRESHOLD_PX) {
        draggedRef.current = true; cleanup();
        document.body.style.cursor = 'grabbing';
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
    ws.onmessage = (e) => {
      try {
        const m = JSON.parse(e.data);
        if (m.type === 'mood') {
          petRef.current?.setMood(m.title as Mood);
          if (m.data) petRef.current?.applyPersonality(m.data as PersonalityTraits);
        } else if (m.type === 'calendar' || m.type === 'thought') {
          const reply = (m.type === 'calendar' ? '日程：' : '想法：') + m.title;
          setDialogReply(reply);
          if (getReplyDisplayTarget(reply) === 'bubble') setBubble(reply); else setDialogOpen(true);
          petRef.current?.bounce();
          setTimeout(() => setBubble(null), 4000);
        }
      } catch { /* */ }
    };
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
      {!inTauri && !dialogOpen && <div style={{ position: 'absolute', left: '50%', top: 'calc(50% + 112px)', transform: 'translateX(-50%)', padding: '2px 8px', borderRadius: 8, background: 'rgba(0,0,0,0.5)', color: '#fffa', fontSize: 10 }}>Tauri 未连接</div>}
      <style>{`@keyframes fadeIn { from { opacity: 0; transform: translateX(-50%) translateY(4px); } to { opacity: 1; transform: translateX(-50%) translateY(0); } }`}</style>
    </div>
  );
}
