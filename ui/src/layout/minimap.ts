// The minimap's geometry (RB-080-04): the canvas (every view and the visible area) fitted into
// the minimap's box, and back from a point of the minimap to the canvas. Pure, so Vitest holds it.

import type { MapRect, MinimapState } from '../ipc/layout';

/** Canvas DIP -> minimap px: `map = (canvas - origin) * scale + pad`. */
export interface Projection {
  scale: number;
  originX: number;
  originY: number;
  pad: number;
}

/** The smallest rectangle around `rects`; `null` for none. */
export function union(rects: MapRect[]): MapRect | null {
  if (rects.length === 0) return null;
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const rect of rects) {
    left = Math.min(left, rect.x);
    top = Math.min(top, rect.y);
    right = Math.max(right, rect.x + rect.width);
    bottom = Math.max(bottom, rect.y + rect.height);
  }
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** Fits the views and the visible area into `width` x `height` px with `pad` px around. */
export function project(map: MinimapState, width: number, height: number, pad = 2): Projection {
  const bounds = union([...map.items.map((item) => item.rect), map.viewport]) ?? {
    x: 0,
    y: 0,
    width: 1,
    height: 1,
  };
  const roomW = Math.max(1, width - 2 * pad);
  const roomH = Math.max(1, height - 2 * pad);
  const scale = Math.min(roomW / Math.max(1, bounds.width), roomH / Math.max(1, bounds.height));
  // Centred in the box.
  const originX = bounds.x - (roomW / scale - bounds.width) / 2;
  const originY = bounds.y - (roomH / scale - bounds.height) / 2;
  return { scale, originX, originY, pad };
}

/** A canvas rectangle in minimap px. */
export function toMap(projection: Projection, rect: MapRect): MapRect {
  const { scale, originX, originY, pad } = projection;
  return {
    x: (rect.x - originX) * scale + pad,
    y: (rect.y - originY) * scale + pad,
    width: rect.width * scale,
    height: rect.height * scale,
  };
}

/** The canvas point under the minimap point `x`/`y` (px). */
export function toCanvas(projection: Projection, x: number, y: number): { x: number; y: number } {
  const { scale, originX, originY, pad } = projection;
  return { x: (x - pad) / scale + originX, y: (y - pad) / scale + originY };
}

/** The centre of `viewport` moved by `dx`/`dy` times its own size (keyboard panning). */
export function panned(viewport: MapRect, dx: number, dy: number): { x: number; y: number } {
  return {
    x: viewport.x + viewport.width / 2 + dx * viewport.width,
    y: viewport.y + viewport.height / 2 + dy * viewport.height,
  };
}
