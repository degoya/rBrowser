// What a `settingsChanged` event carries (RB-090-36): the new value and origin of each changed
// key, so a page takes them instead of asking `getSettings` again. An event without them, or
// with a key left out (one the core's registry does not know), has the page ask as before.

import type { SettingValue } from '../ipc/protocol';

/** The fields of the `settingsChanged` event. */
export interface SettingsChange {
  keys: string[];
  values?: SettingValue[];
}

/** The new values by key when the event carries one for every key it names; `null` otherwise. */
export function changedValues(change: SettingsChange): Map<string, SettingValue> | null {
  if (change.values === undefined) {
    return null;
  }
  const byKey = new Map(change.values.map((value) => [value.key, value]));
  return change.keys.every((key) => byKey.has(key)) ? byKey : null;
}

/** Whether the event names one of `keys`. */
export function touches(change: Pick<SettingsChange, 'keys'>, keys: readonly string[]): boolean {
  return change.keys.some((key) => keys.includes(key));
}
