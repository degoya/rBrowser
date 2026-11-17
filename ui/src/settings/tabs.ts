// The sub-tabs of a settings category (RB-080-13, RB-080-71): a category of more than one section
// shows one section per tab instead of one long page, because a reader comes for one section at
// a time. A tab is a section: its id is the section id, its title the section title, so a key's
// tab is its `x-section` and the search and deep links need no table of their own. A category
// of one section stays one page; `settings.sectionTabs` off puts every section on one page. The
// tab a category was left on is remembered per category (`rememberTab`), in this browser only.
// Plugins is the one category whose tabs are not its sections (RB-080-80): Installed and
// Disabled list the plugins, Settings holds every section of the category.

import { CATALOG_TAB, PLUGIN_TAB_KEYS, PLUGIN_TABS } from './plugins/pluginTabs';
import type { CategoryView, SectionView } from './schema';

/** The category whose tabs are the plugin tabs (`plugins/pluginTabs.ts`). */
export const PLUGINS_CATEGORY = 'plugins';
/** The plugin tab that holds the category's sections. */
const PLUGIN_SETTINGS_TAB = 'settings';

export interface SettingsTab {
  /** The section id; the `?tab=` of the address. */
  id: string;
  titleKey: string;
}

/** The tabs of `category` in section order - Plugins its plugin tabs; none for a category of one
 * section, or with `enabled` (`settings.sectionTabs`) off. */
export function tabsOf(category: CategoryView, enabled = true): SettingsTab[] {
  if (enabled && category.id === PLUGINS_CATEGORY) {
    return PLUGIN_TABS.map((id) => ({ id, titleKey: PLUGIN_TAB_KEYS[id] }));
  }
  if (!enabled || category.sections.length < 2) {
    return [];
  }
  return category.sections.map((section) => ({ id: section.id, titleKey: section.titleKey }));
}

/** The tab shown: `wanted` when the category has it, else the first; `null` without tabs. */
export function activeTab(tabs: readonly SettingsTab[], wanted: string | null): string | null {
  return tabs.find((tab) => tab.id === wanted)?.id ?? tabs[0]?.id ?? null;
}

/** The sections shown under `tab`: all of them in a category without tabs. */
export function sectionsOfTab(category: CategoryView, tab: string | null, enabled = true): SectionView[] {
  const tabs = tabsOf(category, enabled);
  if (tabs.length === 0) {
    return category.sections;
  }
  const shown = activeTab(tabs, tab);
  if (category.id === PLUGINS_CATEGORY) {
    // The catalogue's tab shows its own section; Settings every other (RB-091-01).
    if (shown === CATALOG_TAB) {
      return category.sections.filter((section) => section.id === CATALOG_TAB);
    }
    return shown === PLUGIN_SETTINGS_TAB ? category.sections.filter((section) => section.id !== CATALOG_TAB) : [];
  }
  return category.sections.filter((section) => section.id === shown);
}

/** The tab that shows section `section` of category `category`: the section itself, in Plugins
 * the Catalog tab for the catalogue's section and the Settings tab for the others. */
export function tabOfSection(category: string, section: string): string {
  if (category === PLUGINS_CATEGORY) {
    return section === CATALOG_TAB ? CATALOG_TAB : PLUGIN_SETTINGS_TAB;
  }
  return section;
}

type TabStorage = Pick<Storage, 'getItem' | 'setItem'>;

const PREFIX = 'rbrowser.settings.tab.';

/** The window's storage; `null` where the browser refuses it (a convenience, never needed). */
function localStore(): TabStorage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** Remembers `tab` as the tab `category` was left on. */
export function rememberTab(category: string, tab: string, storage: TabStorage | null = localStore()): void {
  try {
    storage?.setItem(`${PREFIX}${category}`, tab);
  } catch {
    // Storage refused: the category opens at its first tab next time.
  }
}

/** The tab `category` was left on, if one is remembered. */
export function rememberedTab(category: string, storage: TabStorage | null = localStore()): string | null {
  try {
    return storage?.getItem(`${PREFIX}${category}`) ?? null;
  } catch {
    return null;
  }
}
