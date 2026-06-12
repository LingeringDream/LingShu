import { describe, expect, it } from 'vitest';

import { getChatScrollBehavior } from '../chatScroll';

describe('chat scroll behavior', () => {
  it('uses immediate scrolling while a reply is streaming', () => {
    expect(getChatScrollBehavior(true)).toBe('auto');
  });

  it('uses smooth scrolling when the chat is idle', () => {
    expect(getChatScrollBehavior(false)).toBe('smooth');
  });
});
