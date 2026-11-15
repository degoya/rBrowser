// A check run's progress as the findings panel shows it (RB-080-89), without the view: how many
// plugin x view jobs are through, one row per plugin with the running ones on top and only the
// views worth naming (RB-080-98), the run's tally for the line it collapses to, the time left
// from the estimate.

import type { CheckJob, CheckJobState, ChecksProgress } from './types';
import { FINDINGS_ICONS } from './icons';

/** The views of one state a plugin row names: running, failed or skipped. */
export interface AttentionGroup {
  state: 'running' | 'failed' | 'skipped';
  jobs: CheckJob[];
}

/** One plugin's row (RB-080-98): its jobs, one per view, summed up. */
export interface PluginProgress {
  plugin: string;
  jobs: CheckJob[];
  /** Where the plugin as a whole stands. */
  state: CheckJobState;
  /** Jobs through, of `total`. */
  ended: number;
  total: number;
  /** The findings its finished jobs kept. */
  findings: number;
  /** The views worth naming in the row, by state. */
  attention: AttentionGroup[];
}

const ENDED: readonly CheckJobState[] = ['done', 'failed', 'skipped'];
const ATTENTION: readonly AttentionGroup['state'][] = ['running', 'failed', 'skipped'];

/** Whether `job` is through. */
export function ended(job: CheckJob): boolean {
  return ENDED.includes(job.state);
}

/** The jobs through and all of them. */
export function progressCounts(progress: ChecksProgress): { done: number; total: number } {
  return { done: progress.jobs.filter(ended).length, total: progress.jobs.length };
}

/** A plugin as a whole: running while one view runs, waiting while one waits; once through,
 * done when one view got through, else failed when one failed, else skipped. */
export function pluginState(jobs: CheckJob[]): CheckJobState {
  const has = (state: CheckJobState) => jobs.some((job) => job.state === state);
  if (has('running')) {
    return 'running';
  }
  if (has('queued')) {
    return 'queued';
  }
  if (has('done') || jobs.length === 0) {
    return 'done';
  }
  return has('failed') ? 'failed' : 'skipped';
}

/** The running, failed and skipped views of `jobs`, one group per state, in that order. */
export function attention(jobs: CheckJob[]): AttentionGroup[] {
  return ATTENTION.map((state) => ({ state, jobs: jobs.filter((job) => job.state === state) })).filter(
    (group) => group.jobs.length > 0,
  );
}

/** Where a plugin stands: running first, then waiting, then through. */
function rank(state: CheckJobState): number {
  if (state === 'running') {
    return 0;
  }
  return state === 'queued' ? 1 : 2;
}

/** The plugins in plan order, the running ones on top, then the waiting ones. */
export function pluginRows(progress: ChecksProgress): PluginProgress[] {
  const grouped = new Map<string, CheckJob[]>();
  for (const job of progress.jobs) {
    const jobs = grouped.get(job.plugin) ?? [];
    jobs.push(job);
    grouped.set(job.plugin, jobs);
  }
  return [...grouped.entries()]
    .map(([plugin, jobs], index) => ({
      index,
      row: {
        plugin,
        jobs,
        state: pluginState(jobs),
        ended: jobs.filter(ended).length,
        total: jobs.length,
        findings: jobs.reduce((sum, job) => sum + (job.state === 'done' ? (job.findings ?? 0) : 0), 0),
        attention: attention(jobs),
      },
    }))
    .sort((a, b) => rank(a.row.state) - rank(b.row.state) || a.index - b.index)
    .map(({ row }) => row);
}

/** The skip reason of a job the cancel stopped (`checks.skip.cancelled`). */
export const SKIP_CANCELLED = 'checks.skip.cancelled';

/** A run in one line: its jobs, those that got to run (not stopped by a cancel), how many
 * failed, and how many were skipped for a reason of their own (engine, rules off). */
export function runTally(progress: ChecksProgress): { total: number; ran: number; failed: number; skipped: number } {
  const stopped = (job: CheckJob) => job.state === 'skipped' && job.reason === SKIP_CANCELLED;
  const count = (state: CheckJobState) => progress.jobs.filter((job) => job.state === state && !stopped(job)).length;
  const ran = progress.jobs.filter((job) => ended(job) && !stopped(job)).length;
  return { total: progress.jobs.length, ran, failed: count('failed'), skipped: count('skipped') };
}

/** Whole seconds of `ms`. */
export function wholeSeconds(ms: number): number {
  return Math.max(0, Math.floor(ms / 1000));
}

/** The seconds left of `estimateMs` after `elapsedMs`; `null` without an estimate or past it. */
export function secondsLeft(estimateMs: number | undefined, elapsedMs: number): number | null {
  if (estimateMs === undefined || estimateMs <= elapsedMs) {
    return null;
  }
  return Math.ceil((estimateMs - elapsedMs) / 1000);
}

/** The icon of a job's state. */
export const STATE_ICONS: Record<CheckJobState, string> = {
  queued: FINDINGS_ICONS.checkQueued,
  running: FINDINGS_ICONS.checkRunning,
  done: FINDINGS_ICONS.checkDone,
  failed: FINDINGS_ICONS.checkFailed,
  skipped: FINDINGS_ICONS.checkSkipped,
};
