// The core's bounds on a native menu (`layout::native_menu`, RB-090-45, RB-090-63) and the
// fitting of a whole tree to them: the core refuses a menu with more than `MAX_ROWS` rows in one
// menu, `MAX_TOTAL` in all or a row deeper than `MAX_DEPTH` levels, so the shell cuts first -
// what is cut becomes one "more" row at the end of its menu (it opens the command palette, where
// every command is), never a silent drop.
import type { NativeMenuRow } from '../ipc/protocol';

/** The most rows of one menu or submenu, and characters of one label, the core takes. */
export const MAX_ROWS = 200;
export const MAX_TEXT = 256;
/** The most rows of a menu with all its submenus, and the deepest a row hangs (RB-090-63). */
export const MAX_TOTAL = 800;
export const MAX_DEPTH = 4;

/** The id prefix of a more row; each has a number of its own, ids are unique in a tree. */
const MORE_PREFIX = '~more:';

/** Whether `id` is a more row's. */
export function isMoreRow(id: string): boolean {
  return id.startsWith(MORE_PREFIX);
}

/** The rows of `rows` with every submenu's, as the core counts them against `MAX_TOTAL`. */
export function countRows(rows: readonly NativeMenuRow[]): number {
  return rows.reduce((sum, row) => sum + 1 + (row.kind === 'submenu' ? countRows(row.rows) : 0), 0);
}

/** The rows that can be chosen among `rows`, submenus included. */
function choosable(rows: readonly NativeMenuRow[]): number {
  return rows.reduce((sum, row) => sum + (row.kind === 'item' ? 1 : row.kind === 'submenu' ? choosable(row.rows) : 0), 0);
}

/** `rows` without the separators and headings at its end. */
function tidy(rows: NativeMenuRow[]): NativeMenuRow[] {
  const kept = [...rows];
  while (kept.length > 0 && kept.at(-1)?.kind !== 'item' && kept.at(-1)?.kind !== 'submenu') {
    kept.pop();
  }
  return kept;
}

/** The more row for `count` rows; `id` is its own. */
export type MoreRow = (count: number, id: string) => NativeMenuRow;

/**
 * `rows` within the core's bounds: each menu at most `MAX_ROWS` rows, the tree at most
 * `MAX_TOTAL`, no row deeper than `MAX_DEPTH`. A menu that does not fit ends with `more(count)`
 * for the `count` choosable rows cut; a submenu too deep is a more row in its place.
 */
export function fitTree(rows: readonly NativeMenuRow[], more: MoreRow): NativeMenuRow[] {
  let made = 0;
  const next = (count: number): NativeMenuRow => {
    made += 1;
    return more(count, `${MORE_PREFIX}${made}`);
  };
  const budget = { left: MAX_TOTAL };

  function fit(list: readonly NativeMenuRow[], depth: number): NativeMenuRow[] {
    const out: NativeMenuRow[] = [];
    for (const [index, row] of list.entries()) {
      // Room for a more row stays while rows follow this one.
      const reserve = index === list.length - 1 ? 0 : 1;
      const room = Math.min(MAX_ROWS - out.length, budget.left) - reserve;
      if (row.kind !== 'submenu') {
        if (room < 1) {
          budget.left -= 1;
          return [...tidy(out), next(choosable(list.slice(index)))];
        }
        budget.left -= 1;
        out.push(row);
        continue;
      }
      if (depth >= MAX_DEPTH) {
        // Its rows would hang deeper than the core takes: one row in its place.
        if (room < 1) {
          budget.left -= 1;
          return [...tidy(out), next(choosable(list.slice(index)))];
        }
        budget.left -= 1;
        out.push(next(choosable(row.rows)));
        continue;
      }
      if (room < 2) {
        budget.left -= 1;
        return [...tidy(out), next(choosable(list.slice(index)))];
      }
      // The submenu's own row, and the more row this menu may still need, held back.
      budget.left -= 1 + reserve;
      const inner = fit(row.rows, depth + 1);
      budget.left += reserve;
      out.push({ ...row, rows: inner });
    }
    return out;
  }

  return fit(rows, 1);
}
