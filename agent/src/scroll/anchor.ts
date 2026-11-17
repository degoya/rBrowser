// Scroll anchors (RB-020-04). The same page is much taller in a narrow view than in a wide one,
// so a pixel or percent position lands on other content there. The anchor names the content
// instead: the element at the top edge of a scroll container's visible area and the share of
// it above that edge. Cheap enough for every frame of a scroll: one hit test, one walk up the
// tree, a CSS path - no accessible names, no text matching.

import type { ElementSelector, ElementTarget } from '../protocol';
import { cssPath, isGeneratedId, rootOf } from '../selector';

/** A scroll container: the document or an element. */
export type Scroller = Document | Element;

/** A visible area in CSS px of the frame's viewport. */
export interface Area {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Where a scroller stands and how far it can go. */
export interface Position {
  x: number;
  y: number;
  maxX: number;
  maxY: number;
}

/** How far below the top edge the hit test looks. */
const PROBE_INSET = 1;
/** Where across the visible width the hit test looks, in this order. */
const PROBES = [0.5, 0.25, 0.75, 0.1];
/** Tags that make a good anchor: headings, sections, list items, rows, paragraphs. */
const PREFERRED =
  /^(h[1-6]|section|article|header|footer|aside|nav|li|tr|p|figure|pre|blockquote|dt|dd|table|img|form|fieldset)$/;

function round(value: number): number {
  return Math.round(value * 1_000_000) / 1_000_000;
}

/** The element whose scroll offsets are the document's. */
function pageElement(doc: Document): Element {
  return doc.scrollingElement ?? doc.documentElement;
}

/** The visible area of `scroller`: the frame's viewport, or an element's padding box. */
export function viewportOf(scroller: Scroller): Area {
  if (scroller instanceof Element) {
    const rect = scroller.getBoundingClientRect();
    return {
      left: rect.left + scroller.clientLeft,
      top: rect.top + scroller.clientTop,
      width: scroller.clientWidth,
      height: scroller.clientHeight,
    };
  }
  const view = scroller.defaultView;
  return { left: 0, top: 0, width: view?.innerWidth ?? 0, height: view?.innerHeight ?? 0 };
}

/** The scroll offsets of `scroller` and their maximum. */
export function positionOf(scroller: Scroller): Position {
  const element = scroller instanceof Element ? scroller : pageElement(scroller);
  const view = scroller instanceof Element ? null : scroller.defaultView;
  return {
    x: view ? view.scrollX : element.scrollLeft,
    y: view ? view.scrollY : element.scrollTop,
    maxX: Math.max(0, element.scrollWidth - element.clientWidth),
    maxY: Math.max(0, element.scrollHeight - element.clientHeight),
  };
}

/** Whether `element` scrolls on its own (an inner scroll container). */
export function isScroller(element: Element, style: CSSStyleDeclaration): boolean {
  // The longhands, else the shorthand (engines that do not expand it).
  const scrolls = (overflow: string) =>
    /^(auto|scroll)$/.test(overflow || style.overflow || '');
  return (
    (scrolls(style.overflowY) && element.scrollHeight > element.clientHeight) ||
    (scrolls(style.overflowX) && element.scrollWidth > element.clientWidth)
  );
}

/** Whether `element` is a good anchor by itself. */
function preferred(element: Element): boolean {
  if (element.id && !isGeneratedId(element.id)) return true;
  if (element.hasAttribute('data-testid')) return true;
  return PREFERRED.test(element.localName);
}

/** Whether `element` is outside `scroller`'s content (the container itself, body, html). */
function outside(element: Element, scroller: Scroller): boolean {
  if (scroller instanceof Element) return element === scroller || !scroller.contains(element);
  return element === scroller.documentElement || element === scroller.body;
}

/**
 * The anchor for a hit on `hit` in `scroller`: the innermost preferred element from `hit` up,
 * else `hit` itself. Content of an inner scroll container moves with that container's own
 * scroll, so it never anchors an outer scroller: the container (or a preferred element above it)
 * does. Null when the hit is fixed or sticky (it does not move with the scroll) or outside the
 * scroller.
 */
export function anchorAbove(hit: Element, scroller: Scroller): Element | null {
  if (outside(hit, scroller)) return null;
  const view = hit.ownerDocument.defaultView;
  if (!view) return null;
  let candidate: Element | null = null;
  let first: Element | null = hit;
  for (let node: Element | null = hit; node && !outside(node, scroller); node = node.parentElement) {
    const style = view.getComputedStyle(node);
    if (style.position === 'fixed' || style.position === 'sticky') return null;
    if (node !== hit && isScroller(node, style)) {
      candidate = null;
      first = node;
    }
    if (!candidate && preferred(node)) candidate = node;
  }
  return candidate ?? first;
}

/** The anchor at the top edge of `scroller`'s visible area; null when nothing qualifies. */
export function chooseAnchor(scroller: Scroller): Element | null {
  const doc = scroller instanceof Element ? scroller.ownerDocument : scroller;
  const area = viewportOf(scroller);
  if (area.width <= 0 || area.height <= 0 || typeof doc.elementsFromPoint !== 'function') {
    return null;
  }
  const y = area.top + Math.min(PROBE_INSET, area.height / 2);
  for (const share of PROBES) {
    for (const hit of doc.elementsFromPoint(area.left + area.width * share, y)) {
      const anchor = anchorAbove(hit, scroller);
      if (anchor) return anchor;
    }
  }
  return null;
}

/** A cheap, stable selector for an anchor: its test id, else its CSS path. */
function anchorSelector(element: Element): ElementSelector {
  const path = cssPath(element);
  const testid = element.getAttribute('data-testid');
  if (!testid) return { kind: 'css', path };
  const all = Array.from(rootOf(element).querySelectorAll(`[data-testid="${CSS.escape(testid)}"]`));
  const nth = all.indexOf(element);
  return nth > 0 ? { kind: 'testid', testid, nth, path } : { kind: 'testid', testid, path };
}

/**
 * `anchor` as the scroll event carries it: its selector and the point of it at the top-left
 * corner of `scroller`'s visible area, as shares of its width and height.
 */
export function anchorTarget(anchor: Element, scroller: Scroller): ElementTarget {
  const rect = anchor.getBoundingClientRect();
  const area = viewportOf(scroller);
  return {
    element: anchorSelector(anchor),
    offset: {
      x: rect.width > 0 ? round((area.left - rect.left) / rect.width) : 0,
      y: rect.height > 0 ? round((area.top - rect.top) / rect.height) : 0,
    },
  };
}
