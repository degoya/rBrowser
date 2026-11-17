// RB-090-63 review: a menu tree the core would refuse (more than 200 rows in one menu, 800 in
// all, or deeper than 4 levels) is fitted here as a whole, a "more" row in place of what is cut,
// never dropped silently.
import { describe, expect, it } from 'vitest';
import type { NativeMenuRow } from '../ipc/protocol';
import { MAX_DEPTH, MAX_ROWS, MAX_TOTAL, countRows, fitTree, isMoreRow } from './menuBounds';

function items(count: number, prefix: string): NativeMenuRow[] {
  return Array.from({ length: count }, (_, index) => ({ kind: 'item', id: `${prefix}${index}`, label: `${index}` }));
}

const more = (count: number, id: string): NativeMenuRow => ({ kind: 'item', id, label: `${count} more` });

/** The deepest level a row of `rows` hangs at; the menu itself is level 1. */
function depth(rows: readonly NativeMenuRow[]): number {
  return Math.max(1, ...rows.map((row) => (row.kind === 'submenu' ? 1 + depth(row.rows) : 1)));
}

/** Every menu of the tree within `MAX_ROWS` and not empty. */
function everyMenuFits(rows: readonly NativeMenuRow[]): boolean {
  return (
    rows.length > 0 &&
    rows.length <= MAX_ROWS &&
    rows.every((row) => row.kind !== 'submenu' || everyMenuFits(row.rows))
  );
}

/** The ids of the rows that can be chosen, submenus included. */
function ids(rows: readonly NativeMenuRow[]): string[] {
  return rows.flatMap((row) => (row.kind === 'item' ? [row.id] : row.kind === 'submenu' ? ids(row.rows) : []));
}

/** The more rows of the tree. */
function moreRows(rows: readonly NativeMenuRow[]): { id: string; label: string }[] {
  return rows.flatMap((row) =>
    row.kind === 'item' && isMoreRow(row.id) ? [row] : row.kind === 'submenu' ? moreRows(row.rows) : [],
  );
}

describe('fitting a menu tree to the core', () => {
  it('leaves a tree within the bounds as it is', () => {
    const rows: NativeMenuRow[] = [...items(3, 'a'), { kind: 'submenu', label: 'S', rows: items(5, 's') }];
    expect(fitTree(rows, more)).toEqual(rows);
  });

  it('cuts a submenu of more than 200 rows with a more row counting the rest', () => {
    const rows: NativeMenuRow[] = [{ kind: 'submenu', label: 'S', rows: items(250, 's') }];
    const fitted = fitTree(rows, more);
    const sub = fitted[0];
    if (sub?.kind !== 'submenu') throw new Error('a submenu');
    expect(sub.rows).toHaveLength(MAX_ROWS);
    const last = sub.rows.at(-1);
    expect(last?.kind === 'item' && isMoreRow(last.id)).toBe(true);
    expect(last?.kind === 'item' ? last.label : '').toBe('51 more');
  });

  it('keeps the whole tree within 800 rows, each cut menu ending in a more row', () => {
    const rows: NativeMenuRow[] = Array.from({ length: 6 }, (_, index) => ({
      kind: 'submenu' as const,
      label: `S${index}`,
      rows: items(190, `s${index}-`),
    }));
    const fitted = fitTree(rows, more);
    expect(countRows(fitted)).toBeLessThanOrEqual(MAX_TOTAL);
    expect(everyMenuFits(fitted)).toBe(true);
    const shown = ids(fitted).filter((id) => !isMoreRow(id)).length;
    const counted = moreRows(fitted).reduce((sum, row) => sum + Number.parseInt(row.label, 10), 0);
    expect(shown + counted).toBe(6 * 190);
  });

  it('replaces a submenu deeper than 4 levels with a more row, the rows beside it kept', () => {
    let deep: NativeMenuRow[] = items(2, 'leaf');
    for (let level = 0; level < MAX_DEPTH + 1; level += 1) {
      deep = [{ kind: 'submenu', label: `L${level}`, rows: deep }];
    }
    const rows: NativeMenuRow[] = [...deep, { kind: 'item', id: 'beside', label: 'Beside' }];
    const fitted = fitTree(rows, more);
    expect(depth(fitted)).toBeLessThanOrEqual(MAX_DEPTH);
    expect(ids(fitted)).toContain('beside');
    expect(ids(fitted).some(isMoreRow)).toBe(true);
  });

  it('gives every more row an id of its own and leaves no separator before one', () => {
    const rows: NativeMenuRow[] = [
      { kind: 'submenu', label: 'A', rows: [...items(198, 'a'), { kind: 'separator' }, ...items(5, 'b')] },
      { kind: 'submenu', label: 'B', rows: items(300, 'c') },
    ];
    const fitted = fitTree(rows, more);
    const mores = ids(fitted).filter(isMoreRow);
    expect(mores.length).toBe(2);
    expect(new Set(mores).size).toBe(2);
    const first = fitted[0];
    if (first?.kind !== 'submenu') throw new Error('a submenu');
    expect(first.rows.at(-2)?.kind).toBe('item');
  });
});
