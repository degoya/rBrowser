// What fits into one row of the toolbar (RB-080-52). The row is a flex line: before this, every
// group refused to shrink and the address bar took the whole deficit, down to nothing, while the
// rest ran out of the window. Now the address bar keeps `ADDRESS_MIN`, the wide pulldowns shrink
// to an icon first (`compactOrder`), and what still does not fit leaves the row for the overflow
// menu (`overflowOrder`, the first id leaves first). Pure arithmetic over measured widths.

/** The address bar never gets narrower than this, in px (`min-w-60` in `AddressBar.vue`). */
export const ADDRESS_MIN = 240;
/** The row's gap between two items (`gap-2`). */
export const GAP = 8;
/** The row's padding left and right together (`px-2`). */
export const PADDING = 16;
/** The overflow menu's button, and a pulldown shrunk to its icon (`size-8`). */
export const ICON_WIDTH = 32;
/** One row of the toolbar, in px (`h-11`; `SHELL_HEIGHT` of the core). */
export const ROW_HEIGHT = 44;

/** The row's measures, each a rem class above, in px at a text size. */
export interface FitMetrics {
  addressMin: number;
  gap: number;
  padding: number;
  iconWidth: number;
}

/** The measures at `scale`, one rem over its 16 px (Windows' text size, RB-090-26). */
export function fitMetrics(scale = 1): FitMetrics {
  return {
    addressMin: ADDRESS_MIN * scale,
    gap: GAP * scale,
    padding: PADDING * scale,
    iconWidth: ICON_WIDTH * scale,
  };
}

/** How the toolbar treats what does not fit: `ui.toolbar.overflow`. */
export type OverflowMode = 'menu' | 'wrap';

/** A group of the row as measured. */
export interface FitItem {
  id: string;
  /** Its full width; 0 while it renders nothing. */
  width: number;
  /** What it gives up when its pulldown shrinks to an icon; 0 when it cannot. */
  saving: number;
}

export interface FitRow {
  /** The row's width. */
  available: number;
  /** What is always there, with its gaps and the padding; each item adds its width and a gap. */
  fixed: number;
  /** The overflow button's width; 0 for a row that has none. */
  menu: number;
  items: FitItem[];
  /** The groups whose pulldown shrinks to an icon, in this order. */
  compactOrder: readonly string[];
  /** The groups that leave the row, the first one first. */
  overflowOrder: readonly string[];
  /** The overflow button is there whatever this row hides (another row hid something). */
  menuForced?: boolean;
  /** Groups that stay in the row whatever it costs (one whose control just opened something). */
  keep?: readonly string[];
  /** The gap between two items; [`GAP`] by default, more at a larger text size. */
  gap?: number;
}

export interface FitResult {
  compact: string[];
  hidden: string[];
  /** The overflow button is part of the row. */
  menu: boolean;
  /** `false`: even with everything hidden the row is wider than `available`. */
  fits: boolean;
}

/** Fits `row`: nothing changed while it fits, then icons for the pulldowns, then the menu. */
export function fitRow(row: FitRow): FitResult {
  const items = row.items.filter((item) => item.width > 0);
  const compact = new Set<string>();
  const hidden = new Set<string>();
  let menu = row.menuForced === true && row.menu > 0;
  const gap = row.gap ?? GAP;

  const fits = (): boolean => {
    let used = row.fixed + (menu ? row.menu + gap : 0);
    for (const item of items) {
      if (!hidden.has(item.id)) {
        used += item.width - (compact.has(item.id) ? item.saving : 0) + gap;
      }
    }
    return used <= row.available;
  };
  const result = (ok: boolean): FitResult => ({
    compact: [...compact].filter((id) => !hidden.has(id)),
    hidden: items.filter((item) => hidden.has(item.id)).map((item) => item.id),
    menu,
    fits: ok,
  });

  if (fits()) {
    return result(true);
  }
  for (const id of row.compactOrder) {
    const item = items.find((candidate) => candidate.id === id);
    if (item !== undefined && item.saving > 0) {
      compact.add(id);
      if (fits()) {
        return result(true);
      }
    }
  }
  menu = row.menu > 0;
  // A group no order names leaves last, the rightmost first.
  const unnamed = items.map((item) => item.id).filter((id) => !row.overflowOrder.includes(id));
  for (const id of [...row.overflowOrder, ...unnamed.reverse()]) {
    if (items.some((item) => item.id === id) && row.keep?.includes(id) !== true) {
      hidden.add(id);
      if (fits()) {
        return result(true);
      }
    }
  }
  return result(false);
}
