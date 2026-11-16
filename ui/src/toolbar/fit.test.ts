import { describe, expect, it } from 'vitest';
import { ADDRESS_MIN, fitMetrics, fitRow, GAP, ICON_WIDTH, PADDING, type FitItem, type FitRow } from './fit';
import { COMPACT_ORDER, OVERFLOW_ORDER } from './groups';

/** The owner's toolbar of 2026-11-14 as measured widths, in DOM order. */
const ITEMS: FitItem[] = [
  { id: 'sync', width: 76, saving: 0 },
  { id: 'layout', width: 392, saving: 128 },
  { id: 'environment', width: 176, saving: 128 },
  { id: 'bookmarks', width: 190, saving: 128 },
  { id: 'emulation', width: 142, saving: 0 },
  { id: 'minimap-end', width: 144, saving: 0 },
  { id: 'analyze', width: 28, saving: 0 },
  { id: 'settings', width: 28, saving: 0 },
];
/** Logo 96, navigation 88, the address bar's minimum, their gaps and the padding. */
const FIXED = PADDING + 96 + 88 + ADDRESS_MIN + 2 * GAP;

function row(available: number, more: Partial<FitRow> = {}): FitRow {
  return {
    available,
    fixed: FIXED,
    menu: ICON_WIDTH,
    items: ITEMS,
    compactOrder: COMPACT_ORDER,
    overflowOrder: OVERFLOW_ORDER,
    ...more,
  };
}

/** The row's width with `result` applied: never more than there is. */
function used(input: FitRow, result: ReturnType<typeof fitRow>): number {
  let width = input.fixed + (result.menu ? input.menu + GAP : 0);
  for (const item of input.items) {
    if (item.width > 0 && !result.hidden.includes(item.id)) {
      width += item.width - (result.compact.includes(item.id) ? item.saving : 0) + GAP;
    }
  }
  return width;
}

describe('fitRow', () => {
  it('changes nothing while everything fits', () => {
    expect(fitRow(row(2400))).toEqual({ compact: [], hidden: [], menu: false, fits: true });
  });

  it('shrinks the wide selects to icons before anything leaves the row', () => {
    const result = fitRow(row(1600));
    expect(result.hidden).toEqual([]);
    expect(result.menu).toBe(false);
    expect(result.compact).toEqual(['bookmarks']);
    const narrower = fitRow(row(1400));
    expect(narrower.hidden).toEqual([]);
    expect(narrower.compact).toEqual(['bookmarks', 'environment', 'layout']);
  });

  it('sends the canvas minimap into the menu first', () => {
    const result = fitRow(row(1200));
    expect(result.hidden).toEqual(['minimap-end']);
    expect(result.menu).toBe(true);
  });

  it.each([1800, 1400, 1100, 800, 640])('keeps the address bar its minimum at %i px', (available) => {
    const input = row(available);
    const result = fitRow(input);
    expect(result.fits).toBe(true);
    // `fixed` holds the address bar's 240 px: within `available`, nothing is cut off.
    expect(used(input, result)).toBeLessThanOrEqual(available);
    // What left the row is in the menu.
    expect(result.menu).toBe(result.hidden.length > 0);
  });

  it('says so when not even navigation, address bar and the menu fit', () => {
    const result = fitRow(row(300));
    expect(result.fits).toBe(false);
    expect(result.hidden).toHaveLength(ITEMS.length);
    expect(result.menu).toBe(true);
  });

  it('ignores a group that renders nothing', () => {
    const input = row(2400, { items: [...ITEMS, { id: 'trust', width: 0, saving: 0 }] });
    expect(fitRow(input)).toEqual({ compact: [], hidden: [], menu: false, fits: true });
  });

  it('keeps the menu a row was told to have, and gives a row without a menu none', () => {
    expect(fitRow(row(2400, { menuForced: true })).menu).toBe(true);
    const second = fitRow(row(300, { menu: 0 }));
    expect(second.menu).toBe(false);
  });

  it('keeps a group it is told to keep and hides the next ones instead', () => {
    const result = fitRow(row(1210, { keep: ['minimap-end'] }));
    expect(result.fits).toBe(true);
    expect(result.hidden).not.toContain('minimap-end');
    expect(result.hidden).toEqual(['emulation']);
  });

  it('hides a group no order names after the named ones', () => {
    const input = row(700, { items: [...ITEMS, { id: 'new-group', width: 40, saving: 0 }] });
    const result = fitRow(input);
    expect(result.fits).toBe(true);
    expect(result.hidden).toContain('minimap-end');
  });

  it("grows its measures with Windows' text size (RB-090-26)", () => {
    expect(fitMetrics()).toEqual({ addressMin: ADDRESS_MIN, gap: GAP, padding: PADDING, iconWidth: ICON_WIDTH });
    expect(fitMetrics(1.5)).toEqual({ addressMin: 360, gap: 12, padding: 24, iconWidth: 48 });
    expect(fitMetrics(2.25)).toEqual({ addressMin: 540, gap: 18, padding: 36, iconWidth: 72 });
  });

  it('counts the gap it is given, so a larger text size leaves more for the menu', () => {
    // The owner's row at 150 %: every width and gap half again as large.
    const m = fitMetrics(1.5);
    const items = ITEMS.map((item) => ({ ...item, width: item.width * 1.5, saving: item.saving * 1.5 }));
    const fixed = m.padding + 96 * 1.5 + 88 * 1.5 + m.addressMin + 2 * m.gap;
    const input = row(1210 * 1.5, { items, fixed, menu: m.iconWidth, gap: m.gap });
    expect(fitRow(input)).toEqual(fitRow(row(1210)));
  });
});
