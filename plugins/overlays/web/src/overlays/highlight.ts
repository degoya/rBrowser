// The element highlight: every element the selector matches framed, the current match (the
// `index` option, wrapped around the count) emphasised, labelled `<n>/<count>` and scrolled into
// view when the index or the selector changes. The count goes to the panel.

import { type Overlay, type OverlayContext, toRect, visibleRects } from './overlay';

const GROUP = 'highlight';
/** The most matches the overlay follows. */
export const MAX_MATCHES = 1000;

/** `index` wrapped into `0..count`. */
export function wrap(index: number, count: number): number {
  return count === 0 ? 0 : ((index % count) + count) % count;
}

/** The elements `selector` matches, without the overlay's own host; none for a bad selector. */
export function find(doc: Document, selector: string, exclude?: Element | null): { matches: Element[]; error?: string } {
  if (!selector) return { matches: [] };
  try {
    const matches = Array.from(doc.querySelectorAll(selector)).filter((element) => element !== exclude);
    return { matches: matches.slice(0, MAX_MATCHES) };
  } catch {
    return { matches: [], error: 'invalid' };
  }
}

export function createHighlight(): Overlay {
  let selector = '';
  let index = 0;
  let matches: Element[] = [];
  let told = '';

  const current = (): Element | undefined => matches[wrap(index, matches.length)];

  return {
    id: 'highlight',
    update(context: OverlayContext, options: Record<string, unknown>) {
      const nextSelector = typeof options.selector === 'string' ? options.selector : '';
      const nextIndex = typeof options.index === 'number' && Number.isFinite(options.index) ? Math.trunc(options.index) : 0;
      const moved = nextSelector !== selector || nextIndex !== index || matches.length === 0;
      selector = nextSelector;
      index = nextIndex;
      this.measure(context);
      const target = current();
      if (moved && target) target.scrollIntoView({ block: 'center', inline: 'nearest' });
    },
    measure(context: OverlayContext) {
      const found = find(context.doc, selector, context.layer.host);
      const count = found.matches.length;
      const news = `${selector}|${count}|${wrap(index, count)}|${found.error ?? ''}`;
      if (news !== told) {
        told = news;
        context.emit({ type: 'matches', selector, count, index: wrap(index, count), ...(found.error ? { error: found.error } : {}) });
      }
      matches = found.matches;
      this.draw(context);
    },
    draw(context: OverlayContext) {
      const color = context.settings.colors.highlight;
      const active = current();
      const shown = visibleRects(context.win, matches, MAX_MATCHES);
      context.layer.render(
        GROUP,
        shown.map(({ element, rect }) =>
          element === active
            ? { rect: toRect(rect), color, border: 3, fill: true, halo: true, label: `${wrap(index, matches.length) + 1}/${matches.length}` }
            : { rect: toRect(rect), color, border: 2, halo: true },
        ),
      );
    },
    clear(context: OverlayContext) {
      matches = [];
      told = '';
      selector = '';
      context.layer.clear(GROUP);
    },
  };
}
