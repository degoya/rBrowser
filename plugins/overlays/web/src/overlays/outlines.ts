// The layout outlines: every element (or only the flex and grid containers) framed in a colour
// by its nesting depth below <body>. The frames are boxes of the overlay layer, not outlines on
// the page's elements: no page style changes, nothing for the page's MutationObservers, and no
// effect on its layout. Only the boxes in the viewport are drawn.

import { type Overlay, type OverlayContext, toRect, visibleRects } from './overlay';

const GROUP = 'outlines';
/** The most elements the overlay follows, and the most boxes it draws at once. */
export const MAX_ELEMENTS = 5000;
export const MAX_BOXES = 1500;
/** One colour per depth, repeating; each readable on light and dark pages at 1 px. */
export const DEPTH_COLORS = ['#e5262b', '#f08c00', '#2f9e44', '#1c7ed6', '#7048e8', '#d6336c'];

const CONTAINER = /(^|-)(flex|grid)$/;

/** The elements to frame below `body`, each with its depth (1 for a child of body). */
export function collect(body: Element, win: Pick<Window, 'getComputedStyle'>, containersOnly: boolean, exclude?: Element | null): { element: Element; depth: number }[] {
  const found: { element: Element; depth: number }[] = [];
  const visit = (element: Element, depth: number): void => {
    for (const child of Array.from(element.children)) {
      if (found.length >= MAX_ELEMENTS) return;
      if (child === exclude) continue;
      if (!containersOnly || CONTAINER.test(win.getComputedStyle(child).display)) found.push({ element: child, depth });
      visit(child, depth + 1);
    }
  };
  visit(body, 1);
  return found;
}

export function createOutlines(): Overlay {
  let containersOnly: boolean | undefined;
  let elements: { element: Element; depth: number }[] = [];

  return {
    id: 'outlines',
    update(context: OverlayContext, options: Record<string, unknown>) {
      containersOnly = typeof options.containersOnly === 'boolean' ? options.containersOnly : undefined;
      this.measure(context);
    },
    measure(context: OverlayContext) {
      const body = context.doc.body;
      elements = body ? collect(body, context.win, containersOnly ?? context.settings.containersOnly, context.layer.host) : [];
      this.draw(context);
    },
    draw(context: OverlayContext) {
      const depth = new Map(elements.map((entry) => [entry.element, entry.depth]));
      const shown = visibleRects(
        context.win,
        elements.map((entry) => entry.element),
        MAX_BOXES,
      );
      context.layer.render(
        GROUP,
        shown.map(({ element, rect }) => ({
          rect: toRect(rect),
          color: DEPTH_COLORS[((depth.get(element) ?? 1) - 1) % DEPTH_COLORS.length] ?? '#e5262b',
          border: 1,
        })),
      );
    },
    clear(context: OverlayContext) {
      elements = [];
      context.layer.clear(GROUP);
    },
  };
}
