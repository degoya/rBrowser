import { describe, expect, it } from 'vitest';
import { overflowEdges, pageOffset, revealOffset } from './tabScroll';

describe('the tab row that scrolls (RB-080-81)', () => {
  it('names the sides with more tabs beyond them', () => {
    // 1600 px wide: every tab fits.
    expect(overflowEdges(0, 1600, 1600)).toEqual({ start: false, end: false });
    // 400 px wide at the start, in the middle, at the end.
    expect(overflowEdges(0, 1200, 400)).toEqual({ start: false, end: true });
    expect(overflowEdges(300, 1200, 400)).toEqual({ start: true, end: true });
    expect(overflowEdges(800, 1200, 400)).toEqual({ start: true, end: false });
    // A fraction of a pixel left over is no overflow (zoom rounding).
    expect(overflowEdges(799.4, 1200, 400)).toEqual({ start: true, end: false });
  });

  it('scrolls the active tab into sight, with a margin for the neighbour', () => {
    // Already in sight: the row stays.
    expect(revealOffset({ left: 100, width: 80 }, 0, 400, 1200)).toBe(0);
    // Beyond the end: its end comes in, plus the margin.
    expect(revealOffset({ left: 900, width: 100 }, 0, 400, 1200)).toBe(1000 - 400 + 24);
    // Before the start: its start comes in, less the margin.
    expect(revealOffset({ left: 300, width: 100 }, 600, 400, 1200)).toBe(300 - 24);
    // Never past either end of the row.
    expect(revealOffset({ left: 10, width: 100 }, 600, 400, 1200)).toBe(0);
    expect(revealOffset({ left: 1150, width: 50 }, 0, 400, 1200)).toBe(800);
    // A tab wider than the visible part shows its start, where its label begins.
    expect(revealOffset({ left: 500, width: 120 }, 0, 100, 1200)).toBe(500);
  });

  it('pages by most of the visible width, within the row', () => {
    expect(pageOffset(0, 400, 1200, 1)).toBe(320);
    expect(pageOffset(700, 400, 1200, 1)).toBe(800);
    expect(pageOffset(200, 400, 1200, -1)).toBe(0);
  });
});
