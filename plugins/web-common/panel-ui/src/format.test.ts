import { afterEach, describe, expect, it } from 'vitest';
import { errorText, format, splitError } from './format';

afterEach(() => {
  delete (globalThis as { PanelTexts?: unknown }).PanelTexts;
});

describe('panel texts', () => {
  it('fills placeholders and English plurals', () => {
    expect(format('Run {run}: {url}', { run: 'r1', url: 'x' })).toBe('Run r1: x');
    const findings = '{count, plural, =0 {no findings} one {# finding} other {# findings}}';
    expect(format(findings, { count: 0 })).toBe('no findings');
    expect(format(findings, { count: 1 })).toBe('1 finding');
    expect(format(findings, { count: 7 })).toBe('7 findings');
    expect(format('{n, plural, one {# view in {where}} other {# views in {where}}}', { n: 2, where: 'A' })).toBe('2 views in A');
  });

  it('splits the older `code key=value` form', () => {
    expect(splitError('plugin.x.errors.no_sidecar reason=not started')).toEqual({ code: 'plugin.x.errors.no_sidecar', params: { reason: 'not started' } });
  });

  it('says a failure in one sentence, never the code', () => {
    const strings = { 'plugins.error.busy': 'The plugin is busy.', 'plugins.reason.timeout': 'It took too long.' };
    expect(errorText(strings, 'plugins.error.busy', { reason: 'timeout' })).toBe('The plugin is busy. It took too long.');
    expect(errorText(strings, 'plugins.error.busy', { reason: 'no_answer' })).toBe('The plugin is busy.');
    expect(errorText(strings, 'plugins.error.unknown_one', {})).toBe('The plugin could not do that.');
    expect(errorText({ 'p.e': 'Failed ({reason}).' }, 'p.e reason=disk full')).toBe('Failed (disk full).');
    expect(errorText({ 'p.e': 'Failed.' }, 'p.e', { reason: 'disk is full' })).toBe('Failed. disk is full');
  });

  it('uses texts.js when it offers the helpers', () => {
    (globalThis as { PanelTexts?: unknown }).PanelTexts = {
      format: () => 'shared format',
      errorText: (_: unknown, code: string) => `shared ${code}`,
    };
    expect(format('x')).toBe('shared format');
    expect(errorText({}, 'c', {})).toBe('shared c');
  });
});
