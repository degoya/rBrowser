import { describe, expect, it } from 'vitest';
import type { MinimapState } from '../ipc/layout';
import { panned, project, toCanvas, toMap, union } from './minimap';

const map: MinimapState = {
  position: 'end',
  items: [
    { view: 1, breakpoint: 'mobile', rect: { x: 0, y: 0, width: 200, height: 420 }, visible: true },
    { view: 2, breakpoint: 'desktop', rect: { x: 216, y: 0, width: 720, height: 470 }, visible: false },
  ],
  viewport: { x: -100, y: -50, width: 800, height: 450 },
};

describe('the minimap geometry', () => {
  it('fits the views and the visible area into the box', () => {
    const projection = project(map, 160, 36);
    const all = union([...map.items.map((item) => item.rect), map.viewport]);
    expect(all).toEqual({ x: -100, y: -50, width: 1036, height: 520 });
    for (const rect of [...map.items.map((item) => item.rect), map.viewport]) {
      const shown = toMap(projection, rect);
      expect(shown.x).toBeGreaterThanOrEqual(2 - 1e-9);
      expect(shown.y).toBeGreaterThanOrEqual(2 - 1e-9);
      expect(shown.x + shown.width).toBeLessThanOrEqual(158 + 1e-9);
      expect(shown.y + shown.height).toBeLessThanOrEqual(34 + 1e-9);
    }
  });

  it('maps a click back to the canvas point under it', () => {
    const projection = project(map, 160, 36);
    const centre = toMap(projection, { x: 500, y: 200, width: 0, height: 0 });
    const back = toCanvas(projection, centre.x, centre.y);
    expect(back.x).toBeCloseTo(500);
    expect(back.y).toBeCloseTo(200);
  });

  it('pans by a share of the visible area', () => {
    expect(panned(map.viewport, 0.25, 0)).toEqual({ x: 500, y: 175 });
    expect(panned(map.viewport, 0, -0.5)).toEqual({ x: 300, y: -50 });
  });

  it('copes with nothing to show', () => {
    expect(union([])).toBeNull();
    const empty = project({ position: 'start', items: [], viewport: { x: 0, y: 0, width: 0, height: 0 } }, 80, 30);
    expect(Number.isFinite(empty.scale)).toBe(true);
  });
});
