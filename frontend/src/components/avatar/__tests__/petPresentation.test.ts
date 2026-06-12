import { describe, expect, it } from 'vitest';

import {
  getMoodPresentation,
  getReplyDisplayTarget,
  type Mood,
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
