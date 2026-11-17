// What the tab order overlay warns about (RB-070-12): a jump against the reading order (up, or
// back on the same row), a stop far from its place in the visual order, and a focusable element
// nobody sees - no size, opacity 0, before the page where no scroll reaches (a menu moved out
// with a transform), or clipped away by an ancestor (a closed menu with max-height 0 and its
// overflow hidden). Pure geometry: the overlay measures, these decide.

import type { Rect } from '../../types';

type Style = Pick<CSSStyleDeclaration, 'getPropertyValue'>;

/** How far apart two stops must be, at least, in places, to count as displaced. */
export const MIN_DISPLACEMENT = 3;
/** ... and as a share of the stops. */
export const DISPLACEMENT_SHARE = 0.2;

function overlap(start: number, length: number, otherStart: number, otherLength: number): number {
  return Math.min(start + length, otherStart + otherLength) - Math.max(start, otherStart);
}

/** Whether `one` and `two` stand on one row: they share at least half the lower one's height. */
export function sameRow(one: Rect, two: Rect): boolean {
  return overlap(one.y, one.height, two.y, two.height) >= Math.min(one.height, two.height) / 2;
}

/** Whether going from `from` to `to` runs against the reading order: up, or back along a row
 *  (left, or right in a right-to-left page). */
export function isReverse(from: Rect, to: Rect, rtl: boolean): boolean {
  if (sameRow(from, to)) return rtl ? to.x > from.x + 1 : to.x + 1 < from.x;
  return to.y + to.height / 2 < from.y;
}

/** The indices of `rects` in visual order: rows top to bottom, each in reading direction. */
export function visualOrder(rects: readonly Rect[], rtl: boolean): number[] {
  return rects
    .map((_, index) => index)
    .sort((left, right) => {
      const one = rects[left] as Rect;
      const two = rects[right] as Rect;
      if (!sameRow(one, two)) return one.y - two.y;
      return rtl ? two.x + two.width - (one.x + one.width) : one.x - two.x;
    });
}

/** The indices of `rects` (in tab order) whose place in the visual order lies at least
 *  max({@link MIN_DISPLACEMENT}, {@link DISPLACEMENT_SHARE} of the stops) places away. */
export function displaced(rects: readonly Rect[], rtl: boolean): number[] {
  const limit = Math.max(MIN_DISPLACEMENT, Math.ceil(rects.length * DISPLACEMENT_SHARE));
  const visual = visualOrder(rects, rtl);
  const found: number[] = [];
  visual.forEach((index, place) => {
    if (Math.abs(index - place) >= limit) found.push(index);
  });
  return found.sort((left, right) => left - right);
}

/** An ancestor that may clip: its box (viewport px) and its style. */
export interface Clipper {
  rect: Rect;
  style: Style;
}

/** Whether a box clipping one axis hides what lies at `start..start+length` there: `clip` hides
 *  everything outside it; `hidden` only when it has no size - otherwise focus scrolls it to the
 *  element (a carousel), but a closed menu of height 0 shows nothing whatever it scrolls. */
function clipsAway(overflow: string, start: number, length: number, boxStart: number, boxLength: number): boolean {
  if (overflow === 'clip') return overlap(start, length, boxStart, boxLength) <= 0;
  return overflow === 'hidden' && boxLength < 1;
}

/** Why a focusable element with box `rect` (viewport px) cannot be seen, if it cannot: `size`,
 *  `opacity` (of it or an ancestor, `styles`), `offscreen` (before the page's start, where no
 *  scroll reaches) or `clipped` (by one of `clippers`, its ancestors). */
export function hiddenReason(
  rect: Rect,
  styles: readonly Style[],
  win: Pick<Window, 'scrollX' | 'scrollY'>,
  clippers: readonly Clipper[] = [],
): string | undefined {
  if (rect.width < 1 || rect.height < 1) return 'size';
  if (styles.some((style) => Number.parseFloat(style.getPropertyValue('opacity')) === 0)) return 'opacity';
  if (rect.x + win.scrollX + rect.width <= 0 || rect.y + win.scrollY + rect.height <= 0) return 'offscreen';
  for (const { rect: box, style } of clippers) {
    if (clipsAway(style.getPropertyValue('overflow-x'), rect.x, rect.width, box.x, box.width)) return 'clipped';
    if (clipsAway(style.getPropertyValue('overflow-y'), rect.y, rect.height, box.y, box.height)) return 'clipped';
  }
  return undefined;
}
