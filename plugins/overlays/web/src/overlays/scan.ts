// The checking overlays (RB-070-07): each is a finder - which elements to mark, with which label -
// drawn as boxes of the overlay layer. The same finders run in the page script (drawn, the count
// emitted to the panel) and in `assets/measure.js` (`__rbOverlaysMeasure.scan`, the component's
// scan for `overlay_set` and the findings).

import { cut, fitting } from '../answer';
import { cssPath } from '../selector';
import type { PageSettings, Rect } from '../types';
import { type Overlay, type OverlayContext, toRect } from './overlay';

/** One element found, with its label; `rect` where the box is not the element's own. */
export interface Hit {
  element: Element;
  label: string;
  rect?: () => Rect;
}

export interface FindContext {
  doc: Document;
  win: Window;
  settings: PageSettings;
  options: Record<string, unknown>;
}

export type Finder = (context: FindContext) => Hit[];

/** The most elements a finder looks at, and the most hits it keeps. */
export const MAX_ELEMENTS = 5000;
export const MAX_HITS = 1500;
/** The most hits a scan answers (src/scan.rs `MAX_ITEMS`). */
export const MAX_ITEMS = 200;

/** Every element below <body>, at most {@link MAX_ELEMENTS} (the overlay layer hangs below <html>). */
export function elementsBelowBody(doc: Document): Element[] {
  return doc.body ? Array.from(doc.body.querySelectorAll('*')).slice(0, MAX_ELEMENTS) : [];
}

/** A computed value, trimmed. */
export const value = (style: Pick<CSSStyleDeclaration, 'getPropertyValue'>, name: string): string =>
  style.getPropertyValue(name).trim();

/** A whole number option within `min..=max`, else `fallback`. */
export function intOption(options: Record<string, unknown>, name: string, min: number, max: number, fallback: number): number {
  const candidate = options[name];
  return typeof candidate === 'number' && Number.isInteger(candidate) && candidate >= min && candidate <= max ? candidate : fallback;
}

/** The element's full path from the root, one `tag:nth-of-type(n)` per level: the same element
 *  gets the same path in every view, so it is one finding. */
export function fullPath(element: Element): string {
  const parts: string[] = [];
  let node: Element | null = element;
  while (node) {
    const parent: Element | null = node.parentElement;
    const tag = node.localName;
    if (parent) {
      const same = Array.from(parent.children).filter((child) => child.localName === tag);
      parts.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag);
    } else {
      parts.unshift(tag);
    }
    node = parent;
  }
  const path = parts.join(' > ');
  return path.length > 300 ? path.slice(path.length - 300) : path;
}

function shows(win: Window, rect: Rect): boolean {
  if (rect.width <= 0 && rect.height <= 0) return false;
  return !(rect.y + rect.height < 0 || rect.y > win.innerHeight || rect.x + rect.width < 0 || rect.x > win.innerWidth);
}

/** What a scan answers: the whole count and the first hits, at most {@link MAX_ITEMS} and
 *  `MAX_ANSWER_BYTES` (../answer.ts), every text cut (RB-090-17), in document px. */
export function scanAnswer(win: Window, hits: Hit[]): { count: number; items: { selector: string; path: string; label: string; rect: Rect }[] } {
  const items = hits.slice(0, MAX_ITEMS).map((hit) => {
    const rect = hit.rect ? hit.rect() : toRect(hit.element.getBoundingClientRect());
    return {
      selector: cut(cssPath(hit.element)),
      path: cut(fullPath(hit.element)),
      label: cut(hit.label),
      rect: { x: rect.x + win.scrollX, y: rect.y + win.scrollY, width: rect.width, height: rect.height },
    };
  });
  return { count: hits.length, items: fitting(items) };
}

export interface ScanHooks {
  /** Switched on or its options changed, before the first find. */
  on?(context: OverlayContext, options: Record<string, unknown>): void;
  /** Before each find (a sheet the page dropped is put back). */
  refresh?(context: OverlayContext, options: Record<string, unknown>): void;
  /** Switched off, after the boxes are gone. */
  off?(context: OverlayContext): void;
}

/** An overlay that marks what `find` finds in `color` (at most {@link MAX_HITS}), telling the panel
 *  the whole count. */
export function createScanOverlay(id: string, color: string, find: Finder, hooks: ScanHooks = {}): Overlay {
  let options: Record<string, unknown> = {};
  let hits: Hit[] = [];
  let told = -1;

  return {
    id,
    update(context: OverlayContext, next: Record<string, unknown>) {
      options = next;
      hooks.on?.(context, options);
      this.measure(context);
    },
    measure(context: OverlayContext) {
      hooks.refresh?.(context, options);
      const found = find({ doc: context.doc, win: context.win, settings: context.settings, options });
      hits = found.slice(0, MAX_HITS);
      if (found.length !== told) {
        told = found.length;
        // The whole count; past MAX_HITS also how many are drawn (RB-090-17).
        context.emit({ type: 'hits', overlay: id, count: found.length, ...(found.length > hits.length ? { shown: hits.length } : {}) });
      }
      this.draw(context);
    },
    draw(context: OverlayContext) {
      const items = [];
      for (const hit of hits) {
        if (!hit.element.isConnected) continue;
        const rect = hit.rect ? hit.rect() : toRect(hit.element.getBoundingClientRect());
        if (!shows(context.win, rect)) continue;
        items.push({ rect, color, border: 2, halo: true, label: hit.label });
      }
      context.layer.render(id, items);
    },
    clear(context: OverlayContext) {
      hits = [];
      told = -1;
      context.layer.clear(id);
      hooks.off?.(context);
    },
  };
}
