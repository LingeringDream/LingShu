/* global CustomEvent, localStorage */
import { describe, expect, it, beforeEach } from 'vitest';

import {
  AVATAR_CONTROL_EVENT,
  AVATAR_CONTROL_STORAGE_KEY,
  DEFAULT_AVATAR_CONTROL_SETTINGS,
  avatarMoodToPetMood,
  loadAvatarControlSettings,
  normalizeAvatarControlSettings,
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
      sizeScale: 1.18,
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

  it('maps control-only presentation choices to pet runtime moods', () => {
    expect(avatarMoodToPetMood('reminder')).toBe('happy');
    expect(avatarMoodToPetMood('thinking')).toBe('thinking');
  });

  it('normalizes numeric size scale and clamps invalid values', () => {
    expect(normalizeAvatarControlSettings({ sizeScale: 1.2 }).sizeScale).toBe(1.2);
    expect(normalizeAvatarControlSettings({ sizeScale: 2 }).sizeScale).toBe(1.25);
    expect(normalizeAvatarControlSettings({ sizeScale: 0.2 }).sizeScale).toBe(0.75);
  });

  it('migrates legacy preset size values into numeric scale values', () => {
    expect(normalizeAvatarControlSettings({ size: 'small' }).sizeScale).toBe(0.86);
    expect(normalizeAvatarControlSettings({ size: 'medium' }).sizeScale).toBe(1);
    expect(normalizeAvatarControlSettings({ size: 'large' }).sizeScale).toBe(1.12);
  });
});
