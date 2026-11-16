// The contract of one overlay. The controller (../controller.ts) switches each registered overlay
// (./index.ts) on and off by the state, hands it new options, and asks it to measure (after a DOM
// change, a resize or a breakpoint change, throttled) and to draw (at every scroll frame). A new
// overlay (RB-070-07) is one module implementing this and one entry in ./index.ts and in the
// component's src/overlay.rs.

import type { Layer } from '../layer';
import type { PageSettings } from '../types';

export interface OverlayContext {
  readonly doc: Document;
  readonly win: Window;
  readonly layer: Layer;
  settings: PageSettings;
  /** An event for the plugin's panel (`rbPlugin.emit`). */
  emit(message: unknown): void;
  /** A request to the component (`rbPlugin.send`); failures are logged, not thrown. */
  send(message: unknown): void;
}

export interface Overlay {
  readonly id: string;
  /** Switched on, or its options changed. */
  update(context: OverlayContext, options: Record<string, unknown>): void;
  /** The page changed: find the elements again. */
  measure(context: OverlayContext): void;
  /** The page scrolled: draw the elements where they are now. */
  draw(context: OverlayContext): void;
  /** Switched off: remove everything drawn. */
  clear(context: OverlayContext): void;
  /** A post of the component that is no state (the tab order's live focus, RB-070-12). */
  receive?(context: OverlayContext, message: unknown): void;
}

/** The boxes of `elements` that show in the viewport, at most `limit`. */
export function visibleRects(
  win: Window,
  elements: readonly Element[],
  limit: number,
): { element: Element; rect: DOMRect }[] {
  const found: { element: Element; rect: DOMRect }[] = [];
  for (const element of elements) {
    if (found.length >= limit) break;
    if (!element.isConnected) continue;
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 && rect.height <= 0) continue;
    if (rect.bottom < 0 || rect.top > win.innerHeight || rect.right < 0 || rect.left > win.innerWidth) continue;
    found.push({ element, rect });
  }
  return found;
}

export const toRect = (rect: DOMRect) => ({ x: rect.left, y: rect.top, width: rect.width, height: rect.height });
