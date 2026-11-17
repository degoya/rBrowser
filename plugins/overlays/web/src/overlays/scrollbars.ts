// Unneeded scrollbars: an element with `overflow: scroll|auto` that shows a scrollbar while its
// content overflows by at most `scrollbarTolerancePx` (or the option `tolerancePx`), and the
// viewport's own scrollbar for a page a few pixels too tall. Nothing on the page changes.

import type { Rect } from '../types';
import { type Finder, type Hit, createScanOverlay, elementsBelowBody, intOption, value } from './scan';

type Style = Pick<CSSStyleDeclaration, 'getPropertyValue'>;
export interface Box {
  offsetWidth: number;
  offsetHeight: number;
  clientWidth: number;
  clientHeight: number;
  scrollWidth: number;
  scrollHeight: number;
}

const COLOR = '#f08c00';
const scrollable = (overflow: string): boolean => overflow === 'scroll' || overflow === 'auto';

/** The label of an element's unneeded scrollbars (`scroll-y +0px`); none when it has none. */
export function unneededScrollbars(box: Box, style: Style, tolerance: number): string | undefined {
  const px = (name: string): number => Number.parseFloat(value(style, name)) || 0;
  const labels: string[] = [];
  const barY = box.offsetWidth - box.clientWidth - px('border-left-width') - px('border-right-width');
  const barX = box.offsetHeight - box.clientHeight - px('border-top-width') - px('border-bottom-width');
  const overY = box.scrollHeight - box.clientHeight;
  const overX = box.scrollWidth - box.clientWidth;
  if (scrollable(value(style, 'overflow-y')) && barY > 0 && overY <= tolerance) labels.push(`scroll-y +${Math.max(0, Math.round(overY))}px`);
  if (scrollable(value(style, 'overflow-x')) && barX > 0 && overX <= tolerance) labels.push(`scroll-x +${Math.max(0, Math.round(overX))}px`);
  return labels.length ? labels.join(' ') : undefined;
}

/** The viewport's vertical scrollbar when the page overflows by at most `tolerance`. */
export function rootScrollbar(doc: Document, win: Pick<Window, 'innerWidth' | 'innerHeight'>, tolerance: number): Hit | undefined {
  const root = doc.documentElement;
  const bar = win.innerWidth - root.clientWidth;
  const over = root.scrollHeight - root.clientHeight;
  if (bar <= 0 || over > tolerance) return undefined;
  const rect = (): Rect => ({ x: root.clientWidth, y: 0, width: bar, height: win.innerHeight });
  return { element: root, label: `scroll-y +${Math.max(0, Math.round(over))}px`, rect };
}

export const findScrollbars: Finder = ({ doc, win, settings, options }) => {
  const tolerance = intOption(options, 'tolerancePx', 0, 100, settings.scrollbarTolerancePx);
  const hits: Hit[] = [];
  const root = rootScrollbar(doc, win, tolerance);
  if (root) hits.push(root);
  for (const element of elementsBelowBody(doc)) {
    if (!('offsetWidth' in element)) continue;
    const label = unneededScrollbars(element as HTMLElement, win.getComputedStyle(element), tolerance);
    if (label) hits.push({ element, label });
  }
  return hits;
};

export const createScrollbars = () => createScanOverlay('scrollbars', COLOR, findScrollbars);
