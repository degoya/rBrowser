// Test helpers: load a fixture page into happy-dom's document, size the viewport, and give every
// element a box (happy-dom lays nothing out, so every rect would be empty).

import { vi } from 'vitest';
import syncPage from '../../tests/fixtures/site/sync.html?raw';

/** happy-dom's window extension, for the viewport size. */
interface HappyWindow {
  happyDOM?: { setViewport(viewport: { width: number; height: number }): void };
}

/** Replaces the document with `html` (default: `tests/fixtures/site/sync.html`). */
export function loadPage(html: string = syncPage): void {
  const parsed = new DOMParser().parseFromString(html, 'text/html');
  document.head.innerHTML = parsed.head.innerHTML;
  document.body.innerHTML = parsed.body.innerHTML;
}

/** Sets the viewport (CSS pixels); media queries follow. */
export function setViewport(width: number, height = 800): void {
  (window as unknown as HappyWindow).happyDOM?.setViewport({ width, height });
}

/** Every element gets the box `rect` (default 100 x 40 at 10, 20). */
export function giveBoxes(rect = { left: 10, top: 20, width: 100, height: 40 }): void {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(
    () =>
      ({
        ...rect,
        x: rect.left,
        y: rect.top,
        right: rect.left + rect.width,
        bottom: rect.top + rect.height,
        toJSON: () => rect,
      }) as DOMRect,
  );
}

/** The element `selector` finds; throws when there is none. */
export function el<T extends Element = HTMLElement>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (!found) throw new Error(`no element for ${selector}`);
  return found;
}
