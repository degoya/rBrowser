// The tab order measured (RB-070-12): each stop of ./order.ts with its box in the top viewport and
// its warnings (./warnings.ts). The page script draws it; `assets/measure.js` answers it to the
// component (`__rbOverlaysMeasure.tabOrder`) for `overlay_tab_order` and the view comparison.

import { cut, fitting } from '../../answer';
import type { Rect } from '../../types';
import { composedAncestors, composedPath, composedSelector, type OrderOptions, type TabStop, tabOrder } from './order';
import { type Clipper, displaced, hiddenReason, isReverse } from './warnings';

/** One stop, measured. */
export interface TabItem {
  stop: TabStop;
  /** Its place in the order, from 1. */
  index: number;
  /** Its box in the top viewport, px. */
  rect: Rect;
  /** `positive-tabindex`, `invisible-<size|opacity|offscreen|clipped>`, `reverse-jump`, `visual-order`. */
  warnings: string[];
}

/** What a measurement answers per stop, in document px of the top document. */
export interface TabEntry {
  index: number;
  selector: string;
  path: string;
  tabindex: number;
  rect: Rect;
  warnings: string[];
  iframe?: boolean;
}

/** The most stops an answer lists (src/tab_order.rs `MAX_ITEMS`). */
export const MAX_ENTRIES = 250;

const toRect = (box: DOMRect): Rect => ({ x: box.left, y: box.top, width: box.width, height: box.height });

/** Where the content of `frames` starts in the top viewport. */
export function frameOffset(frames: readonly HTMLIFrameElement[]): { x: number; y: number } {
  let x = 0;
  let y = 0;
  for (const frame of frames) {
    const box = frame.getBoundingClientRect();
    x += box.left + frame.clientLeft;
    y += box.top + frame.clientTop;
  }
  return { x, y };
}

/** The box of `stop` in the top viewport now. */
export function viewportRect(stop: TabStop): Rect {
  const own = toRect(stop.element.getBoundingClientRect());
  const offset = frameOffset(stop.frames);
  return { ...own, x: own.x + offset.x, y: own.y + offset.y };
}

/** Why the element of `stop` cannot be seen, if it cannot (in its own document's terms). */
function hidden(stop: TabStop): string | undefined {
  const element = stop.element;
  const win = element.ownerDocument.defaultView;
  if (!win) return undefined;
  const ancestors = composedAncestors(element);
  const styles = [element, ...ancestors].map((node) => win.getComputedStyle(node));
  const clippers: Clipper[] = [];
  ancestors.forEach((ancestor, index) => {
    const style = styles[index + 1];
    if (!style || ancestor === element.ownerDocument.documentElement) return;
    const overflow = `${style.getPropertyValue('overflow-x')} ${style.getPropertyValue('overflow-y')}`;
    if (/hidden|clip/.test(overflow)) clippers.push({ rect: toRect(ancestor.getBoundingClientRect()), style });
  });
  return hiddenReason(toRect(element.getBoundingClientRect()), styles, win, clippers);
}

/** The tab order of `doc` measured, at most `limit` stops. */
export function tabList(doc: Document, win: Window, options: OrderOptions, limit = Infinity): TabItem[] {
  const stops = tabOrder(doc, options).slice(0, limit);
  const rtl = doc.documentElement ? win.getComputedStyle(doc.documentElement).direction === 'rtl' : false;
  const items: TabItem[] = stops.map((stop, place) => ({ stop, index: place + 1, rect: viewportRect(stop), warnings: [] }));
  for (const item of items) {
    if (item.stop.tabindex > 0) item.warnings.push('positive-tabindex');
    const reason = item.stop.iframe ? undefined : hidden(item.stop);
    if (reason) item.warnings.push(`invisible-${reason}`);
  }
  const seen = items.filter((item) => !item.warnings.some((warning) => warning.startsWith('invisible-')));
  seen.forEach((item, place) => {
    const before = seen[place - 1];
    if (before && isReverse(before.rect, item.rect, rtl)) item.warnings.push('reverse-jump');
  });
  for (const place of displaced(seen.map((item) => item.rect), rtl)) seen[place]?.warnings.push('visual-order');
  return items;
}

/** `items` as a measurement answers them: the whole count and the first stops, at most
 *  {@link MAX_ENTRIES} and `MAX_ANSWER_BYTES` (RB-090-17), in document px. */
export function tabEntries(win: Window, items: readonly TabItem[]): { count: number; items: TabEntry[] } {
  const entries = items.slice(0, MAX_ENTRIES).map((item) => ({
    index: item.index,
    selector: cut(composedSelector(item.stop.element, item.stop.frames)),
    path: cut(composedPath(item.stop.element, item.stop.frames)),
    tabindex: item.stop.tabindex,
    rect: { x: item.rect.x + win.scrollX, y: item.rect.y + win.scrollY, width: item.rect.width, height: item.rect.height },
    warnings: item.warnings,
    ...(item.stop.iframe ? { iframe: true } : {}),
  }));
  return { count: items.length, items: fitting(entries) };
}
