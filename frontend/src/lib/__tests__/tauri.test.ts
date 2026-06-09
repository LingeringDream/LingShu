import { describe, it, expect } from 'vitest';
import { isTauri } from '../tauri';

describe('isTauri', () => {
  it('returns false in browser (non-Tauri env)', () => {
    // In vitest/jsdom, window.__TAURI__ is undefined
    expect(isTauri()).toBe(false);
  });
});
