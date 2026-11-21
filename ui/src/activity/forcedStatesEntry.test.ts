import { describe, expect, it } from 'vitest';
import type { ViewForcedStates } from '../ipc/activity';
import { sampleState } from '../ipc/mockState';
import { barEntries, barSettings, isRunning } from './activities';
import { entryView, type EntryData } from './entries';
import { forcedStatesEntry } from './forcedStatesEntry';

/** The key and its arguments, so the entry is tested apart from the catalogue's wording. */
const t = (key: string, args?: Record<string, unknown>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

const LABELS: Record<number, string> = { 1: 'Mobile', 2: 'Tablet', 3: 'Desktop' };
const label = (view: number): string => LABELS[view] ?? String(view);

function data(forced: ViewForcedStates[] | undefined): EntryData {
  const state = sampleState();
  if (forced !== undefined) state.forcedStates = forced;
  return { inputs: { state, flowRun: null, checks: null, portal: null }, nowMs: 0, viewLabel: label };
}

describe('the forced states entry (RB-094-16)', () => {
  it('runs while a view has a forced state, and not otherwise', () => {
    expect(isRunning('forcedStates', data(undefined).inputs)).toBe(false);
    expect(isRunning('forcedStates', data([]).inputs)).toBe(false);
    expect(isRunning('forcedStates', data([{ view: 1, states: ['hover'] }]).inputs)).toBe(true);
    expect(entryView('forcedStates', data(undefined), t)).toBeNull();
  });

  it('names the states and the views, counts the views, and resets every view', () => {
    const view = entryView(
      'forcedStates',
      data([
        { view: 1, states: ['hover'] },
        { view: 3, states: ['hover'] },
      ]),
      t,
    );
    expect(view?.text).toBe(
      'shell.activity.forced_states.status {"groups":"shell.activity.forced_states.group {\\"states\\":\\":hover\\",\\"views\\":\\"Mobile, Desktop\\"}"}',
    );
    expect(view?.short).toBe('2');
    expect(view?.color).toBe('warning');
    expect(view?.actions.map((action) => [action.key, action.command, action.message, action.primary === true])).toEqual([
      [
        'reset',
        'view.force_state.reset',
        { type: 'runCommand', id: 'view.force_state.reset', args: { target: 'all' } },
        true,
      ],
    ]);
    expect(view?.actions[0]?.label).toBe('shell.activity.forced_states.reset');
    expect(view?.closeLabel).toBe('shell.activity.close {"name":"shell.activity.name.forcedStates"}');
  });

  it('groups the views by their states, in the order the views come', () => {
    const view = forcedStatesEntry(
      [
        { view: 1, states: ['hover', 'focus-within'] },
        { view: 2, states: ['visited'] },
        { view: 3, states: ['hover', 'focus-within'] },
      ],
      label,
      t,
      'fold',
    );
    expect(view?.text).toBe(
      `shell.activity.forced_states.status ${JSON.stringify({
        groups: [
          'shell.activity.forced_states.group {"states":":hover, :focus-within","views":"Mobile, Desktop"}',
          'shell.activity.forced_states.group {"states":":visited","views":"Tablet"}',
        ].join('; '),
      })}`,
    );
    expect(view?.short).toBe('3');
    expect(view?.closeLabel).toBe('fold');
  });

  it('can be left to the toolbar with its switch', () => {
    const settings = barSettings({ 'ui.activityBar.show.forcedStates': false });
    expect(settings.show.forcedStates).toBe(false);
    expect(barEntries(['forcedStates', 'time'], settings, [])).toEqual(['time']);
    expect(barSettings({}).show.forcedStates).toBe(true);
  });
});
