// What the page script reads of one element: its box, computed colours and font, the
// backgrounds of its ancestors, the sync agent's role and name and target. Shared by the page
// script (the tooltip) and the collector the MCP tool runs (`collect-entry.ts`).

import { nameOf, roleOf } from '../../../../agent/src/role';
import { centreTarget, cssPath } from '../../../../agent/src/selector';
import { isOpaque, toSrgb } from './color';
import type { PageData, Sides } from './types';

/** The most ancestors whose background is read. */
export const MAX_LAYERS = 64;
/** The most classes sent. */
export const MAX_CLASSES = 32;
/**
 * The longest texts sent: the page shapes them, and a request may not pass 64 KiB. The component
 * cuts again; a selector or target beyond its limit is left out (no CDP locator by selector, no
 * outline in the other views).
 */
export const MAX_TEXT = 256;
export const MAX_SELECTOR = 1024;
export const MAX_TARGET = 4096;

function cut(text: string): string {
  return text.length > MAX_TEXT ? text.slice(0, MAX_TEXT) : text;
}

function px(value: string): number {
  const number = Number.parseFloat(value);
  return Number.isFinite(number) ? number : 0;
}

function sides(style: CSSStyleDeclaration, prefix: string, suffix = ''): Sides {
  const side = (name: string) => px(style.getPropertyValue(`${prefix}-${name}${suffix}`));
  return [side('top'), side('right'), side('bottom'), side('left')];
}

/** The parent of `node` across a shadow boundary. */
export function composedParent(node: Element): Element | null {
  if (node.parentElement) return node.parentElement;
  const root = node.getRootNode();
  return root instanceof ShadowRoot ? root.host : null;
}

function backgrounds(element: Element, view: Window): PageData['backgrounds'] {
  const layers: PageData['backgrounds'] = [];
  for (let node: Element | null = element; node && layers.length < MAX_LAYERS; node = composedParent(node)) {
    const style = view.getComputedStyle(node);
    const color = toSrgb(style.backgroundColor || 'transparent');
    const image = (style.backgroundImage || 'none') !== 'none';
    layers.push({ color, image });
    if (isOpaque(color)) break;
  }
  return layers;
}

/** Reads `element`; `point` is where the pointer was (the element's centre without one). */
export function collect(element: Element, point?: { x: number; y: number }): PageData {
  const view = element.ownerDocument.defaultView ?? window;
  const style = view.getComputedStyle(element);
  const rect = element.getBoundingClientRect();
  const role = roleOf(element) ?? '';
  const inDocument = element.getRootNode() === element.ownerDocument;
  const selector = inDocument ? cssPath(element) : null;
  const target = centreTarget(element);
  return {
    tag: cut(element.localName),
    id: cut(element.id),
    classes: Array.from(element.classList).slice(0, MAX_CLASSES).map(cut),
    rect: { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
    dpr: view.devicePixelRatio || 1,
    box: {
      margin: sides(style, 'margin'),
      border: sides(style, 'border', '-width'),
      padding: sides(style, 'padding'),
    },
    color: toSrgb(style.color),
    backgrounds: backgrounds(element, view),
    font: {
      family: cut(style.fontFamily),
      size: px(style.fontSize),
      weight: Number.parseInt(style.fontWeight, 10) || 400,
      style: style.fontStyle,
      lineHeight: cut(style.lineHeight),
      letterSpacing: cut(style.letterSpacing),
    },
    role: cut(role),
    name: role ? cut(nameOf(element, role)) : '',
    selector: selector !== null && selector.length <= MAX_SELECTOR ? selector : null,
    point: point ?? { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 },
    scroll: { x: view.scrollX || 0, y: view.scrollY || 0 },
    target: JSON.stringify(target).length <= MAX_TARGET ? target : null,
  };
}

/** The innermost element at a viewport point, through open shadow roots. */
export function elementAt(doc: Document, x: number, y: number): Element | null {
  let element = doc.elementFromPoint(x, y);
  while (element?.shadowRoot) {
    const inner = element.shadowRoot.elementFromPoint(x, y);
    if (!inner || inner === element) break;
    element = inner;
  }
  return element;
}
