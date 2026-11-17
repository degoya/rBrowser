// The collector of the MCP tool `peek_inspect`: built into ../src/collect.js as the global
// `PeekCollect`, which the component runs through `evaluate` in each view - the page script's
// own data code, so the tool and the tooltip read the same.

import { collect, elementAt } from './collect';
import type { PageData } from './types';

export interface Query {
  selector?: string;
  x?: number;
  y?: number;
}

/** The page data of the element at `query`, or null when there is none (or the selector is bad). */
export function inspect(query: Query): PageData | null {
  let element: Element | null = null;
  try {
    if (typeof query.selector === 'string') {
      element = document.querySelector(query.selector);
    } else if (typeof query.x === 'number' && typeof query.y === 'number') {
      element = elementAt(document, query.x, query.y);
    }
  } catch {
    return null;
  }
  if (!element) return null;
  const point = typeof query.x === 'number' && typeof query.y === 'number' ? { x: query.x, y: query.y } : undefined;
  return collect(element, point);
}
