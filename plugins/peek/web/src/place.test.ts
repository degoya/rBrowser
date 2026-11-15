import { describe, expect, it } from 'vitest';
import { GAP, placeTooltip } from './place';

const viewport = { width: 800, height: 600 };
const tip = { width: 200, height: 100 };

describe('the tooltip placement', () => {
  it('goes below the element', () => {
    expect(placeTooltip({ x: 50, y: 50, width: 100, height: 20 }, tip, viewport)).toEqual({
      left: 50,
      top: 50 + 20 + GAP,
    });
  });

  it('goes above when there is no room below', () => {
    expect(placeTooltip({ x: 50, y: 520, width: 100, height: 40 }, tip, viewport)).toEqual({
      left: 50,
      top: 520 - GAP - 100,
    });
  });

  it('stays inside the viewport for a huge element and at the right edge', () => {
    const place = placeTooltip({ x: 780, y: 0, width: 300, height: 600 }, tip, viewport);
    expect(place.left).toBe(800 - 200 - GAP);
    expect(place.top).toBe(600 - 100 - GAP);
    expect(placeTooltip({ x: -40, y: 10, width: 10, height: 10 }, tip, viewport).left).toBe(GAP);
  });
});
