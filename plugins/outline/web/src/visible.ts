// Whether an element is exposed at all and whether it is on the screen - the two facts the
// outline compares across views ("only visible on desktop"). The geometry comes through `Env`,
// so the tests (happy-dom has no layout) give it and the page uses the real one.

import type { Box } from './types';

/** The page's geometry as the collector reads it. */
export interface Env {
  /** The element's box in CSS px of the viewport. */
  rect(element: Element): Box;
  /** The width the page can scroll to, in CSS px. */
  pageWidth(): number;
}

export const realEnv: Env = {
  rect(element) {
    const rect = element.getBoundingClientRect();
    return [rect.x, rect.y, rect.width, rect.height];
  },
  pageWidth() {
    return Math.max(document.documentElement.scrollWidth, window.innerWidth);
  },
};

/** Rounded to whole CSS px, finite. */
export function round(box: Box): Box {
  return box.map((value) => (Number.isFinite(value) ? Math.round(value) : 0)) as Box;
}

function styleHides(element: Element): boolean {
  const style = getComputedStyle(element);
  return style.display === 'none' || style.contentVisibility === 'hidden';
}

/** Inside a closed `<details>`, apart from its `<summary>`. */
function inClosedDetails(element: Element, parent: Element): boolean {
  return (
    parent.localName === 'details' &&
    !(parent as HTMLDetailsElement).open &&
    !(element.localName === 'summary' && element.parentElement === parent)
  );
}

/** Not exposed at all: hidden by `display`, `visibility`, `hidden`, `inert` or a closed
 * `<details>`, on itself or an ancestor. */
export function isHidden(element: Element): boolean {
  const visibility = getComputedStyle(element).visibility;
  if (visibility === 'hidden' || visibility === 'collapse') return true;
  for (let node: Element | null = element; node; node = node.parentElement) {
    if (node.hasAttribute('hidden') || node.hasAttribute('inert') || styleHides(node)) return true;
    const parent = node.parentElement;
    if (parent && inClosedDetails(node, parent)) return true;
  }
  return false;
}

/** On the screen: exposed, with a box of some size that is not pushed off the page sideways
 * (an off-canvas menu). Without any geometry (no layout) the box does not count. */
export function isVisible(element: Element, hidden: boolean, box: Box, env: Env): boolean {
  if (hidden) return false;
  const [x, , width, height] = box;
  if (width <= 0 && height <= 0) return !hasLayout(env);
  const right = x + width;
  return right > 0 && x < env.pageWidth();
}

function hasLayout(env: Env): boolean {
  return env.pageWidth() > 0 && env.rect(document.documentElement)[2] > 0;
}
