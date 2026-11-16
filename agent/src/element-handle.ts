// An element handed over by the core (RB-090-53, "Force state"): the core resolves a node into
// this world (`DOM.resolveNode` with the agent's context) and asks for its target, then finds the
// same element in another view's agent - the selectors of the click sync (RB-020-01), without a
// pointer and without scrolling. Both are on the agent's frozen handle (`boot.ts`); the page's
// main world sees neither.

import { isTarget } from './agent';
import type { ElementTarget } from './protocol';
import { locate } from './resolve';
import { centreTarget } from './selector';

/** The target of `element` - the element itself, by its centre - or null for what is none. */
export function targetOf(element: unknown, win: Window): ElementTarget | null {
  // The world's own `Element`: the core's node is wrapped for this world.
  const ElementType = (win as unknown as { Element: typeof Element }).Element;
  if (!(element instanceof ElementType)) return null;
  try {
    return centreTarget(element);
  } catch {
    return null;
  }
}

/** The element `target` names in `doc`, a hidden one too (a submenu closed until the state is
 *  forced); null when it is missing or the target does not read. */
export function findTarget(target: unknown, doc: Document): Element | null {
  if (!isTarget(target)) return null;
  try {
    const found = locate(target, doc, false);
    return typeof found === 'string' ? null : found;
  } catch {
    return null;
  }
}
