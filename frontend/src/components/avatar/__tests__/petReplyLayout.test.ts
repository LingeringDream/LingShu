import { describe, expect, it } from 'vitest';

import {
  PET_REPLY_BUBBLE_RECT,
  PET_REPLY_PANEL_RECT,
  rectIntersectsPetBody,
} from '../petReplyLayout';

describe('pet reply layout', () => {
  it('keeps the compact reply bubble outside the pet body safe area', () => {
    const oldBubbleRect = { left: 14, top: 124, width: 172, height: 38 };

    expect(rectIntersectsPetBody(oldBubbleRect)).toBe(true);
    expect(rectIntersectsPetBody(PET_REPLY_BUBBLE_RECT)).toBe(false);
  });

  it('docks the full reply panel below the pet body instead of covering it', () => {
    const oldPanelRect = { left: 6, top: 42, width: 188, height: 210 };

    expect(rectIntersectsPetBody(oldPanelRect)).toBe(true);
    expect(rectIntersectsPetBody(PET_REPLY_PANEL_RECT)).toBe(false);
  });
});
