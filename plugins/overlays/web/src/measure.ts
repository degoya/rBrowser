// The overflow measurement, shared by the page script (which draws it) and `overlay_overflow`
// (which evaluates it in each view, src/report.rs):
//
// - the document scrolls horizontally when the root's scrollWidth exceeds its clientWidth;
// - a cause is an element whose right edge passes the viewport, not clipped by an ancestor
//   (`overflow-x` other than `visible`), not fixed (a fixed box never scrolls the document) -
//   the outermost one only, so a wide element is named and not its children;
// - a `100vw` element is one a stylesheet or its own style gives `width`/`min-width: 100vw`, or,
//   beside a classic scrollbar, one exactly as wide as the window (scrollbar included).

import { cssPath } from './selector';
import type { Mark, Report } from './types';

/** The most causes and 100vw elements a report names (src/report.rs `MAX_MARKS`). */
export const MAX_MARKS = 50;
const CLIPPING = /^(hidden|clip|scroll|auto)$/;
/** Sub-pixel layout: an edge this close to the viewport's is inside it. */
const TOLERANCE = 0.5;

/** The part of `window` the measurement reads; a test hands its own. */
export type MeasureWindow = Pick<Window, 'innerWidth' | 'scrollX' | 'getComputedStyle' | 'matchMedia'>;

export interface MeasureOptions {
  show100vw: boolean;
  /** An element the measurement leaves out with its subtree: the overlay's own host. */
  exclude?: Element | null;
}

export interface Measured {
  report: Report;
  causes: Element[];
  vw100: Element[];
}

function mark(element: Element, scrollX: number, scrollY: number, overhang?: number): Mark {
  const rect = element.getBoundingClientRect();
  const found: Mark = {
    selector: cssPath(element),
    rect: { x: rect.left + scrollX, y: rect.top + scrollY, width: rect.width, height: rect.height },
  };
  if (overhang !== undefined) found.overhang = overhang;
  return found;
}

function findCauses(body: Element, limit: number, win: MeasureWindow, exclude?: Element | null): Element[] {
  const found: Element[] = [];
  const visit = (element: Element): void => {
    if (found.length >= MAX_MARKS || element === exclude) return;
    const style = win.getComputedStyle(element);
    if (style.display === 'none' || style.position === 'fixed') return;
    const rect = element.getBoundingClientRect();
    if (rect.width > 0 && rect.right + win.scrollX > limit + TOLERANCE) {
      found.push(element);
      return;
    }
    if (element !== body && CLIPPING.test(style.overflowX)) return;
    for (const child of Array.from(element.children)) visit(child);
  };
  visit(body);
  return found;
}

const is100vw = (value: string | null | undefined): boolean =>
  (value ?? '').replace(/\s+/g, '').toLowerCase() === '100vw';

/** The selectors of the rules that apply now and set `width` or `min-width` to 100vw. */
function ruleSelectors(doc: Document, win: MeasureWindow): string[] {
  const selectors: string[] = [];
  const walk = (rules: CSSRuleList, depth: number): void => {
    for (const rule of Array.from(rules)) {
      const styled = rule as CSSStyleRule;
      if (typeof styled.selectorText === 'string' && styled.style) {
        if (is100vw(styled.style.getPropertyValue('width')) || is100vw(styled.style.getPropertyValue('min-width'))) {
          selectors.push(styled.selectorText);
        }
      }
      const grouping = rule as CSSGroupingRule & { media?: MediaList };
      if (depth < 8 && grouping.cssRules) {
        if (grouping.media && grouping.media.mediaText && !win.matchMedia(grouping.media.mediaText).matches) continue;
        walk(grouping.cssRules, depth + 1);
      }
    }
  };
  for (const sheet of Array.from(doc.styleSheets)) {
    let rules: CSSRuleList;
    try {
      rules = sheet.cssRules;
    } catch {
      continue; // a cross-origin sheet: the geometry below still finds its elements
    }
    walk(rules, 0);
  }
  return selectors;
}

function visible(element: Element): boolean {
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}

function find100vw(doc: Document, win: MeasureWindow, clientWidth: number, exclude?: Element | null): Element[] {
  const found = new Set<Element>();
  const inside = (element: Element): boolean => !exclude || !exclude.contains(element);
  const add = (element: Element): void => {
    if (found.size < MAX_MARKS && inside(element) && visible(element)) found.add(element);
  };
  for (const selector of ruleSelectors(doc, win)) {
    try {
      doc.querySelectorAll(selector).forEach(add);
    } catch {
      // a selector querySelectorAll does not take (a pseudo-element, a nested rule)
    }
  }
  doc.querySelectorAll('[style*="vw"]').forEach((element) => {
    const style = (element as HTMLElement).style;
    if (style && (is100vw(style.width) || is100vw(style.minWidth))) add(element);
  });
  if (win.innerWidth - clientWidth >= 1 && doc.body) {
    for (const element of Array.from(doc.body.querySelectorAll('*'))) {
      if (found.size >= MAX_MARKS) break;
      if (Math.abs(element.getBoundingClientRect().width - win.innerWidth) < TOLERANCE) add(element);
    }
  }
  return [...found];
}

/** The measurement with the elements it names. */
export function measureElements(doc: Document, win: MeasureWindow, options: MeasureOptions): Measured {
  const root = doc.documentElement;
  const clientWidth = root.clientWidth;
  const scrollWidth = root.scrollWidth;
  const hasHorizontalScroll = scrollWidth > clientWidth;
  const scrollY = (win as Partial<Window>).scrollY ?? 0;
  const causes = hasHorizontalScroll && doc.body ? findCauses(doc.body, clientWidth, win, options.exclude) : [];
  const vw100 = options.show100vw ? find100vw(doc, win, clientWidth, options.exclude) : [];
  const report: Report = {
    hasHorizontalScroll,
    scrollWidth,
    clientWidth,
    innerWidth: win.innerWidth,
    causes: causes.map((element) => {
      const right = element.getBoundingClientRect().right + win.scrollX;
      return mark(element, win.scrollX, scrollY, Math.round(right - clientWidth));
    }),
    vw100: vw100.map((element) => mark(element, win.scrollX, scrollY)),
  };
  return { report, causes, vw100 };
}

/** The report alone: what `overlay_overflow` answers. */
export function measure(options: Partial<MeasureOptions> = {}, doc: Document = document, win: MeasureWindow = window): Report {
  return measureElements(doc, win, { show100vw: options.show100vw ?? true, exclude: options.exclude ?? null }).report;
}
