import { describe, it, expect, beforeEach } from 'vitest';
import { getAuthToken, setAuthToken } from '../api';

describe('api — auth tokens', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('getAuthToken returns null when no token stored', () => {
    expect(getAuthToken()).toBeNull();
  });

  it('setAuthToken stores and getAuthToken retrieves', () => {
    setAuthToken('test-token-123');
    expect(getAuthToken()).toBe('test-token-123');
  });

  it('setAuthToken overwrites previous token', () => {
    setAuthToken('old-token');
    setAuthToken('new-token');
    expect(getAuthToken()).toBe('new-token');
  });

  it('storage key is namespaced under lingshu', () => {
    setAuthToken('abc');
    const stored = localStorage.getItem('lingshu_auth_token');
    expect(stored).toBe('abc');
  });
});
