import { describe, expect, it } from 'vitest';
import { EMPTY_SUMMARY, SETTINGS_PLACES, summaryLines } from './summary';

describe('the setup wizard summary', () => {
  it('says nothing changed when every step was skipped', () => {
    expect(summaryLines(EMPTY_SUMMARY)).toEqual([{ key: 'firstrun.done.nothing' }]);
  });

  it('names what each step did', () => {
    expect(
      summaryLines({
        plugins: {
          results: [
            { id: 'seo', ok: true, origin: 'bundle' },
            { id: 'axe', ok: false, origin: 'repository' },
          ],
          restart: true,
          removed: ['video'],
        },
        project: { op: 'written', requestId: 2, root: 'C:/site', path: 'C:/site/.rbrowser/config.json', open: false },
        basics: { theme: 'dark', layout: 'canvas', sync: false },
      }).map((line) => line.key),
    ).toEqual([
      'firstrun.done.plugins',
      'firstrun.done.plugins_failed',
      'firstrun.done.plugins_removed',
      'firstrun.done.project',
      'firstrun.done.theme.dark',
      'firstrun.done.layout.canvas',
      'firstrun.done.sync_off',
    ]);
  });

  it('leaves out the project choices that had no project', () => {
    const lines = summaryLines({ ...EMPTY_SUMMARY, basics: { theme: 'system', layout: null, sync: null } });
    expect(lines.map((line) => line.key)).toEqual(['firstrun.done.theme.system']);
  });

  it('points at the wizard switch among the places in the settings', () => {
    expect(SETTINGS_PLACES.map((place) => place.key)).toContain('ui.firstRunWizard');
  });
});
