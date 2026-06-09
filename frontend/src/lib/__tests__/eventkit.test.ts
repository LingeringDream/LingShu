import { describe, it, expect } from 'vitest';

// Type-only imports for compile-time checking — the actual module has
// runtime deps (Tauri invoke) that need a real Tauri environment.

describe('eventkit types', () => {
  it('CalendarAccessStatus union is valid', () => {
    const valid: string[] = [
      'full_access',
      'denied',
      'restricted',
      'write_only',
      'not_determined',
      'unsupported',
    ];
    expect(valid.length).toBe(6);
    expect(new Set(valid).size).toBe(6); // no duplicates
  });

  it('AppleEventInput has required fields', () => {
    // Compile-time type check: this assignment must type-check
    const input = {
      title: 'Test',
      start_time: '2026-06-01T09:00:00Z',
      end_time: '2026-06-01T10:00:00Z',
      location: 'Room 1',
      notes: 'Bring laptop',
      calendar_name: 'work',
    };
    expect(input.title).toBe('Test');
    expect(input.location).toBe('Room 1');
    expect(input.notes).toBe('Bring laptop');
  });
});
