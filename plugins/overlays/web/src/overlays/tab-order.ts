// The tab order overlay (RB-070-12): a numbered marker on every stop of the tab order
// (./tab-order/order.ts), lines between them in order (setting `tabOrder.showLines`), a jump
// against the reading order and its line in orange, a stop with a warning in red. The stop count
// goes to the panel. Live (setting `tabOrder.live`): a stop focused in this view is ringed and
// named to the component, which posts it to every view; each view rings the stop with the same
// path - without moving the focus anywhere.

import { LIVE, MARKER, REVERSE, WARNED } from './tab-order/colors';
import { composedPath } from './tab-order/order';
import { type TabItem, tabList, viewportRect } from './tab-order/list';
import { type Overlay, type OverlayContext } from './overlay';
import type { Shape } from '../shapes';

const MARKERS = 'tab-order';
const LINES = 'tab-order-lines';
const RING = 'tab-order-live';
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

export function createTabOrder(): Overlay {
  let items: TabItem[] = [];
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
      if (items.length !== told) {
        told = items.length;
        context.emit({ type: 'hits', overlay: 'tab-order', count: items.length });
      }
      this.draw(context);
    },
    draw(context: OverlayContext) {
      const kept = items.filter((item) => item.stop.element.isConnected);
      const boxes = kept.map((item) => viewportRect(item.stop));
      const drawn = shapes(kept, boxes, context.settings.tabOrderShowLines);
      // The lines first, so the markers lie on them.
      context.layer.renderShapes(LINES, drawn.lines);
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
      told = -1;
      live = '';
      sent = '';
      for (const group of [LINES, MARKERS, RING]) context.layer.clear(group);
    },
    receive(context: OverlayContext, message: unknown) {
      const focus = message as { type?: unknown; path?: unknown } | null;
      if (focus?.type !== 'tab-focus' || typeof focus.path !== 'string' || focus.path.length > MAX_PATH) return;
      live = focus.path;
      this.draw(context);
    },
  };
}
