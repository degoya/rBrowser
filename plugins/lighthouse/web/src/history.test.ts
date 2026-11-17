// The history's logic without the DOM (RB-091-02): run labels, the breakpoints and runs to pick
// from, the default pair to compare, and how a metric and its change read.
import { describe, expect, it } from 'vitest';
import { breakpointsOf, defaultPair, formatDelta, formatMetric, runLabel, runsWith, type HistoryRun } from './panel/history';

const RUNS: HistoryRun[] = [
  { runId: '20261112-100000', startedAt: 0, url: null, cancelled: false, breakpoints: [
    { breakpoint: 'mobile', scores: { performance: 80 }, report: '20261112-100000/mobile.html' },
    { breakpoint: 'desktop', scores: { performance: 90 }, report: '20261112-100000/desktop.html' },
  ] },
  { runId: '20261105-120000', startedAt: 0, url: null, cancelled: false, breakpoints: [
    { breakpoint: 'mobile', scores: { performance: 55 }, report: '20261105-120000/mobile.html' },
  ] },
  { runId: '20261101-090000', startedAt: 0, url: null, cancelled: true, breakpoints: [
    { breakpoint: 'desktop', scores: { performance: 70 }, report: '20261101-090000/desktop.html' },
    { breakpoint: 'tablet', scores: {}, report: '20261101-090000/tablet.html' },
  ] },
];

describe('the report history', () => {
  it('labels a run by its start time in UTC', () => {
    expect(runLabel('20261112-100000')).toBe('2026-11-12 10:00:00 UTC');
    expect(runLabel('odd')).toBe('odd');
  });

  it('offers every breakpoint once, in the order the runs name them', () => {
    expect(breakpointsOf(RUNS)).toEqual(['mobile', 'desktop', 'tablet']);
    expect(breakpointsOf([])).toEqual([]);
  });

  it('offers the runs that measured a breakpoint', () => {
    expect(runsWith(RUNS, 'desktop').map((run) => run.runId)).toEqual(['20261112-100000', '20261101-090000']);
    expect(runsWith(RUNS, 'phone')).toEqual([]);
  });

  it('compares the two newest runs of a breakpoint by default, the older one first', () => {
    expect(defaultPair(RUNS, 'desktop')).toEqual(['20261101-090000', '20261112-100000']);
    expect(defaultPair(RUNS, 'tablet')).toBeNull();
  });

  it('reads a metric in its unit and a change with its sign', () => {
    expect(formatMetric(1234.5, 'ms')).toBe('1,235 ms');
    expect(formatMetric(0.0213, '')).toBe('0.021');
    expect(formatMetric(null, 'ms')).toBe('-');
    expect(formatDelta(250, 'ms')).toBe('+250 ms');
    expect(formatDelta(-0.01, '')).toBe('-0.010');
    expect(formatDelta(0, 'ms')).toBe('0 ms');
    expect(formatDelta(null, 'ms')).toBe('');
  });
});
