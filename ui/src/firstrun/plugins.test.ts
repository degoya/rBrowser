import { describe, expect, it } from 'vitest';
import {
  CATEGORIES,
  type AvailablePlugin,
  changesOf,
  grantsOf,
  groupByCategory,
  initialChoice,
  parseAvailable,
  parseOutcome,
  parseProgress,
  totalBytes,
  visiblePlugins,
  withAll,
  withNone,
} from './plugins';
import { SAMPLE_PLUGINS } from './testing';

const byId = (id: string): AvailablePlugin => {
  const plugin = SAMPLE_PLUGINS.find((entry) => entry.id === id);
  if (plugin === undefined) throw new Error(id);
  return plugin;
};

describe('the plugin step', () => {
  it('reads the core answer and drops what is malformed', () => {
    const parsed = parseAvailable({
      offline: true,
      choicePending: true,
      plugins: [...SAMPLE_PLUGINS, { id: 'broken' }, { ...byId('seo'), category: 'games' }],
    });
    expect(parsed?.offline).toBe(true);
    expect(parsed?.choicePending).toBe(true);
    expect(parseAvailable({ plugins: [] })?.choicePending).toBe(false);
    expect(parsed?.plugins.map((plugin) => plugin.id)).toEqual(SAMPLE_PLUGINS.map((plugin) => plugin.id));
    expect(parseAvailable('nope')).toBeNull();
    expect(parseAvailable({ plugins: 'x' })).toBeNull();
  });

  it('ticks the recommended plugins while the first choice is pending, else what is installed', () => {
    const pending = SAMPLE_PLUGINS.map((plugin) => ({ ...plugin, installed: false }));
    expect(initialChoice({ offline: false, choicePending: true, plugins: pending })).toEqual(['seo', 'axe', 'video']);
    expect(initialChoice({ offline: false, choicePending: false, plugins: SAMPLE_PLUGINS })).toEqual(['video']);
  });

  it('installs the newly ticked and removes the unticked installed ones', () => {
    expect(changesOf(SAMPLE_PLUGINS, ['seo', 'vue-devtools'])).toEqual({ install: ['seo', 'vue-devtools'], remove: ['video'] });
    expect(changesOf(SAMPLE_PLUGINS, ['video'])).toEqual({ install: [], remove: [] });
  });

  it('groups by category in a fixed order and leaves empty groups out', () => {
    const groups = groupByCategory(SAMPLE_PLUGINS);
    expect(groups.map(([category]) => category)).toEqual(['checks', 'devtools', 'capture']);
    expect(CATEGORIES.indexOf('checks')).toBeLessThan(CATEGORIES.indexOf('tools'));
  });

  it('filters by category and by a search over name and description', () => {
    expect(visiblePlugins(SAMPLE_PLUGINS, 'devtools', '').map((plugin) => plugin.id)).toEqual(['vue-devtools']);
    expect(visiblePlugins(SAMPLE_PLUGINS, 'all', 'ACCESS').map((plugin) => plugin.id)).toEqual(['axe']);
    expect(visiblePlugins(SAMPLE_PLUGINS, 'all', '  ').length).toBe(SAMPLE_PLUGINS.length);
  });

  it('All ticks every visible plugin, None unticks the visible ones only', () => {
    const visible = visiblePlugins(SAMPLE_PLUGINS, 'checks', '');
    expect(withAll(['video'], visible)).toEqual(['video', 'seo', 'axe']);
    expect(withNone(['video', 'seo', 'axe'], visible)).toEqual(['video']);
    const capture = visiblePlugins(SAMPLE_PLUGINS, 'capture', '');
    expect(withNone(['video'], capture)).toEqual([]);
  });

  it('sums the size and lists the grants of what gets installed, once each', () => {
    expect(totalBytes(SAMPLE_PLUGINS, ['seo', 'axe'])).toBe(1_500_000 + 2_000_000);
    expect(grantsOf(SAMPLE_PLUGINS, ['seo', 'axe', 'vue-devtools'])).toEqual(['cdp', 'http', 'views']);
  });

  it('reads the install progress and the outcome', () => {
    expect(parseProgress({ pluginInstallProgress: { id: 'seo', done: 1, total: 2, state: 'installed' } })).toEqual({
      id: 'seo',
      done: 1,
      total: 2,
      state: 'installed',
    });
    expect(parseProgress({ value: 1 })).toBeNull();
    expect(
      parseOutcome({
        results: [
          { id: 'seo', ok: true, origin: 'repository' },
          { id: 'axe', ok: false, error: 'plugins.catalog.error.download', origin: 'bundle' },
        ],
        restart: true,
      }),
    ).toEqual({
      results: [
        { id: 'seo', ok: true, origin: 'repository' },
        { id: 'axe', ok: false, error: 'plugins.catalog.error.download', origin: 'bundle' },
      ],
      restart: true,
    });
    expect(parseOutcome({ results: 'x' })).toBeNull();
  });
});
