import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import type { JsonValue } from '../ipc/protocol';
import { countReferences, removalNote } from './breakpointRefs';

const VALUES: Record<string, JsonValue> = {
  layout: {
    columns: 3,
    rows: 1,
    cells: [
      { breakpoint: 'mobile', col: 1, row: 1 },
      { breakpoint: 'tablet', col: 2, row: 1 },
      { breakpoint: 'tablet', col: 3, row: 1 },
    ],
    hidden: ['tablet'],
    modes: { split: { primary: ['tablet'], secondary: ['mobile'] }, canvas: { items: [{ breakpoint: 'tablet', x: 0, y: 0 }] } },
  },
  layouts: {
    presets: {
      pair: { mode: 'grid', layout: { columns: 1, rows: 1, cells: [{ breakpoint: 'tablet', col: 1, row: 1 }] }, hidden: ['tablet'] },
      big: { mode: 'focus', focus: 'tablet' },
      side: { mode: 'split', secondary: ['tablet'] },
    },
  },
  'envCompare.breakpoints': ['tablet', 'mobile'],
};

const read = (key: string): JsonValue | undefined => VALUES[key];

describe('what removing a breakpoint takes along (RB-090-61)', () => {
  it('counts the places rb_config::breakpoint_refs removes', () => {
    expect(countReferences(read, 'tablet')).toEqual({ cells: 3, hidden: 2, other: 5 });
    expect(countReferences(read, 'mobile')).toEqual({ cells: 1, hidden: 0, other: 2 });
    expect(countReferences(read, 'desktop')).toEqual({ cells: 0, hidden: 0, other: 0 });
    expect(countReferences(() => undefined, 'tablet')).toEqual({ cells: 0, hidden: 0, other: 0 });
  });

  it('says it in one sentence, or nothing', () => {
    const { t } = createShellI18n().global;
    expect(removalNote({ cells: 2, hidden: 0, other: 0 }, t, 'en')).toBe('Also removed from 2 layout cells.');
    expect(removalNote({ cells: 1, hidden: 1, other: 2 }, t, 'en')).toBe(
      'Also removed from 1 layout cell, 1 list of hidden views, and 2 other layout or compare settings.',
    );
    expect(removalNote({ cells: 0, hidden: 0, other: 0 }, t, 'en')).toBeNull();
  });
});
