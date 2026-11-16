import { describe, expect, it } from 'vitest';
import { format } from '../../../../web-common/panel-ui/src/format';
import { groups, readState, request, strengthFor, type Text, viewRow } from './model';

const strings: Record<string, string> = {
  'plugin.simulators.panel.view_row': '{label}: {type} ({mode})',
  'plugin.simulators.panel.mode_native': 'drawn by Chromium',
  'plugin.simulators.types.deuteranopia.name': 'Deuteranopia',
};
/** The panel's `t` over `strings`: the text below the prefix, formatted; the key when it is missing. */
const t: Text = (key, params = {}) => format(strings[`plugin.simulators.${key}`] ?? key, params);

const CATALOG = [
  { id: 'protanopia', group: 'color', severity: null, native: true },
  { id: 'protanomaly', group: 'color', severity: 0.6, native: false },
  { id: 'myopia', group: 'vision', severity: 1, native: true },
  { id: 'dimmed', group: 'environment', severity: 0.6, native: false },
];

describe('the picker model', () => {
  it('reads the state the component answers and drops what does not fit', () => {
    const state = readState({
      active: 2,
      views: [
        { view: 2, label: 'Mobile', engine: 'chromium', type: 'deuteranopia', severity: null, mode: 'native' },
        { view: 'x', label: 3 },
      ],
      catalog: CATALOG,
    });
    expect(state.active).toBe(2);
    expect(state.views).toHaveLength(1);
    expect(state.catalog).toHaveLength(4);
    expect(readState(null)).toEqual({ active: null, views: [], catalog: [] });
  });

  it('groups the catalogue for the picker, with Off first', () => {
    const grouped = groups(readState({ catalog: CATALOG }).catalog);
    expect(grouped.map((group) => group.id)).toEqual(['none', 'color', 'vision', 'environment']);
    expect(grouped[0]?.entries.map((entry) => entry.id)).toEqual(['none']);
    expect(grouped[1]?.entries.map((entry) => entry.id)).toEqual(['protanopia', 'protanomaly']);
  });

  it('writes a view row in words, not colour', () => {
    const [view] = readState({
      views: [{ view: 2, label: 'Mobile', engine: 'chromium', type: 'deuteranopia', severity: null, mode: 'native' }],
    }).views;
    expect(viewRow(t, view!)).toBe('Mobile: Deuteranopia (drawn by Chromium)');
  });

  it('asks for the active view, or every view, and sends a strength only when it was moved', () => {
    expect(request('deuteranopia', null, false)).toEqual({ type: 'deuteranopia' });
    expect(request('dimmed', 0.4, true)).toEqual({ type: 'dimmed', severity: 0.4, target: 'all' });
  });

  it('shows the active view\'s strength for its own simulation, else the default, none without one', () => {
    const state = readState({
      active: 2,
      views: [{ view: 2, label: 'Mobile', engine: 'chromium', type: 'protanomaly', severity: 0.4, mode: 'script' }],
      catalog: CATALOG,
    });
    expect(strengthFor(state, 'protanomaly')).toBe(0.4);
    expect(strengthFor(state, 'dimmed')).toBe(0.6);
    expect(strengthFor(state, 'protanopia')).toBeNull();
    expect(strengthFor(state, 'none')).toBeNull();
  });
});
