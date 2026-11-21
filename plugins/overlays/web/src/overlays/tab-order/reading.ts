// The reading order against the source order (RB-094-02), for the tab order overlay's reading
// arrows (setting `tabOrder.readingOrder`): the outline plugin's rule `reading-order`
// (plugins/outline/src/rules/reading.rs) mirrored - of each neighbouring pair in source order, the
// later one that shows before the earlier one, left of it on the same line or above it and not in a
// column to its right, right to left where the content reads so. The shared fixture
// tests/fixtures/reading-order/cases.json pins both to the same cases. The overlay reads the tab
// stops; stops nobody sees and stops fixed or sticky on the screen have no place in it.

import type { Rect } from '../../types';
import type { TabItem } from './list';

/** A box `x, y, width, height` in whole CSS px, as the outline plugin measures it. */
export type Box = [number, number, number, number];

/** Whether `next` lies before `previous` in reading order (reading.rs `before`): on the same line
 *  (overlapping by at least half the lower one's height) and ending left of `previous`'s start, or
 *  entirely above it and not in a column to its right; `tolerance` the px the two may overlap by
 *  and still count as one before the other. */
export function before(previous: Box, next: Box, tolerance: number): boolean {
  const [px, py, pw, ph] = previous;
  const [nx, ny, nw, nh] = next;
  const overlap = Math.min(py + ph, ny + nh) - Math.max(py, ny);
  if (overlap > 0 && overlap * 2 >= Math.min(ph, nh)) return nx < px && nx + nw <= px + tolerance;
  const above = ny < py && ny + nh <= py + tolerance;
  const rightColumn = nx >= px + pw;
  return above && !rightColumn;
}

/** `rect` in whole px as left to right reads it: mirrored for content that reads right to left. */
export function readingBox(rect: Rect, rtl: boolean): Box {
  const [x, y, width, height] = [rect.x, rect.y, rect.width, rect.height].map((value) => (Number.isFinite(value) ? Math.round(value) : 0)) as Box;
  return rtl ? [-(x + width), y, width, height] : [x, y, width, height];
}

/** The places in `rects` (in source order) of the elements that show before the one before them. */
export function outOfPlace(rects: readonly Rect[], rtl: boolean, tolerance: number): number[] {
  const found: number[] = [];
  for (let place = 1; place < rects.length; place += 1) {
    const previous = rects[place - 1];
    const next = rects[place];
    if (previous && next && before(readingBox(previous, rtl), readingBox(next, rtl), tolerance)) found.push(place);
  }
  return found;
}

/** Whether the content of `doc` reads right to left: the nearest `dir` attribute, else the CSS
 *  `direction` (the outline collector's `directionOf`). */
export function contentRtl(doc: Document, win: Window): boolean {
  const root = doc.body ?? doc.documentElement;
  if (!root) return false;
  const attribute = root.closest('[dir]')?.getAttribute('dir')?.toLowerCase();
  if (attribute === 'rtl' || attribute === 'ltr') return attribute === 'rtl';
  return win.getComputedStyle(root).direction === 'rtl';
}

/** The tab stops in source order that have a place in the reading order, and those of them read
 *  out of place. */
export function readingOrder(items: readonly TabItem[], rtl: boolean, tolerance: number): { sequence: TabItem[]; misread: Set<TabItem> } {
  const sequence = items
    .filter((item) => !item.fixed && item.rect.width > 0 && item.rect.height > 0 && !item.warnings.some((warning) => warning.startsWith('invisible-')))
    .sort((left, right) => (left.stop.source ?? left.index) - (right.stop.source ?? right.index));
  const misread = new Set(outOfPlace(sequence.map((item) => item.rect), rtl, tolerance).map((place) => sequence[place] as TabItem));
  return { sequence, misread };
}
