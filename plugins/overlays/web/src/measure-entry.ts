// The entry of assets/measure.js: the component evaluates it in each view (src/report.rs and
// src/scan.rs wrap it into one expression), and the bundle's global `__rbOverlaysMeasure` is
// this module: `measure` for `overlay_overflow`, `scan` for the checking overlays of
// `overlay_set`, `tabOrder` for `overlay_tab_order` and the tab order overlay (RB-070-12). The page script's own overlay host is left out, as the page script leaves it out.

import { measure as measureWith } from './measure';
import { HOST_TAG } from './constants';
import { type Finder, scanAnswer } from './overlays/scan';
import { findScrollbars } from './overlays/scrollbars';
import { PageSheet } from './overlays/sheet';
import { findSmallText } from './overlays/small-text';
import { findStacking } from './overlays/stacking';
import { findTargets } from './overlays/target-size';
import { tabEntries, tabList } from './overlays/tab-order/list';
import { findClipped, SPACING_CSS } from './overlays/text-spacing';
import { DEFAULT_SETTINGS, type Report } from './types';

export function measure(options: { show100vw?: boolean } = {}): Report {
  return measureWith({ show100vw: options.show100vw ?? true, exclude: document.querySelector(HOST_TAG) });
}

const FINDERS: Record<string, Finder> = {
  stacking: findStacking,
  scrollbars: findScrollbars,
  'text-spacing': findClipped,
  'target-size': findTargets,
  'small-text': findSmallText,
};

/** The hits of the checking overlay `overlay`: `{ count, items }`. Text spacing is applied for
 *  the scan and taken off again. */
export function scan(overlay: string, options: Record<string, unknown> = {}): { count: number; items: unknown[] } {
  const find = FINDERS[overlay];
  if (!find) return { count: 0, items: [] };
  const spacing = overlay === 'text-spacing' ? new PageSheet(SPACING_CSS) : undefined;
  spacing?.attach(document, window);
  try {
    return scanAnswer(window, find({ doc: document, win: window, settings: DEFAULT_SETTINGS, options }));
  } finally {
    spacing?.detach(document);
  }
}

/** The tab order of the page, measured: `{ count, items }`, each item `{ index, selector, path,
 *  tabindex, rect, warnings, iframe? }` in document px, the first `MAX_ENTRIES` of them. */
export function tabOrder(options: { includeIframes?: boolean } = {}): ReturnType<typeof tabEntries> {
  const items = tabList(document, window, { includeIframes: options.includeIframes ?? DEFAULT_SETTINGS.tabOrderIncludeIframes });
  return tabEntries(window, items);
}
