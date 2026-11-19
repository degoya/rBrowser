// Resolution in a target view (RB-020-01): a target from another view -> its element here ->
// the viewport point of its offset, scrolled into view when it is outside. The engine sends
// real input to that point (RB-020-03); `not_found` and `not_visible` mark the view desynced
// (RB-020-02).

import type { ElementSelector, ElementTarget, ResolveResult } from './protocol';
import { matchAll, type Root } from './selector';
import { isVisible } from './visible';

export { isVisible };

/** Why no element was found. */
export type Miss = 'not_found' | 'not_visible';

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

/** The matches of `selector` in `root`; the CSS path when the primary kind finds nothing. */
function candidates(selector: ElementSelector, root: Root): Element[] {
  const found = matchAll(selector, root);
  if (found.length > 0 || selector.kind === 'css') return found;
  return matchAll({ kind: 'css', path: selector.path }, root);
}

/**
 * The element `selector` means in `root`: its `nth` match when that is visible, else the first
 * visible match (a responsive twin, such as the mobile copy of a navigation link). With
 * `visibleOnly` false the `nth` match as it is (a form control set by script, RB-020-05).
 */
function pick(selector: ElementSelector, root: Root, visibleOnly: boolean): Element | Miss {
  const found = candidates(selector, root);
  if (found.length === 0) return 'not_found';
  const nth = 'nth' in selector && selector.nth ? selector.nth : 0;
  const preferred = found[nth] ?? found[0];
  if (preferred && (!visibleOnly || isVisible(preferred))) return preferred;
  return found.find(isVisible) ?? 'not_visible';
}

/**
 * The element `target` means in `doc`, through its shadow hosts; with `visibleOnly` false a
 * hidden one too (a visually hidden checkbox behind its styled label).
 */
export function locate(target: ElementTarget, doc: Document, visibleOnly = true): Element | Miss {
  let root: Root = doc;
  for (const host of target.hosts ?? []) {
    const found = pick(host, root, visibleOnly);
    if (typeof found === 'string') return found;
    if (!found.shadowRoot) return 'not_found';
    root = found.shadowRoot;
  }
  return pick(target.element, root, visibleOnly);
}

function point(element: Element, target: ElementTarget): { x: number; y: number } {
  const rect = element.getBoundingClientRect();
  return {
    x: rect.left + target.offset.x * rect.width,
    y: rect.top + target.offset.y * rect.height,
  };
}

/** Resolves `target` in `doc` to a point of the frame's viewport (CSS pixels). */
export function resolve(target: ElementTarget, doc: Document): ResolveResult {
  const element = locate(target, doc);
  if (typeof element === 'string') return { status: element };
  const view = doc.defaultView;
  let at = point(element, target);
  let scrolled = false;
  if (view && (at.x < 0 || at.y < 0 || at.x >= view.innerWidth || at.y >= view.innerHeight)) {
    element.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
    scrolled = true;
    at = point(element, target);
  }
  return { status: 'ok', x: round(at.x), y: round(at.y), scrolled };
}
