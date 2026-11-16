// The switches of the settings window itself (`settings.*`, General > Settings window): read
// from the values the window already holds, with the schema's default while none is set.

import type { SettingValue } from '../ipc/protocol';

/** Print every key under its title (RB-080-70); default off. */
export const SHOW_KEYS_KEY = 'settings.showKeys';
/** One tab per section of a category (RB-080-71); default on. */
export const SECTION_TABS_KEY = 'settings.sectionTabs';

/** The boolean at `key` among `values`; `fallback` when it is absent or no boolean. */
export function flagOf(values: ReadonlyMap<string, SettingValue>, key: string, fallback: boolean): boolean {
  const value = values.get(key)?.value;
  return typeof value === 'boolean' ? value : fallback;
}
