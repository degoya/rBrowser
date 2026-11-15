// Marks where a font is used (RB-070-13): a click in the Fonts panel outlines every element whose
// text is set in the family (optionally one weight and style) in each view, in one host element
// `rbrowser-fonts` with a closed shadow root - the sync agent ignores events inside `rbrowser-*`,
// the page sees neither the boxes nor their styles, and nothing catches the pointer. The
// component runs it through `evaluate` in the plugin's isolated world (src/collect-entry.ts).

import { firstFamily, weightOf } from './faces';
import type { Box, MarkRequest } from './types';

/** The host element's name. */
export const HOST = 'rbrowser-fonts';
/** The most elements outlined. */
export const MAX_MARKS = 200;
const COLOR = '#d9480f';
const HOST_STYLE = 'position:absolute;left:0;top:0;width:0;height:0;z-index:2147483647;pointer-events:none;';

/** The page as the marker reads it. */
export interface MarkEnv {
  /** The element's box in CSS px of the document. */
  rect(element: Element): Box;
  shown(element: Element): boolean;
  scrollTo(element: Element): void;
}

export const realMarkEnv: MarkEnv = {
  rect(element) {
    const rect = element.getBoundingClientRect();
    return [rect.x + window.scrollX, rect.y + window.scrollY, rect.width, rect.height];
  },
  shown: (element) => element.getClientRects().length > 0,
  scrollTo: (element) => element.scrollIntoView({ block: 'center', inline: 'nearest' }),
};

/** Removes the marks. */
export function clear(): void {
  for (const host of Array.from(document.querySelectorAll(HOST))) host.remove();
}

function hasOwnText(element: Element): boolean {
  if (/^(input|textarea|select)$/.test(element.localName)) return true;
  return Array.from(element.childNodes).some((node) => node.nodeType === Node.TEXT_NODE && (node.textContent ?? '').trim() !== '');
}

/** The elements `request` names, in document order. */
export function matches(request: MarkRequest, env: MarkEnv): Element[] {
  const family = (request.family ?? '').trim().toLowerCase();
  if (!family) return [];
  const found: Element[] = [];
  const root = document.body ?? document.documentElement;
  for (const element of Array.from(root.querySelectorAll('*'))) {
    if (found.length >= MAX_MARKS) break;
    if (element.closest(HOST) || !hasOwnText(element) || !env.shown(element)) continue;
    const style = getComputedStyle(element);
    if (firstFamily(style.fontFamily).toLowerCase() !== family) continue;
    if (typeof request.weight === 'number' && weightOf(style.fontWeight) !== request.weight) continue;
    if (typeof request.style === 'string' && (style.fontStyle || 'normal').toLowerCase() !== request.style) continue;
    found.push(element);
  }
  return found;
}

/** Outlines the elements `request` names (the old marks replaced) and scrolls the first into
 * view; answers how many. */
export function mark(request: MarkRequest, env: MarkEnv = realMarkEnv): { count: number } {
  clear();
  const found = matches(request, env);
  if (found.length === 0) return { count: 0 };
  const host = document.createElement(HOST);
  host.setAttribute('style', HOST_STYLE);
  const root = host.attachShadow({ mode: 'closed' });
  for (const element of found) {
    const [x, y, width, height] = env.rect(element);
    const box = document.createElement('div');
    box.setAttribute(
      'style',
      `position:absolute;left:${x - 2}px;top:${y - 2}px;width:${width + 4}px;height:${height + 4}px;` +
        `outline:2px solid ${COLOR};background:rgba(217,72,15,0.08);box-sizing:border-box;`,
    );
    root.append(box);
  }
  document.documentElement.append(host);
  const first = found[0];
  if (first) env.scrollTo(first);
  return { count: found.length };
}
