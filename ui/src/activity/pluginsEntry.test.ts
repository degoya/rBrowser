import { describe, expect, it } from 'vitest';
import type { PluginProgress } from '../ipc/activity';
import { sampleState } from '../ipc/mockState';
import { isRunning } from './activities';
import { entryView, type EntryData } from './entries';
import { pluginsEntry } from './pluginsEntry';

/** The key and its arguments, so the entry is tested apart from the catalogue's wording. */
const t = (key: string, args?: Record<string, unknown>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

const SWEEP: PluginProgress = {
  plugin: 'width-sweep',
  name: 'Width sweep',
  done: 12,
  total: 160,
  cancel: 'plugin.width_sweep.cancel',
};
const SEO: PluginProgress = { plugin: 'seo', name: 'SEO', done: 1, total: 4 };

function data(progress: PluginProgress[] | undefined): EntryData {
  const state = sampleState();
  if (progress !== undefined) state.pluginProgress = progress;
  return { inputs: { state, flowRun: null, checks: null, portal: null }, nowMs: 0, viewLabel: String };
}

describe('the plugins entry (RB-093-01)', () => {
  it('runs while a plugin call reports progress, and not otherwise', () => {
    expect(isRunning('plugins', data(undefined).inputs)).toBe(false);
    expect(isRunning('plugins', data([]).inputs)).toBe(false);
    expect(isRunning('plugins', data([SWEEP]).inputs)).toBe(true);
    expect(entryView('plugins', data(undefined), t)).toBeNull();
  });

  it("names the plugin and how far it is, with the plugin's own Cancel", () => {
    const view = entryView('plugins', data([SWEEP]), t);
    expect(view?.text).toBe('shell.activity.plugins.status {"name":"Width sweep","done":12,"total":160}');
    expect(view?.short).toBe('12/160');
    expect(view?.color).toBe('primary');
    expect(view?.actions.map((action) => [action.key, action.message, action.primary === true])).toEqual([
      ['cancel-width-sweep', { type: 'runCommand', id: 'plugin.width_sweep.cancel' }, true],
    ]);
    expect(view?.actions[0]?.label).toBe('shell.activity.plugins.cancel {"name":"Width sweep"}');
    expect(view?.actions[0]?.command).toBe('plugin.width_sweep.cancel');
  });

  it('offers no Cancel for a plugin without a cancel command', () => {
    expect(pluginsEntry([SEO], t, 'fold')?.actions).toEqual([]);
  });

  it('says that a plugin does not know its total yet', () => {
    const view = pluginsEntry([{ ...SEO, done: 0, total: 0 }], t, 'fold');
    expect(view?.text).toBe('shell.activity.plugins.working {"name":"SEO"}');
    expect(view?.short).toBe('0');
  });

  it('shows the first of several plugins and counts the others, each with its Cancel', () => {
    const second = { ...SWEEP, plugin: 'other', name: 'Other', cancel: 'plugin.other.cancel' };
    const view = pluginsEntry([SEO, SWEEP, second], t, 'fold');
    expect(view?.text).toBe(
      'shell.activity.plugins.more {"status":"shell.activity.plugins.status {\\"name\\":\\"SEO\\",\\"done\\":1,\\"total\\":4}","count":2}',
    );
    expect(view?.actions.map((action) => [action.key, action.primary === true])).toEqual([
      ['cancel-width-sweep', true],
      ['cancel-other', false],
    ]);
    expect(view?.closeLabel).toBe('fold');
  });
});
