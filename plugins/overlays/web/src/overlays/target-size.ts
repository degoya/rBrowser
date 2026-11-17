// Target size (WCAG 2.5.8): interactive elements smaller than `targetSizePx` (24, or 44 for
// 2.5.5) either way. At 24 px and below the spacing exception holds: an undersized target passes
// when a circle of that diameter on its centre meets no other target and no other undersized
// target's circle. A link inside a sentence passes too (the inline exception).

import type { Rect } from '../types';
import { type Finder, createScanOverlay, intOption, value } from './scan';

const COLOR = '#d6336c';
export const TARGETS = [
  'a[href]',
  'area[href]',
  'button',
  'input:not([type="hidden"])',
  'select',
  'textarea',
  'summary',
  '[role="button"]',
  '[role="link"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="switch"]',
  '[role="tab"]',
  '[role="menuitem"]',
  '[role="option"]',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');
/** The most targets compared with each other. */
export const MAX_TARGETS = 2000;

const centre = (rect: Rect): [number, number] => [rect.x + rect.width / 2, rect.y + rect.height / 2];

/** The indices of `rects` that fail at `min` px; `related(i, j)` pairs (one inside the other)
 *  never clash. The spacing exception holds when `spacing`. */
export function undersized(rects: Rect[], min: number, spacing: boolean, related: (i: number, j: number) => boolean = () => false): number[] {
  const small = rects.map((rect) => rect.width < min || rect.height < min);
  const radius = min / 2;
  const failing: number[] = [];
  rects.forEach((rect, i) => {
    if (!small[i]) return;
    if (!spacing) {
      failing.push(i);
      return;
    }
    const [cx, cy] = centre(rect);
    const clash = rects.some((other, j) => {
      if (j === i || related(i, j)) return false;
      if (small[j]) {
        const [ox, oy] = centre(other);
        return Math.hypot(cx - ox, cy - oy) < min;
      }
      const dx = Math.max(other.x - cx, 0, cx - (other.x + other.width));
      const dy = Math.max(other.y - cy, 0, cy - (other.y + other.height));
      return Math.hypot(dx, dy) < radius;
    });
    if (clash) failing.push(i);
  });
  return failing;
}

/** Whether `element` is a link inside a sentence: inline, with text of its parent around it. */
export function inSentence(element: Element, display: string): boolean {
  if (display !== 'inline') return false;
  const own = element.textContent?.trim().length ?? 0;
  const around = element.parentElement?.textContent?.trim().length ?? 0;
  return own > 0 && around > own;
}

export const findTargets: Finder = ({ doc, win, settings, options }) => {
  const min = intOption(options, 'sizePx', 8, 96, settings.targetSizePx);
  const elements = Array.from(doc.querySelectorAll(TARGETS))
    .filter((element) => !(element as HTMLButtonElement).disabled)
    .slice(0, MAX_TARGETS)
    .map((element) => ({ element, box: element.getBoundingClientRect() }))
    .filter(({ box }) => box.width > 0 && box.height > 0);
  const rects = elements.map(({ box }) => ({ x: box.left, y: box.top, width: box.width, height: box.height }));
  const related = (i: number, j: number): boolean => {
    const one = elements[i]?.element;
    const two = elements[j]?.element;
    return !!one && !!two && (one.contains(two) || two.contains(one));
  };
  return undersized(rects, min, min <= 24, related).flatMap((index) => {
    const entry = elements[index];
    if (!entry || inSentence(entry.element, value(win.getComputedStyle(entry.element), 'display'))) return [];
    return [{ element: entry.element, label: `${Math.round(entry.box.width)}x${Math.round(entry.box.height)}` }];
  });
};

export const createTargetSize = () => createScanOverlay('target-size', COLOR, findTargets);
