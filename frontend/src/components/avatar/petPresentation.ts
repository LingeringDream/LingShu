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

export function getReplyDisplayTarget(reply: string): 'bubble' | 'dialog' {
  const compact = reply.trim().replace(/\s+/g, '');
  return compact.length <= 36 ? 'bubble' : 'dialog';
}
