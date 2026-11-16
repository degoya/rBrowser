// Where the tooltip goes: below the element, else above it, else inside the viewport's bottom
// edge; never out of the viewport on either axis.

import type { Rect } from './types';

/** The distance from the element and from the viewport's edges, in CSS px. */
export const GAP = 8;

export function placeTooltip(
  anchor: Rect,
  tip: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number } {
  const maxLeft = Math.max(GAP, viewport.width - tip.width - GAP);
  const left = Math.min(Math.max(anchor.x, GAP), maxLeft);
  const below = anchor.y + anchor.height + GAP;
  const above = anchor.y - GAP - tip.height;
  let top: number;
  if (below + tip.height <= viewport.height - GAP) top = below;
  else if (above >= GAP) top = above;
  else top = Math.max(GAP, viewport.height - tip.height - GAP);
  return { left: Math.round(left), top: Math.round(top) };
}
