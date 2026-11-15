// The settings search: a key matches when every word of the query is found in its label, its
// key, its description or its help text (the translated texts), ignoring case. "Only changed" keeps the keys
// whose value does not come from the default.

import type { SettingValue } from '../ipc/protocol';
import type { SettingDef } from './schema';

export type Translate = (key: string) => string;

export interface SearchFilter {
  query: string;
  onlyChanged: boolean;
}

/** Where a word was found, for the tests and the result count. */
export type MatchField = 'label' | 'key' | 'description' | 'help';

/** The fields of `def` that contain `word`. */
export function matchFields(def: SettingDef, word: string, t: Translate): MatchField[] {
  const needle = word.toLowerCase();
  const fields: [MatchField, string][] = [
    ['label', t(def.labelKey)],
    ['key', def.key],
    ['description', t(def.descriptionKey)],
    ['help', t(def.helpKey)],
  ];
  return fields.filter(([, text]) => text.toLowerCase().includes(needle)).map(([field]) => field);
}

/** Whether `def` matches every word of `query` (an empty query matches all). */
export function matches(def: SettingDef, query: string, t: Translate): boolean {
  const words = query.trim().split(/\s+/).filter((word) => word !== '');
  return words.every((word) => matchFields(def, word, t).length > 0);
}

/** Whether the value of `def` is set somewhere (not the default). */
export function isChanged(def: SettingDef, values: ReadonlyMap<string, SettingValue>): boolean {
  const value = values.get(def.key);
  return value !== undefined && value.origin !== 'default';
}

/** The predicate the layout filters with. */
export function filterOf(
  filter: SearchFilter,
  values: ReadonlyMap<string, SettingValue>,
  t: Translate,
): (def: SettingDef) => boolean {
  return (def) =>
    matches(def, filter.query, t) && (!filter.onlyChanged || isChanged(def, values));
}
