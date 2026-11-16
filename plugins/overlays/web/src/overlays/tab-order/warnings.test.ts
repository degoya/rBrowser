import { describe, expect, it } from 'vitest';
import { contrast } from '../../layer';
import { MARKER, REVERSE, RING, WARNED } from './colors';
import { displaced, hiddenReason, isReverse, visualOrder } from './warnings';

const rect = (x: number, y: number, width = 40, height = 20) => ({ x, y, width, height });
const style = (values: Record<string, string>) => ({ getPropertyValue: (name: string) => values[name] ?? '' });

describe('a jump against the reading order', () => {
  it('is one up, or one to the left on the same row', () => {
    expect(isReverse(rect(100, 0), rect(10, 0), false)).toBe(true);
    expect(isReverse(rect(10, 0), rect(100, 0), false)).toBe(false);
    expect(isReverse(rect(10, 100), rect(10, 0), false)).toBe(true);
    // The next line, starting left again, reads on.
    expect(isReverse(rect(300, 0), rect(10, 40), false)).toBe(false);
  });

  it('runs the other way in a right-to-left page', () => {
    expect(isReverse(rect(10, 0), rect(100, 0), true)).toBe(true);
    expect(isReverse(rect(100, 0), rect(10, 0), true)).toBe(false);
  });
});

describe('the visual order', () => {
  it('reads rows top to bottom, each left to right', () => {
    expect(visualOrder([rect(200, 0), rect(10, 2), rect(10, 50), rect(100, 0)], false)).toEqual([1, 3, 0, 2]);
    expect(visualOrder([rect(200, 0), rect(10, 2)], true)).toEqual([0, 1]);
  });

  it('marks the stops that stand far from their place in it', () => {
    // Eight stops in a row, the last one drawn first: it is seven places off, the rest one each.
    const rects = [1, 2, 3, 4, 5, 6, 7, 0].map((column) => rect(column * 50, 0));
    expect(displaced(rects, false)).toEqual([7]);
    expect(displaced(rects.slice(0, 7), false)).toEqual([]);
  });
});

describe('a focusable element nobody sees', () => {
  const viewport = { scrollX: 0, scrollY: 0, innerWidth: 400, innerHeight: 800 };

  it('has no size, no opacity or lies before the page where no scroll reaches', () => {
    expect(hiddenReason(rect(10, 10, 0, 20), [], viewport)).toBe('size');
    expect(hiddenReason(rect(10, 10), [style({ opacity: '0' })], viewport)).toBe('opacity');
    expect(hiddenReason(rect(-300, 10, 280, 20), [], viewport)).toBe('offscreen');
    expect(hiddenReason(rect(10, -50), [], viewport)).toBe('offscreen');
    expect(hiddenReason(rect(10, 5000), [], viewport)).toBeUndefined();
    expect(hiddenReason(rect(10, 10), [style({ opacity: '0.4' })], viewport)).toBeUndefined();
  });

  it('is clipped away by an ancestor that hides its overflow', () => {
    const menu = { rect: rect(0, 0, 400, 0), style: style({ 'overflow-x': 'visible', 'overflow-y': 'hidden' }) };
    expect(hiddenReason(rect(10, 10), [], viewport, [menu])).toBe('clipped');
    const open = { rect: rect(0, 0, 400, 200), style: menu.style };
    expect(hiddenReason(rect(10, 10), [], viewport, [open])).toBeUndefined();
    const scrolls = { rect: rect(0, 0, 400, 0), style: style({ 'overflow-y': 'auto' }) };
    expect(hiddenReason(rect(10, 10), [], viewport, [scrolls])).toBeUndefined();
    // Focus scrolls a carousel to its next slide; a box that clips cannot be scrolled.
    const carousel = { rect: rect(0, 0, 300, 100), style: style({ 'overflow-x': 'hidden' }) };
    expect(hiddenReason(rect(320, 10), [], viewport, [carousel])).toBeUndefined();
    const clip = { rect: rect(0, 0, 300, 100), style: style({ 'overflow-x': 'clip' }) };
    expect(hiddenReason(rect(320, 10), [], viewport, [clip])).toBe('clipped');
  });
});

describe('the marker colours', () => {
  it('give their white numbers WCAG AA contrast and show on light pages; the white ring on dark ones', () => {
    for (const color of [MARKER, WARNED, REVERSE]) {
      expect(contrast(color, RING)).toBeGreaterThanOrEqual(4.5);
    }
    expect(contrast(RING, '#000000')).toBeGreaterThanOrEqual(3);
    expect(contrast(RING, '#1f2937')).toBeGreaterThanOrEqual(3);
  });
});
