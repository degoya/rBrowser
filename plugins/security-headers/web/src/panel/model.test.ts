// The security header panel's model (RB-093-03): the comparison as table lines, the findings,
// the failures and the status line.
import { describe, expect, it } from 'vitest';
import { COMPARISON, STRINGS } from './fixtures';
import { failureText, findingText, lines, readEnvironments, sortedFindings, statusOf, text } from './model';

describe('the table', () => {
  it('has a line per row with a cell per environment', () => {
    const all = lines(STRINGS, COMPARISON, false);
    expect(all.map((line) => line.key)).toEqual(['content-security-policy', 'strict-transport-security', 'x-content-type-options', 'cookie:sid', 'cookie:pref']);
    expect(all[0]?.cells.prod).toEqual({ text: "script-src 'self'", sent: true });
    expect(all[2]?.cells.staging).toEqual({ text: 'Not sent', sent: false });
    expect(all[3]?.label).toBe('Cookie sid');
    expect(all[4]?.cells.prod).toEqual({ text: 'No attributes', sent: false });
  });

  it('leaves the cells of a failed environment empty', () => {
    const all = lines(STRINGS, COMPARISON, false);
    expect(all[0]?.cells.local).toEqual({ text: '', sent: false });
  });

  it('shows only the differences when asked', () => {
    expect(lines(STRINGS, COMPARISON, true).map((line) => line.key)).toEqual(['content-security-policy', 'x-content-type-options', 'cookie:sid']);
  });
});

describe('the environments', () => {
  it('lists those read and says why another failed', () => {
    expect(readEnvironments(COMPARISON).map((environment) => environment.id)).toEqual(['staging', 'prod']);
    const local = COMPARISON.environments[2];
    expect(local && failureText(STRINGS, local.error ?? {})).toBe('The request failed: connection refused');
    expect(failureText(STRINGS, {})).toBe('Not read');
  });

  it('sorts the findings errors first and words them', () => {
    const staging = COMPARISON.environments[0];
    const sorted = staging ? sortedFindings(staging) : [];
    expect(sorted.map((finding) => finding.rule)).toEqual(['csp.unsafe', 'nosniff.missing']);
    expect(sorted[0] && findingText(STRINGS, sorted[0])).toBe("The Content-Security-Policy lets 'unsafe-inline' work for scripts.");
  });
});

describe('the status line', () => {
  it('names a failure, the loading and the differences', () => {
    expect(statusOf(STRINGS, { failure: 'Broken', loading: false, comparison: null })).toEqual({ tone: 'error', text: 'Broken' });
    expect(statusOf(STRINGS, { failure: null, loading: true, comparison: null })?.tone).toBe('busy');
    expect(statusOf(STRINGS, { failure: null, loading: false, comparison: COMPARISON })).toEqual({ tone: 'warn', text: '3 differences' });
    const same = { ...COMPARISON, environments: COMPARISON.environments.slice(0, 2), differences: 0 };
    expect(statusOf(STRINGS, { failure: null, loading: false, comparison: same })).toEqual({ tone: 'ok', text: text(STRINGS, 'panel.same') });
    const one = { ...same, differences: 1 };
    expect(statusOf(STRINGS, { failure: null, loading: false, comparison: one })?.text).toBe('1 difference');
  });
});
