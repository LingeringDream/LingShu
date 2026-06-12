import { describe, expect, it } from 'vitest';

import { combinePetRenderScale } from '../petScale';

describe('pet render scale', () => {
  it('keeps the user size setting while applying animation scale', () => {
    expect(combinePetRenderScale(1.2, 1)).toBeCloseTo(1.2);
    expect(combinePetRenderScale(1.2, 0.85)).toBeCloseTo(1.02);
  });
});
