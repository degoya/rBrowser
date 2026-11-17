// What deleting a breakpoint in the settings takes along (RB-090-61), for the confirmation: the
// places `rb_config::breakpoint_refs` (crates/rb-config/src/breakpoint_refs.rs) removes on the
// write, counted in the values in effect - the cells of the grid and of the presets' grids, the
// hidden selections, the split and canvas fields, a preset's focus, `envCompare.breakpoints`.

import type { InjectionKey } from 'vue';
import type { JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

/** The references to one breakpoint, by kind, as the core counts them. */
export interface References {
  cells: number;
  hidden: number;
  other: number;
}

/** The note under a list entry's delete confirmation: `(key, entry)` -> text, or `null`. */
export const REMOVAL_NOTE: InjectionKey<(key: string, item: JsonValue) => string | null> = Symbol('removalNote');

/** The settings key of the breakpoints, the one list whose entries the others name. */
export const BREAKPOINTS_KEY = 'breakpoints';

const MODES = ['horizontal', 'split', 'canvas'] as const;

function ids(list: unknown, id: string): number {
  return Array.isArray(list) ? list.filter((entry) => entry === id).length : 0;
}

function items(list: unknown, id: string): number {
  return Array.isArray(list) ? list.filter((entry) => isObject(entry) && entry.breakpoint === id).length : 0;
}

function modeFields(fields: unknown, id: string): number {
  return isObject(fields) ? ids(fields.primary, id) + ids(fields.secondary, id) + items(fields.items, id) : 0;
}

function layoutOf(layout: unknown, id: string, found: References): void {
  if (!isObject(layout)) return;
  found.cells += items(layout.cells, id);
  found.hidden += ids(layout.hidden, id);
  const modes = layout.modes;
  if (isObject(modes)) {
    for (const mode of MODES) found.other += modeFields(modes[mode], id);
  }
}

/** The references to `id` in the values in effect, read by key (`layout`, `layouts`,
 * `envCompare.breakpoints`). */
export function countReferences(read: (key: string) => JsonValue | undefined, id: string): References {
  const found: References = { cells: 0, hidden: 0, other: 0 };
  layoutOf(read('layout'), id, found);
  const layouts = read('layouts');
  const presets = isObject(layouts) ? layouts.presets : undefined;
  if (isObject(presets)) {
    for (const preset of Object.values(presets)) {
      if (!isObject(preset)) continue;
      layoutOf(preset.layout, id, found);
      found.hidden += ids(preset.hidden, id);
      found.other += modeFields(preset, id) + (preset.focus === id ? 1 : 0);
    }
  }
  found.other += ids(read('envCompare.breakpoints'), id);
  return found;
}

type Translate = (key: string, args: Record<string, unknown>) => string;

/** "Also removed from 2 layout cells." - or `null` when nothing goes along. */
export function removalNote(found: References, t: Translate, locale: string): string | null {
  const parts = (['cells', 'hidden', 'other'] as const)
    .filter((kind) => found[kind] > 0)
    .map((kind) => t(`settings.list.refs.${kind}`, { n: found[kind] }));
  if (parts.length === 0) return null;
  const list = new Intl.ListFormat(locale, { type: 'conjunction' }).format(parts);
  return t('settings.list.refs.note', { list });
}
