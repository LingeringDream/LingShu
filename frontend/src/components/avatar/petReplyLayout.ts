export interface PetRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export const PET_WINDOW_SIZE = {
  width: 200,
  height: 260,
} as const;

export const PET_BODY = {
  cx: 100,
  cy: 110,
  radius: 64,
} as const;

export const PET_REPLY_BUBBLE_RECT: PetRect = {
  left: 14,
  top: 6,
  width: 172,
  height: 38,
};

export const PET_REPLY_PANEL_RECT: PetRect = {
  left: 8,
  top: 180,
  width: 184,
  height: 72,
};

export function rectIntersectsPetBody(rect: PetRect, body = PET_BODY): boolean {
  const nearestX = Math.max(rect.left, Math.min(body.cx, rect.left + rect.width));
  const nearestY = Math.max(rect.top, Math.min(body.cy, rect.top + rect.height));
  return Math.hypot(nearestX - body.cx, nearestY - body.cy) <= body.radius;
}
