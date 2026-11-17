// Focus tracking: a clear ring with a label on the focused element (into open shadow roots),
// also where the page removes its `outline`. The ring is a box of the overlay layer; the page's
// styles stay as they are.

import { type Overlay, type OverlayContext, toRect } from './overlay';

const GROUP = 'focus-tracking';
const COLOR = '#ff6a00';

/** The focused element, followed into open shadow roots; none for the body or the root. */
export function deepActive(doc: Document): Element | null {
  let active: Element | null = doc.activeElement;
  while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
  return active && active !== doc.body && active !== doc.documentElement ? active : null;
}

/** A short name of `element`: its tag, id and first class. */
export function describe(element: Element): string {
  const id = element.id ? `#${element.id}` : '';
  const first = element.classList[0];
  return `${element.localName}${id}${first ? `.${first}` : ''}`.slice(0, 60);
}

export function createFocusTracking(): Overlay {
  let listening: { doc: Document; listener: () => void } | undefined;

  return {
    id: 'focus-tracking',
    update(context: OverlayContext) {
      if (!listening) {
        const listener = (): void => this.draw(context);
        context.doc.addEventListener('focusin', listener, true);
        context.doc.addEventListener('focusout', listener, true);
        listening = { doc: context.doc, listener };
      }
      this.draw(context);
    },
    measure(context: OverlayContext) {
      this.draw(context);
    },
    draw(context: OverlayContext) {
      // A focusout fires before the next element has focus: draw what holds it after the event.
      const active = deepActive(context.doc);
      context.layer.render(
        GROUP,
        active ? [{ rect: toRect(active.getBoundingClientRect()), color: COLOR, border: 3, halo: true, label: `focus ${describe(active)}` }] : [],
      );
    },
    clear(context: OverlayContext) {
      if (listening) {
        listening.doc.removeEventListener('focusin', listening.listener, true);
        listening.doc.removeEventListener('focusout', listening.listener, true);
        listening = undefined;
      }
      context.layer.clear(GROUP);
    },
  };
}
