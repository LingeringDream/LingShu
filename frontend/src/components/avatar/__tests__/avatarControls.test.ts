/* global CustomEvent, localStorage */
import { describe, expect, it, beforeEach } from 'vitest';

import {
  AVATAR_CONTROL_EVENT,
  AVATAR_CONTROL_STORAGE_KEY,
  DEFAULT_AVATAR_CONTROL_SETTINGS,
  avatarMoodToPetMood,
  avatarSizeToScale,
  loadAvatarControlSettings,
  publishAvatarControlSettings,
} from '../avatarControls';

describe('avatar control bridge', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('loads defaults when no settings have been saved', () => {
    expect(loadAvatarControlSettings()).toEqual(DEFAULT_AVATAR_CONTROL_SETTINGS);
  });

  it('persists settings and dispatches an in-window control event', async () => {
    const settings = {
      visible: false,
      mood: 'reminder' as const,
      size: 'large' as const,
      bubbleText: '该休息一下了。',
    };

    const received = new Promise((resolve) => {
      window.addEventListener(
        AVATAR_CONTROL_EVENT,
        (event) => resolve((event as CustomEvent).detail),
        { once: true },
      );
    });

    await publishAvatarControlSettings(settings);

    expect(JSON.parse(localStorage.getItem(AVATAR_CONTROL_STORAGE_KEY) ?? '{}')).toEqual(settings);
    await expect(received).resolves.toEqual(settings);
  });

  it('maps control-only presentation choices to pet runtime values', () => {
    expect(avatarMoodToPetMood('reminder')).toBe('happy');
    expect(avatarMoodToPetMood('thinking')).toBe('thinking');
    expect(avatarSizeToScale('small')).toBeLessThan(avatarSizeToScale('medium'));
    expect(avatarSizeToScale('large')).toBeGreaterThan(avatarSizeToScale('medium'));
  });
});
