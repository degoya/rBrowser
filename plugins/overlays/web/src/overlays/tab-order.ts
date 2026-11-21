// The tab order overlay (RB-070-12): a numbered marker on every stop of the tab order
// (./tab-order/order.ts), lines between them in order (setting `tabOrder.showLines`), a jump
// against the reading order and its line in orange, a stop with a warning in red. The stop count
// goes to the panel. Live (setting `tabOrder.live`): a stop focused in this view is ringed and
// named to the component, which posts it to every view; each view rings the stop with the same
// path - without moving the focus anywhere. The reading order (setting `tabOrder.readingOrder`,
// RB-094-02): dashed arrows between the stops in source order, beside the tab order lines or
// instead of them, and a frame on each stop the reading order reaches out of place
// (./tab-order/reading.ts, the outline plugin's `reading-order` rule).

import { LIVE, MARKER, MISREAD, READING, REVERSE, WARNED } from './tab-order/colors';
import { composedPath } from './tab-order/order';
import { type TabItem, tabList, viewportRect } from './tab-order/list';
import { contentRtl, readingOrder } from './tab-order/reading';
import { type Overlay, type OverlayContext } from './overlay';
import type { Item } from '../layer';
import type { Shape } from '../shapes';
import type { Rect } from '../types';

const MARKERS = 'tab-order';
const LINES = 'tab-order-lines';
const RING = 'tab-order-live';
const READING_ARROWS = 'tab-order-reading';
const MISREAD_FRAMES = 'tab-order-misread';
/** The most stops the overlay draws. */
export const MAX_DRAWN = 500;
/** The longest path a focus message carries (src/tab_order.rs `MAX_TEXT_CHARS`). */
const MAX_PATH = 300;

const warned = (item: TabItem): boolean => item.warnings.some((warning) => warning === 'positive-tabindex' || warning.startsWith('invisible-'));

/** The focused element of `doc`, followed into open shadow roots and same-origin iframes. */
export function focused(doc: Document): Element | null {
  let active: Element | null = doc.activeElement;
  for (let depth = 0; active && depth < 32; depth += 1) {
    const inner: Element | null | undefined =
      active.shadowRoot?.activeElement ?? (active.localName === 'iframe' ? frameActive(active as HTMLIFrameElement) : undefined);
    if (!inner) break;
    active = inner;
  }
  return active && active !== doc.body && active !== doc.documentElement ? active : null;
}

function frameActive(frame: HTMLIFrameElement): Element | null {
  try {
    const inner = frame.contentDocument;
    return inner?.activeElement && inner.activeElement !== inner.body ? inner.activeElement : null;
  } catch {
    return null;
  }
}

/** The markers and lines of `items`, at their boxes `rects`. */
export function shapes(items: readonly TabItem[], rects: readonly { x: number; y: number }[], lines: boolean): { markers: Shape[]; lines: Shape[] } {
  const markers: Shape[] = [];
  const bars: Shape[] = [];
  items.forEach((item, place) => {
    const at = rects[place];
    if (!at) return;
    const reverse = item.warnings.includes('reverse-jump');
    const color = warned(item) ? WARNED : reverse ? REVERSE : MARKER;
    markers.push({ kind: 'marker', x: at.x, y: at.y, text: String(item.index), color });
    const before = rects[place - 1];
    if (lines && before) bars.push({ kind: 'line', x1: before.x, y1: before.y, x2: at.x, y2: at.y, color: reverse ? REVERSE : MARKER });
  });
  return { markers, lines: bars };
}

const centre = (rect: Rect): { x: number; y: number } => ({ x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 });

/** The reading order of `sequence` (stops in source order) at their boxes `rects`: a dashed arrow
 *  from each centre to the next, and a frame on each stop of `misread`, the arrow to it alike. */
export function readingShapes(sequence: readonly TabItem[], rects: readonly Rect[], misread: ReadonlySet<TabItem>): { arrows: Shape[]; marks: Item[] } {
  const lines: Shape[] = [];
  const heads: Shape[] = [];
  const marks: Item[] = [];
  sequence.forEach((item, place) => {
    const rect = rects[place];
    if (!rect) return;
    const color = misread.has(item) ? MISREAD : READING;
    if (misread.has(item)) marks.push({ rect, color, border: 2, halo: true });
    const previous = rects[place - 1];
    if (!previous) return;
    const from = centre(previous);
    const to = centre(rect);
    lines.push({ kind: 'line', x1: from.x, y1: from.y, x2: to.x, y2: to.y, color, dashed: true });
    heads.push({ kind: 'head', x: to.x, y: to.y, angle: Math.atan2(to.y - from.y, to.x - from.x), color });
  });
  // The heads after the lines, so they lie on them.
  return { arrows: [...lines, ...heads], marks };
}

export function createTabOrder(): Overlay {
  let items: TabItem[] = [];
  let reading: { sequence: TabItem[]; misread: Set<TabItem> } = { sequence: [], misread: new Set() };
  let paths: string[] = [];
  let told = -1;
  let live = '';
  let sent = '';
  let listening: { doc: Document; listener: () => void } | undefined;

  const stopListening = (): void => {
    listening?.doc.removeEventListener('focusin', listening.listener, true);
    listening = undefined;
  };

  return {
    id: 'tab-order',
    update(context: OverlayContext) {
      if (context.settings.tabOrderLive && !listening) {
        const listener = (): void => {
          const active = focused(context.doc);
          const place = active ? items.findIndex((item) => item.stop.element === active) : -1;
          const path = paths[place];
          if (!path) return;
          live = path;
          this.draw(context);
          if (path !== sent) {
            sent = path;
            context.send({ type: 'tab-focus', path });
          }
        };
        context.doc.addEventListener('focusin', listener, true);
        listening = { doc: context.doc, listener };
      } else if (!context.settings.tabOrderLive) {
        stopListening();
        live = '';
      }
      this.measure(context);
    },
    measure(context: OverlayContext) {
      items = tabList(context.doc, context.win, { includeIframes: context.settings.tabOrderIncludeIframes }, MAX_DRAWN);
      paths = items.map((item) => composedPath(item.stop.element, item.stop.frames));
      reading =
        context.settings.tabOrderReadingOrder === 'off'
          ? { sequence: [], misread: new Set() }
          : readingOrder(items, contentRtl(context.doc, context.win), context.settings.tabOrderReadingTolerancePx);
      if (items.length !== told) {
        told = items.length;
        context.emit({ type: 'hits', overlay: 'tab-order', count: items.length });
      }
      this.draw(context);
    },
    draw(context: OverlayContext) {
      const kept = items.filter((item) => item.stop.element.isConnected);
      const boxes = kept.map((item) => viewportRect(item.stop));
      const mode = context.settings.tabOrderReadingOrder;
      const drawn = shapes(kept, boxes, context.settings.tabOrderShowLines && mode !== 'instead');
      const boxOf = new Map(kept.map((item, place) => [item, boxes[place] as Rect]));
      const sequence = reading.sequence.filter((item) => boxOf.has(item));
      const read = readingShapes(sequence, sequence.map((item) => boxOf.get(item) as Rect), reading.misread);
      // The lines and arrows first, so the markers lie on them.
      context.layer.renderShapes(LINES, drawn.lines);
      context.layer.renderShapes(READING_ARROWS, read.arrows);
      context.layer.render(MISREAD_FRAMES, read.marks);
      context.layer.renderShapes(MARKERS, drawn.markers);
      const place = live ? paths.indexOf(live) : -1;
      const item = items[place];
      const rect = item?.stop.element.isConnected ? viewportRect(item.stop) : undefined;
      context.layer.render(RING, rect && item ? [{ rect, color: LIVE, border: 3, halo: true, label: `tab ${item.index}` }] : []);
    },
    clear(context: OverlayContext) {
      stopListening();
      items = [];
      paths = [];
      reading = { sequence: [], misread: new Set() };
      told = -1;
      live = '';
      sent = '';
      for (const group of [LINES, READING_ARROWS, MISREAD_FRAMES, MARKERS, RING]) context.layer.clear(group);
    },
    receive(context: OverlayContext, message: unknown) {
      const focus = message as { type?: unknown; path?: unknown } | null;
      if (focus?.type !== 'tab-focus' || typeof focus.path !== 'string' || focus.path.length > MAX_PATH) return;
      live = focus.path;
      this.draw(context);
    },
  };
}
