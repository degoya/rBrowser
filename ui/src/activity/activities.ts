// What the activity bar shows (RB-090-67), as plain functions of what the shell already gets:
// which activities run (the snapshot's `video`, `recorder`, `sync.desynced`, `timeControl`,
// `pluginProgress` and `forcedStates`, the Flows panel's `flowRun`, the checks' `checksProgress`, the portal's
// `getPortal`), which of them stand in the bar under the settings and the folded entries, where
// the shell asks the core for room, and the counters an entry shows. The components
// (`ActivityBar.vue`, `ActivityIndicators.vue`) and the page's store (`store.ts`) only wire
// these up.

import type { Activity, ActivityBarPosition, VideoActivity } from '../ipc/activity';
import type { ChecksProgress, FlowRunState, FlowStep, JsonValue, PortalState, StateSnapshot } from '../ipc/protocol';
import { isPresenting } from '../portal/presentation';

/** Every activity, in the bar's order. */
export const ACTIVITIES: readonly Activity[] = [
  'video',
  'recorder',
  'flows',
  'checks',
  'portal',
  'desync',
  'time',
  'plugins',
  'forcedStates',
];

export const POSITION_SETTING = 'ui.activityBar.position';
export const MODE_SETTING = 'ui.activityBar.mode';
/** `ui.activityBar.show.<activity>`. */
export const SHOW_SETTING_PREFIX = 'ui.activityBar.show.';

/** The activity bar's settings in effect. */
export interface BarSettings {
  position: ActivityBarPosition;
  /**
   * `always`: the bar keeps its room, empty while nothing runs (RB-094-13); `bar`: the bar only
   * while something runs; `indicator`: the toolbar's indicators alone.
   */
  mode: 'always' | 'bar' | 'indicator';
  show: Record<Activity, boolean>;
}

/** The settings from the shell's values; a key not set (or a value no build knows) is its default. */
function modeOf(value: JsonValue | undefined): BarSettings['mode'] {
  return value === 'indicator' || value === 'bar' ? value : 'always';
}

export function barSettings(values: Record<string, JsonValue>): BarSettings {
  const show = Object.fromEntries(
    ACTIVITIES.map((activity) => [activity, values[`${SHOW_SETTING_PREFIX}${activity}`] !== false]),
  ) as Record<Activity, boolean>;
  return {
    position: values[POSITION_SETTING] === 'bottom' ? 'bottom' : 'top',
    mode: modeOf(values[MODE_SETTING]),
    show,
  };
}

/** What the activities are read from. */
export interface ActivityInputs {
  state: StateSnapshot | null;
  flowRun: FlowRunState | null;
  checks: ChecksProgress | null;
  portal: PortalState | null;
}

/** Whether `activity` runs: the recorder also while it offers the recording just stopped. */
export function isRunning(activity: Activity, inputs: ActivityInputs): boolean {
  const { state } = inputs;
  switch (activity) {
    case 'video':
      return state?.video !== undefined;
    case 'recorder':
      return state?.recorder !== undefined || state?.activityBar?.recorded !== undefined;
    case 'flows':
      return inputs.flowRun?.running !== undefined;
    case 'checks':
      return inputs.checks !== null && !inputs.checks.finished;
    case 'portal':
      return inputs.portal?.running === true || isPresenting(inputs.portal?.presentation);
    case 'desync':
      return (state?.sync.desynced?.length ?? 0) > 0;
    case 'time':
      return state?.timeControl !== undefined;
    case 'plugins':
      return (state?.pluginProgress?.length ?? 0) > 0;
    case 'forcedStates':
      return (state?.forcedStates?.length ?? 0) > 0;
  }
}

/** The activities that run, in the bar's order. */
export function running(inputs: ActivityInputs): Activity[] {
  return ACTIVITIES.filter((activity) => isRunning(activity, inputs));
}

/** The entries the bar holds: running, a mode with the bar, switched on, not folded. */
export function barEntries(run: readonly Activity[], settings: BarSettings, collapsed: readonly Activity[]): Activity[] {
  if (settings.mode === 'indicator') {
    return [];
  }
  return run.filter((activity) => settings.show[activity] && !collapsed.includes(activity));
}

/** The place the shell asks the core for: the settings' position while the bar has entries. */
export function wantedPlace(entries: readonly Activity[], settings: BarSettings): ActivityBarPosition | null {
  if (settings.mode === 'indicator') return null;
  return settings.mode === 'always' || entries.length > 0 ? settings.position : null;
}

/** The entries the page at `place` draws: none until the core made room there. */
export function drawnAt(
  place: ActivityBarPosition,
  granted: ActivityBarPosition | undefined,
  entries: readonly Activity[],
): Activity[] {
  return granted === place ? [...entries] : [];
}

/** The folded entries whose activity ended: the shell opens them again for the next run. */
export function ended(collapsed: readonly Activity[], run: readonly Activity[]): Activity[] {
  return collapsed.filter((activity) => !run.includes(activity));
}

/** Whole seconds since `startedAtMs`, never below 0. */
export function elapsedSeconds(startedAtMs: number, nowMs: number): number {
  return Math.max(0, Math.floor((nowMs - startedAtMs) / 1000));
}

/** A video's recorded seconds: to now or to its pause, the pauses that ended left out (RB-090-68). */
export function videoSeconds(video: VideoActivity, nowMs: number): number {
  return elapsedSeconds(video.startedAtMs + (video.pausedMs ?? 0), video.pausedAtMs ?? nowMs);
}

/** `step` of the run going on taken into `flowRun`: the view's step and the run's total (RB-090-68). */
export function withStep(flowRun: FlowRunState | null, step: FlowStep): FlowRunState | null {
  const run = flowRun?.running;
  if (flowRun === null || run === undefined || run.runId !== step.runId) return flowRun;
  const number = step.number;
  const views =
    number === undefined
      ? run.views
      : run.views.map((view) => (view.view === step.view ? { ...view, step: { number, name: step.name } } : view));
  const total = step.total === undefined ? {} : { totalSteps: step.total };
  return { ...flowRun, running: { ...run, views, ...total } };
}

/** One running view's step: `total` while the number is within the flow's steps, else `null`. */
export interface RunningStep {
  view: number;
  number: number;
  total: number | null;
  name: string;
}

/** The steps the running views of a flow run are at (RB-090-68). */
export function runningSteps(flowRun: FlowRunState | null): RunningStep[] {
  const run = flowRun?.running;
  if (run === undefined) return [];
  const total = run.totalSteps;
  return run.views.flatMap((view) =>
    view.status === 'running' && view.step !== undefined
      ? [
          {
            view: view.view,
            number: view.step.number,
            total: total !== undefined && view.step.number <= total ? total : null,
            name: view.step.name,
          },
        ]
      : [],
  );
}

/** A flow run's views through and all of them. */
export function flowCounts(flowRun: FlowRunState | null): { flow: string; done: number; total: number } | null {
  const run = flowRun?.running;
  if (run === undefined) {
    return null;
  }
  const done = run.views.filter((view) => view.status !== 'running' && view.status !== 'queued').length;
  return { flow: run.flow, done, total: run.views.length };
}

/** A check run's jobs through (done, failed or skipped) and all of them. */
export function checkCounts(progress: ChecksProgress | null): { done: number; total: number } | null {
  if (progress === null) {
    return null;
  }
  const done = progress.jobs.filter((job) => job.state !== 'queued' && job.state !== 'running').length;
  return { done, total: progress.jobs.length };
}
