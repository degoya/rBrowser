import { describe, expect, it } from 'vitest';
import { format } from '../../../../web-common/panel-ui/src/format';
import { GROUPS, isOn, selectorOf, setArgs, type Text, toggleKey, TOGGLES, Views } from './model';

const strings: Record<string, string> = {
  'plugin.overlays.panel.view': 'View {view}',
  'plugin.overlays.panel.matches': '{current} of {count} matches',
  'plugin.overlays.panel.invalid': 'Not a valid selector',
  'plugin.overlays.panel.scroll': 'scrolls {px} px: {causes}',
  'plugin.overlays.panel.no_scroll': 'no horizontal scroll',
  'plugin.overlays.panel.hits': '{overlay}: {count}',
  'plugin.overlays.panel.hits_shown': '{overlay}: {shown} of {count} shown',
  'plugin.overlays.panel.toggle.stacking': 'Stacking contexts',
  'plugin.overlays.panel.toggle.target_size': 'Target size',
};
/** The panel's `t` over `strings`: the text below the prefix, formatted; the key when it is missing. */
const t: Text = (key, params = {}) => format(strings[`plugin.overlays.${key}`] ?? key, params);
const all = () => true;

describe('the panel model', () => {
  it('lists per view the matches and the overflow of the overlays that are on', () => {
    const views = new Views();
    expect(views.take(2, { type: 'matches', count: 3, index: 1 })).toBe(true);
    expect(views.take(1, { type: 'matches', count: 0, index: 0, error: 'invalid' })).toBe(true);
    const report = {
      hasHorizontalScroll: true,
      scrollWidth: 416,
      clientWidth: 375,
      innerWidth: 375,
      causes: [{ selector: 'div.promo', rect: { x: 0, y: 0, width: 400, height: 80 }, overhang: 41 }],
      vw100: [],
    };
    expect(views.take(2, { type: 'overflow', report })).toBe(true);
    expect(views.take(2, { type: 'other' })).toBe(false);
    expect(views.take('2', { type: 'matches', count: 1 })).toBe(false);
    expect(views.lines(t, all)).toEqual([
      'View 1: Not a valid selector',
      'View 2: 2 of 3 matches; scrolls 41 px: div.promo (+41px)',
    ]);
    expect(views.lines(t, () => false)).toEqual([]);
  });

  it('lists the hits of the checking overlays that are on', () => {
    const views = new Views();
    expect(views.take(1, { type: 'hits', overlay: 'target-size', count: 4 })).toBe(true);
    expect(views.take(1, { type: 'hits', overlay: 'small-text', count: 2 })).toBe(true);
    expect(views.take(1, { type: 'hits', overlay: 7, count: 2 })).toBe(false);
    expect(views.lines(t, (overlay) => overlay === 'target-size')).toEqual(['View 1: Target size: 4']);
  });

  // RB-090-17: past what the page draws, the line names how many of the whole count it shows.
  it('names how many hits of a big page are drawn', () => {
    const views = new Views();
    expect(views.take(1, { type: 'hits', overlay: 'stacking', count: 2345, shown: 1500 })).toBe(true);
    expect(views.take(2, { type: 'hits', overlay: 'stacking', count: 12, shown: 'x' })).toBe(true);
    expect(views.lines(t, all)).toEqual(['View 1: Stacking contexts: 1500 of 2345 shown', 'View 2: Stacking contexts: 12']);
  });

  it('switches every overlay of the plugin, keyed with underscores', () => {
    expect(TOGGLES).toContain('target-size');
    expect(TOGGLES).toHaveLength(16);
    expect(toggleKey('text-spacing')).toBe('text_spacing');
  });

  // RB-080-86: the switches are grouped by kind; every overlay is in exactly one group.
  it('puts every overlay into exactly one group', () => {
    const grouped = GROUPS.flatMap((group) => group.toggles);
    expect([...grouped].sort()).toEqual([...TOGGLES].sort());
    expect(GROUPS.map((group) => group.id)).toEqual(['layout', 'selector', 'accessibility', 'content']);
    expect(GROUPS.find((group) => group.id === 'selector')?.toggles).toEqual(['highlight', 'force-state']);
  });

  it('reads the switches and the selector from the state', () => {
    const state = { all: { overflow: { enabled: true }, highlight: { enabled: false, options: { selector: '.card' } } }, views: {} };
    expect(isOn(state, 'overflow')).toBe(true);
    expect(isOn(state, 'highlight')).toBe(false);
    expect(isOn(undefined, 'outlines')).toBe(false);
    expect(selectorOf(state)).toBe('.card');
  });

  it('sends the selector and the forced state only with the switches that use them', () => {
    expect(setArgs('outlines', true, ' .card ', 'hover', false)).toEqual({ overlay: 'outlines', enabled: true, options: {} });
    expect(setArgs('highlight', true, ' .card ', 'hover', false)).toEqual({ overlay: 'highlight', enabled: true, options: { selector: '.card' } });
    expect(setArgs('highlight', false, '.card', 'hover', false)).toEqual({ overlay: 'highlight', enabled: false, options: {} });
    expect(setArgs('force-state', true, '', 'focus', true)).toEqual({ overlay: 'force-state', enabled: true, options: { states: ['focus'] }, target: 'active' });
  });
});
