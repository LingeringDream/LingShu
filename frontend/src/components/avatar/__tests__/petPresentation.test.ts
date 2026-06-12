import { describe, expect, it } from 'vitest';

import {
  getMoodPresentation,
  getReplyDisplayTarget,
  lerpColor,
  lerpPresentation,
  traitsToModifiers,
  type Mood,
  type PersonalityTraits,
} from '../petPresentation';

describe('pet presentation helpers', () => {
  it('shows short replies in the speech bubble', () => {
    expect(getReplyDisplayTarget('我在呢，日程已经准备好。')).toBe('bubble');
  });

  it('keeps long replies in the click-open dialog', () => {
    const reply = '今天下午三点有设计评审，四点半还有一次同步会。我建议你在两点四十五分预留十五分钟整理材料，并把会议链接放到日程备注里。';

    expect(getReplyDisplayTarget(reply)).toBe('dialog');
  });

  it.each([
    ['idle', 0x2e6bff, 0.45, 'open'],
    ['thinking', 0x9b8cff, 1.15, 'focused'],
    ['speaking', 0x6ce0a0, 0.85, 'open'],
    ['happy', 0xffc060, 1.35, 'smiling'],
    ['sleepy', 0x8899bb, 0.25, 'sleepy'],
  ] satisfies Array<[Mood, number, number, string]>)(
    'maps %s mood to the expected visual presentation',
    (mood, color, orbitSpeed, eyeShape) => {
      expect(getMoodPresentation(mood)).toMatchObject({
        color,
        orbitSpeed,
        eyeShape,
      });
    },
  );
});

describe('mood transition lerp', () => {
  it('returns the endpoints at t=0 and t=1', () => {
    expect(lerpColor(0x2e6bff, 0x9b8cff, 0)).toBe(0x2e6bff);
    expect(lerpColor(0x2e6bff, 0x9b8cff, 1)).toBe(0x9b8cff);
  });

  it('interpolates each RGB channel independently', () => {
    // r: 0x2e→0x9b, g: 0x6b→0x8c, b: 0xff→0xff at the midpoint
    expect(lerpColor(0x2e6bff, 0x9b8cff, 0.5)).toBe(0x657cff);
  });

  it('eases numeric fields but switches eye shape immediately', () => {
    const mid = lerpPresentation(
      getMoodPresentation('idle'),
      getMoodPresentation('thinking'),
      0.5,
    );

    expect(mid.orbitSpeed).toBeCloseTo(0.8);
    expect(mid.pulse).toBeCloseTo(0.95);
    expect(mid.color).toBe(0x657cff);
    expect(mid.eyeShape).toBe('focused');
  });
});

describe('personality-driven modifiers', () => {
  const base: PersonalityTraits = {
    directness: 0.5,
    warmth: 0.5,
    proactivity: 0.5,
    risk_tolerance: 0.5,
    verbosity: 0.5,
    formality: 0.5,
    humor: 0.5,
  };

  it('blinks more often as proactivity rises (design doc §9)', () => {
    const calm = traitsToModifiers({ ...base, proactivity: 0 });
    const eager = traitsToModifiers({ ...base, proactivity: 1 });

    expect(calm.blinkInterval).toBe(280);
    expect(eager.blinkInterval).toBe(160);
  });

  it('orbits faster for energetic personalities', () => {
    const calm = traitsToModifiers({ ...base, proactivity: 0, risk_tolerance: 0 });
    const eager = traitsToModifiers({ ...base, proactivity: 1, risk_tolerance: 1 });

    expect(eager.orbitSpeedMult).toBeGreaterThan(calm.orbitSpeedMult);
  });

  it('bounces more for warm/humorous and less for formal personalities', () => {
    const warm = traitsToModifiers({ ...base, warmth: 1, humor: 1, formality: 0 });
    const formal = traitsToModifiers({ ...base, warmth: 0, humor: 0, formality: 1 });

    expect(warm.bounceMagnitude).toBeGreaterThan(formal.bounceMagnitude);
  });
});
