import { describe, expect, it } from 'vitest';
import { attention, pluginRows, pluginState, progressCounts, runTally, secondsLeft, wholeSeconds } from './progress';
import { gridProgress, sampleProgress } from './testing';
import type { CheckJob } from './types';

function job(state: CheckJob['state'], view: number, findings?: number): CheckJob {
  return { plugin: 'seo', view, breakpoint: `bp${view}`, state, findings };
}

describe('the progress of a check run', () => {
  it('counts the jobs through', () => {
    expect(progressCounts(sampleProgress())).toEqual({ done: 2, total: 4 });
  });

  it('puts the running plugin on top, the rest in plan order', () => {
    expect(pluginRows(sampleProgress()).map((row) => row.plugin)).toEqual(['a11y-axe', 'seo']);
    const through = sampleProgress({
      jobs: sampleProgress().jobs.map((job) => ({ ...job, state: 'done' as const })),
    });
    expect(pluginRows(through).map((row) => row.plugin)).toEqual(['seo', 'a11y-axe']);
  });

  it('puts the waiting plugins between the running and the finished ones', () => {
    const rows = pluginRows(gridProgress());
    const states = rows.map((row) => row.state);
    expect(states.indexOf('queued')).toBeGreaterThan(states.lastIndexOf('running'));
    expect(states.lastIndexOf('queued')).toBeLessThan(states.indexOf('done'));
  });

  it('sums each plugin into one row: its state, views through and findings', () => {
    const rows = pluginRows(sampleProgress());
    expect(rows[1]).toMatchObject({ plugin: 'seo', state: 'done', ended: 2, total: 2, findings: 1 });
    expect(rows[0]).toMatchObject({ plugin: 'a11y-axe', state: 'running', ended: 0, total: 2, findings: 0 });
  });

  it('calls a plugin failed or skipped only when no view of it got through', () => {
    expect(pluginState([job('done', 1), job('failed', 2)])).toBe('done');
    expect(pluginState([job('failed', 1), job('skipped', 2)])).toBe('failed');
    expect(pluginState([job('skipped', 1), job('skipped', 2)])).toBe('skipped');
    expect(pluginState([job('done', 1), job('queued', 2)])).toBe('queued');
    expect(pluginState([job('running', 1), job('queued', 2)])).toBe('running');
  });

  it('names only the running, failed and skipped views, one group per state', () => {
    const groups = attention([job('done', 1, 3), job('skipped', 2), job('running', 3), job('failed', 4), job('running', 5)]);
    expect(groups.map((group) => [group.state, group.jobs.map((each) => each.view)])).toEqual([
      ['running', [3, 5]],
      ['failed', [4]],
      ['skipped', [2]],
    ]);
    expect(attention([job('done', 1), job('queued', 2)])).toEqual([]);
  });

  it('tallies a run for its summary line', () => {
    expect(runTally(gridProgress({ finished: true }))).toEqual({ total: 99, ran: 47, failed: 1, skipped: 3 });
  });

  it('counts the jobs a cancel stopped neither as run nor as skipped', () => {
    const progress = sampleProgress({
      finished: true,
      cancelled: true,
      jobs: sampleProgress().jobs.map((job) =>
        job.state === 'done' ? job : { ...job, state: 'skipped' as const, reason: 'checks.skip.cancelled' },
      ),
    });
    expect(runTally(progress)).toEqual({ total: 4, ran: 2, failed: 0, skipped: 0 });
  });

  it('tells the time left only with an estimate not yet passed', () => {
    expect(secondsLeft(40_000, 12_400)).toBe(28);
    expect(secondsLeft(undefined, 12_400)).toBeNull();
    expect(secondsLeft(10_000, 12_400)).toBeNull();
    expect(wholeSeconds(12_999)).toBe(12);
  });
});
