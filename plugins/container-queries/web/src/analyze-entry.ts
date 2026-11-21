// The entry of assets/analyze.js: the component evaluates it in each view (src/script.rs wraps
// it into one expression) and the bundle's global `__rbContainers` is this module - `analyze` for
// `containers_analyze`, `resize` and `reset` for `containers_resize`. The page script's own
// layer is left out, as the page script leaves it out.

import { analyze as analyzeDoc, findByPath, liveEnv } from './collect';
import { HOST_TAG } from './overlay';
import { resetAll, resetElement, resizeElement } from './resize';
import type { Report } from './types';

/** This view's containers and conditions. */
export function analyze(): Report {
  return analyzeDoc(document, liveEnv(window), { exclude: document.querySelector(HOST_TAG) });
}

/** Pins the width of the container at `path`; false when the page has no such element. */
export function resize(path: string, width: number): boolean {
  const element = findByPath(document, path);
  if (!element) return false;
  resizeElement(element, width);
  return true;
}

/** Resets the container at `path`, or every container without one; answers how many were reset. */
export function reset(path?: string | null): number {
  if (!path) return resetAll(document);
  const element = findByPath(document, path);
  return element && resetElement(element) ? 1 : 0;
}
