import { describe, expect, it } from 'vitest';
import { baselineArg, columnFields, columnsArg, failure, placeholders, setupKeys, shownArg, unitKey, unitsOf, type RulersState } from './panel/model';

const t = (key: string, params: Record<string, unknown> = {}): string => `${key}${Object.keys(params).length ? JSON.stringify(params) : ''}`;
const STATE: RulersState = {
  visible: { rulers: true, columns: false, baseline: true },
  guides: [],
  columns: { '*': { preset: 'bootstrap' }, tablet: { columns: 8 } },
  baseline: { lineHeight: 8, offset: 0 },
};

describe('the rulers-guides panel model', () => {
  it('measures a vertical guide from either edge or in percent, a horizontal one from the top', () => {
    expect(unitsOf('x')).toEqual(['px', 'px-right', 'percent']);
    expect(unitsOf('y')).toEqual(['px']);
    expect([unitKey('x', 'px-right'), unitKey('y', 'px')]).toEqual(['panel.unit_px-right', 'panel.unit_px_top']);
  });

  it('lists the breakpoints, those with a setup of their own, then every other one', () => {
    expect(setupKeys(['desktop', 'mobile'], STATE)).toEqual(['desktop', 'mobile', 'tablet', '*']);
  });

  it('sends only the fields that are set, as numbers', () => {
    const fields = { ...columnFields({ columns: 4, gutter: 16 }), maxWidth: '1200', align: 'center' };
    expect(columnsArg('mobile', fields)).toEqual({ columns: { mobile: { align: 'center', columns: 4, gutter: 16, maxWidth: 1200 } } });
    expect(columnsArg('*', columnFields(undefined))).toEqual({ columns: { '*': {} } });
    expect(baselineArg('24', '')).toEqual({ baseline: { lineHeight: 24, offset: 0 } });
  });

  it('switches a layer on with its setup, unless it is on', () => {
    expect(shownArg(STATE, 'columns')).toEqual({ visible: { columns: true } });
    expect(shownArg(STATE, 'baseline')).toEqual({});
  });

  it("shows the template's values where a field is empty", () => {
    const templates = {
      '': { columns: 12, gutter: 24, margin: 0, maxWidth: null },
      bootstrap: { columns: 12, gutter: 24, margin: 12, maxWidth: { min: 540, max: 1320 } },
    };
    expect(placeholders(t, templates, 'bootstrap')).toEqual({ columns: '12', gutter: '24', margin: '12', maxWidth: 'panel.max_width_range{"min":540,"max":1320}' });
    expect(placeholders(t, templates, 'unknown').maxWidth).toBe('panel.max_width_full');
    expect(placeholders(t, {}, '')).toEqual({});
  });

  it('says a failure as a sentence: the plugin text, a host code, else a general one', () => {
    const strings = { 'plugin.rulers-guides.errors.bad_args': 'Bad: {reason}' };
    const own = (key: string, params: Record<string, unknown> = {}): string => (key === 'errors.bad_args' ? `Bad: ${String(params.reason ?? '')}` : key);
    expect(failure(own, strings, 'plugin.rulers-guides.errors.bad_args')).toBe('Bad.');
    expect(failure(own, strings, 'plugin.rulers-guides.errors.bad_args', { reason: 'no guide 9' })).toBe('Bad: no guide 9.');
    expect(failure(own, strings, 'plugins.error.busy')).toBe('panel.error_busy');
    expect(failure(own, strings, 'plugin.rulers-guides.errors.unknown')).toBe('panel.error_other');
    expect(failure(own, strings, 'plugins.error.new')).toBe('panel.error_other');
  });
});
