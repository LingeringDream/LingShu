export type Mood = 'idle' | 'thinking' | 'speaking' | 'happy' | 'sleepy';

export type EyeShape = 'open' | 'focused' | 'smiling' | 'sleepy';

export type MoodPresentation = {
  color: number;
  glowColor: number;
  orbitSpeed: number;
  pulse: number;
  eyeShape: EyeShape;
};

const MOOD_PRESENTATION: Record<Mood, MoodPresentation> = {
  idle: {
    color: 0x2e6bff,
    glowColor: 0x22a8ff,
    orbitSpeed: 0.45,
    pulse: 1,
    eyeShape: 'open',
  },
  thinking: {
    color: 0x9b8cff,
    glowColor: 0x5848f5,
    orbitSpeed: 1.15,
    pulse: 0.9,
    eyeShape: 'focused',
  },
  speaking: {
    color: 0x6ce0a0,
    glowColor: 0x22d3a6,
    orbitSpeed: 0.85,
    pulse: 1.2,
    eyeShape: 'open',
  },
  happy: {
    color: 0xffc060,
    glowColor: 0xffd48a,
    orbitSpeed: 1.35,
    pulse: 1.25,
    eyeShape: 'smiling',
  },
  sleepy: {
    color: 0x8899bb,
    glowColor: 0x6b7fa3,
    orbitSpeed: 0.25,
    pulse: 0.75,
    eyeShape: 'sleepy',
  },
};

export function getMoodPresentation(mood: Mood): MoodPresentation {
  return MOOD_PRESENTATION[mood];
}

// ── Mood transition (lerp) ────────────────────────────────────────────────

function lerp(from: number, to: number, t: number): number {
  return from + (to - from) * t;
}

// Per-channel RGB interpolation — a plain numeric lerp on 0xRRGGBB values
// would bleed between channels.
export function lerpColor(from: number, to: number, t: number): number {
  const r = Math.round(lerp((from >> 16) & 0xff, (to >> 16) & 0xff, t));
  const g = Math.round(lerp((from >> 8) & 0xff, (to >> 8) & 0xff, t));
  const b = Math.round(lerp(from & 0xff, to & 0xff, t));
  return (r << 16) | (g << 8) | b;
}

// Ease the live presentation toward the target mood each frame. Numeric
// fields glide; eyeShape is discrete and switches immediately.
export function lerpPresentation(
  current: MoodPresentation,
  target: MoodPresentation,
  t: number,
): MoodPresentation {
  return {
    color: lerpColor(current.color, target.color, t),
    glowColor: lerpColor(current.glowColor, target.glowColor, t),
    orbitSpeed: lerp(current.orbitSpeed, target.orbitSpeed, t),
    pulse: lerp(current.pulse, target.pulse, t),
    eyeShape: target.eyeShape,
  };
}

// ── Personality-driven animation modifiers ────────────────────────────────
// Mirrors the server-side PersonalityValues struct (7 f32 traits, 0–1).
// Only the subset that drives visual behaviour is declared here.
export interface PersonalityTraits {
  directness: number;
  warmth: number;
  proactivity: number;
  risk_tolerance: number;
  verbosity: number;
  formality: number;
  humor: number;
  has_role_prompt?: boolean;
}

// Behavioral animation multipliers derived from personality.
// All values are *multipliers* applied on top of the base mood parameters.
export interface PersonalityModifiers {
  orbitSpeedMult: number;   // scales orbitSpeed from MoodPresentation
  pulseMult: number;        // scales pulse coefficient
  blinkInterval: number;    // base frames between blinks (lower = more frequent)
  idleLookFreq: number;     // frames between idle look-around events
  bounceMagnitude: number;  // scale factor for bounce tsc overshoot
}

export function defaultModifiers(): PersonalityModifiers {
  return {
    orbitSpeedMult: 1,
    pulseMult: 1,
    blinkInterval: 220,
    idleLookFreq: 300,
    bounceMagnitude: 1,
  };
}

export function traitsToModifiers(t: PersonalityTraits): PersonalityModifiers {
  // Energetic personalities (high proactivity / risk) orbit faster; blink
  // frequency tracks proactivity alone. Warm / humorous personalities bounce
  // more and look around more. Formal personalities are calmer (less bounce).
  const energy = (t.proactivity + t.risk_tolerance) / 2;          // 0–1
  const expressiveness = (t.warmth + t.humor) / 2;                // 0–1
  const calmness = t.formality;                                     // 0–1

  return {
    orbitSpeedMult: 0.7 + energy * 0.6,
    pulseMult: 0.85 + expressiveness * 0.3,
    blinkInterval: Math.round(280 - t.proactivity * 120),
    idleLookFreq: Math.round(400 - expressiveness * 200),
    bounceMagnitude: 0.7 + expressiveness * 0.6 - calmness * 0.3,
  };
}

export function getReplyDisplayTarget(reply: string): 'bubble' | 'dialog' {
  const compact = reply.trim().replace(/\s+/g, '');
  return compact.length <= 36 ? 'bubble' : 'dialog';
}
