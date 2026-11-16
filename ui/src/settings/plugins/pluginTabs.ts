// The tabs of Settings > Plugins (RB-080-80): Installed - what loads or will load; Disabled -
// what is switched off and what did not load, with its reason; Settings - the keys of the plugin
// system and of every plugin, one card each. A failed plugin has no tab of its own: its badge
// and reason mark it in Disabled, and a tab that is empty nearly always would only add a stop.
// A plugin moves with its switch: off, it is in Disabled at once, while the next start decides
// (the row says so).

import type { JsonValue, SettingValue } from '../../ipc/protocol';
import type { InstalledPlugin } from './installed';

export const PLUGIN_TABS = ['installed', 'disabled', 'settings'] as const;
export type PluginTab = (typeof PLUGIN_TABS)[number];
/** The tabs that list plugins; `settings` shows the keys. */
export type PluginListTab = Exclude<PluginTab, 'settings'>;

export const PLUGIN_TAB_KEYS: Record<PluginTab, string> = {
  installed: 'settings.plugins.tab.installed',
  disabled: 'settings.plugins.tab.disabled',
  settings: 'settings.plugins.tab.settings',
};

/** What a list tab says when it holds no plugin. */
export const PLUGIN_TAB_EMPTY_KEYS: Record<PluginListTab, string> = {
  installed: 'settings.plugins.tab.none_installed',
  disabled: 'settings.plugins.tab.none_disabled',
};

/** The ids switched off, and whether the bundled plugins are listed. */
export const DISABLED_KEY = 'plugins.disabled';
export const SHOW_BUNDLED_KEY = 'plugins.showBundled';

export function disabledIds(values: ReadonlyMap<string, SettingValue>): string[] {
  const value: JsonValue | undefined = values.get(DISABLED_KEY)?.value;
  return Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : [];
}

export function showBundled(values: ReadonlyMap<string, SettingValue>): boolean {
  return values.get(SHOW_BUNDLED_KEY)?.value !== false;
}

/** The list tab of `plugin`: Disabled for a refused package, one that failed, and one whose id
 * `plugins.disabled` holds; Installed for the rest (running, paused, waiting for trust, or
 * switched on again for the next start). */
export function pluginTab(plugin: InstalledPlugin, disabled: readonly string[]): PluginListTab {
  if (!plugin.verified || plugin.state === 'error' || disabled.includes(plugin.id)) {
    return 'disabled';
  }
  return 'installed';
}

/** The plugins of each list tab among `plugins`. */
export function countTabs(plugins: readonly InstalledPlugin[], disabled: readonly string[]): Record<PluginListTab, number> {
  const counts: Record<PluginListTab, number> = { installed: 0, disabled: 0 };
  for (const plugin of plugins) {
    counts[pluginTab(plugin, disabled)] += 1;
  }
  return counts;
}
