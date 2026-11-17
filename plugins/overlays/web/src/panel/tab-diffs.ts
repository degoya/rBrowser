// The panel's view of the tab order compared across views (RB-070-12): the `differences` the
// component answers to `overlay_tab_order` and posts after switching the overlay on
// (src/tab_compare.rs), checked and turned into lines.

import type { Text } from './model';

export interface OnlyIn {
  selector: string;
  views: number[];
  missing: number[];
}

export interface Moved {
  selector: string;
  positions: { view: number; index: number }[];
}

export interface Differences {
  views: number[];
  onlyIn: OnlyIn[];
  onlyInCount: number;
  moved: Moved[];
  movedCount: number;
}

const numbers = (value: unknown): number[] | undefined =>
  Array.isArray(value) && value.every((entry) => typeof entry === 'number') ? (value as number[]) : undefined;

const record = (value: unknown): Record<string, unknown> => (value && typeof value === 'object' ? (value as Record<string, unknown>) : {});

/** `value` as differences; none when it is not the component's shape. */
export function asDifferences(value: unknown): Differences | undefined {
  const raw = record(value);
  const views = numbers(raw.views);
  if (!views) return undefined;
  const onlyIn = (Array.isArray(raw.onlyIn) ? raw.onlyIn : []).map(record).flatMap((entry) => {
    const present = numbers(entry.views);
    const missing = numbers(entry.missing);
    return typeof entry.selector === 'string' && present && missing ? [{ selector: entry.selector, views: present, missing }] : [];
  });
  const moved = (Array.isArray(raw.moved) ? raw.moved : []).map(record).flatMap((entry) => {
    const positions = Array.isArray(entry.positions)
      ? entry.positions.map(record).filter((position) => typeof position.view === 'number' && typeof position.index === 'number')
      : undefined;
    return typeof entry.selector === 'string' && positions ? [{ selector: entry.selector, positions: positions as Moved['positions'] }] : [];
  });
  const count = (field: unknown, fallback: number): number => (typeof field === 'number' ? field : fallback);
  return { views, onlyIn, onlyInCount: count(raw.onlyInCount, onlyIn.length), moved, movedCount: count(raw.movedCount, moved.length) };
}

/** The panel's lines for `differences`. */
export function diffLines(t: Text, differences: Differences): string[] {
  if (differences.views.length < 2) return [t('panel.tab_single')];
  const lines: string[] = [];
  for (const entry of differences.onlyIn) lines.push(t('panel.tab_only', { selector: entry.selector, views: entry.views.join(', ') }));
  if (differences.onlyInCount > differences.onlyIn.length) lines.push(t('panel.tab_more', { count: differences.onlyInCount - differences.onlyIn.length }));
  for (const entry of differences.moved) {
    const positions = entry.positions.map((position) => t('panel.tab_position', position)).join(', ');
    lines.push(t('panel.tab_moved', { selector: entry.selector, positions }));
  }
  if (differences.movedCount > differences.moved.length) lines.push(t('panel.tab_more', { count: differences.movedCount - differences.moved.length }));
  return lines.length ? lines : [t('panel.tab_same')];
}
