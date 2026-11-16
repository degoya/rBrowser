import { describe, expect, it } from 'vitest';
import { layoutOf, type SettingsSchema } from './schema';
import { activeTab, rememberedTab, rememberTab, sectionsOfTab, tabOfSection, tabsOf } from './tabs';
import { realDefs } from './test/fakeCore';

const schema: SettingsSchema = {
  version: 1,
  categories: [
    { id: 'layout', sections: ['presets', 'grid', 'zoom', 'focus-mode', 'modes'] },
    { id: 'breakpoints', sections: ['breakpoints', 'device-presets', 'analyzer'] },
    { id: 'sync', sections: ['navigation', 'scroll', 'interaction', 'leader-submit', 'page-api', 'message-bus'] },
  ],
  defs: realDefs(),
  project: null,
};

function category(id: string) {
  const found = layoutOf(schema).find((c) => c.id === id);
  if (found === undefined) {
    throw new Error(`no ${id}`);
  }
  return found;
}

describe('the sub-tabs of a category (RB-080-13)', () => {
  it('gives Layout and Breakpoints & Devices one tab per section, in section order', () => {
    expect(tabsOf(category('layout')).map((tab) => tab.id)).toEqual([
      'presets',
      'grid',
      'zoom',
      'focus-mode',
      'modes',
    ]);
    expect(tabsOf(category('breakpoints')).map((tab) => tab.id)).toEqual([
      'breakpoints',
      'device-presets',
      'analyzer',
    ]);
    expect(tabsOf(category('layout'))[1]?.titleKey).toBe('settings.section.layout.grid');
  });

  // RB-080-71: every category of more than one section, not only Layout and Breakpoints.
  it('gives every category of more than one section its tabs; one section stays one page', () => {
    expect(tabsOf(category('sync')).map((tab) => tab.id)).toEqual(
      category('sync').sections.map((section) => section.id),
    );
    expect(tabsOf(category('sync')).length).toBeGreaterThan(1);
    const single = { ...category('sync'), sections: category('sync').sections.slice(0, 1) };
    expect(tabsOf(single)).toEqual([]);
  });

  it('shows every section on one page when settings.sectionTabs is off', () => {
    expect(tabsOf(category('layout'), false)).toEqual([]);
    expect(sectionsOfTab(category('layout'), 'grid', false).length).toBe(category('layout').sections.length);
  });

  it('shows the wanted tab, else the first', () => {
    const tabs = tabsOf(category('layout'));
    expect(activeTab(tabs, 'zoom')).toBe('zoom');
    expect(activeTab(tabs, 'nope')).toBe('presets');
    expect(activeTab(tabs, null)).toBe('presets');
    expect(activeTab([], 'zoom')).toBeNull();
  });

  it('shows only the sections of the active tab; a category without tabs shows all', () => {
    expect(sectionsOfTab(category('layout'), 'grid').map((s) => s.id)).toEqual(['grid']);
    const single = { ...category('sync'), sections: category('sync').sections.slice(0, 1) };
    expect(sectionsOfTab(single, null).length).toBe(1);
  });
});

describe('the tab a category was left on (RB-080-71)', () => {
  function memory(): Pick<Storage, 'getItem' | 'setItem'> {
    const items = new Map<string, string>();
    return { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => void items.set(key, value) };
  }

  it('remembers the tab per category', () => {
    const storage = memory();
    rememberTab('engines', 'firefox', storage);
    rememberTab('layout', 'zoom', storage);
    expect(rememberedTab('engines', storage)).toBe('firefox');
    expect(rememberedTab('layout', storage)).toBe('zoom');
    expect(rememberedTab('sync', storage)).toBeNull();
  });

  it('forgets nothing and fails nothing when the storage throws', () => {
    const broken = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    };
    expect(() => rememberTab('engines', 'firefox', broken)).not.toThrow();
    expect(rememberedTab('engines', broken)).toBeNull();
  });
});

describe('the plugin tabs of Settings > Plugins (RB-080-80)', () => {
  const plugins = { id: 'plugins', titleKey: 'settings.category.plugins', sections: category('layout').sections };

  it('gives Plugins Installed, Disabled and Settings, its sections on Settings only', () => {
    expect(tabsOf(plugins).map((tab) => tab.id)).toEqual(['installed', 'disabled', 'settings']);
    expect(sectionsOfTab(plugins, 'installed')).toEqual([]);
    expect(sectionsOfTab(plugins, 'disabled')).toEqual([]);
    expect(sectionsOfTab(plugins, 'settings')).toEqual(plugins.sections);
    expect(tabOfSection('plugins', 'installed')).toBe('settings');
    expect(tabOfSection('layout', 'grid')).toBe('grid');
  });

  it('shows every section on one page with settings.sectionTabs off', () => {
    expect(tabsOf(plugins, false)).toEqual([]);
    expect(sectionsOfTab(plugins, 'installed', false)).toEqual(plugins.sections);
  });
});
