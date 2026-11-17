// What the activity bar shows (RB-090-67), as plain functions of what the shell already gets:
// which activities run (the snapshot's `video`, `recorder` and `sync.desynced`, the Flows panel's
// `flowRun`, the checks' `checksProgress`, the portal's `getPortal`), which of them stand in the
// bar under the settings and the folded entries, where the shell asks the core for room, and the
// counters an entry shows. The components (`ActivityBar.vue`, `ActivityIndicators.vue`) and the
// page's store (`store.ts`) only wire these up.

import type { Activity, ActivityBarPosition } from '../ipc/activity';
import type { ChecksProgress, FlowRunState, JsonValue, PortalState, StateSnapshot } from '../ipc/protocol';

/** Every activity, in the bar's order. */
export const ACTIVITIES: readonly Activity[] = ['video', 'recorder', 'flows', 'checks', 'portal', 'desync'];

export const POSITION_SETTING = 'ui.activityBar.position';
export const MODE_SETTING = 'ui.activityBar.mode';
/** `ui.activityBar.show.<activity>`. */
export const SHOW_SETTING_PREFIX = 'ui.activityBar.show.';

/** The activity bar's settings in effect. */
export interface BarSettings {
  position: ActivityBarPosition;
  /** `bar`: the bar with the controls; `indicator`: the toolbar's indicators alone. */
  mode: 'bar' | 'indicator';
  show: Record<Activity, boolean>;
}

/** The settings from the shell's values; a key not set (or a value no build knows) is its default. */
export function barSettings(values: Record<string, JsonValue>): BarSettings {
  const show = Object.fromEntries(
    ACTIVITIES.map((activity) => [activity, values[`${SHOW_SETTING_PREFIX}${activity}`] !== false]),
  ) as Record<Activity, boolean>;
  return {
    position: values[POSITION_SETTING] === 'bottom' ? 'bottom' : 'top',
    mode: values[MODE_SETTING] === 'indicator' ? 'indicator' : 'bar',
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
      return inputs.portal?.running === true;
    case 'desync':
      return (state?.sync.desynced?.length ?? 0) > 0;
  }
}

/** The activities that run, in the bar's order. */
export function running(inputs: ActivityInputs): Activity[] {
  return ACTIVITIES.filter((activity) => isRunning(activity, inputs));
}

/** The entries the bar holds: running, the mode `bar`, switched on, not folded. */
export function barEntries(run: readonly Activity[], settings: BarSettings, collapsed: readonly Activity[]): Activity[] {
  if (settings.mode !== 'bar') {
    return [];
  }
  return run.filter((activity) => settings.show[activity] && !collapsed.includes(activity));
}

/** The place the shell asks the core for: the settings' position while the bar has entries. */
export function wantedPlace(entries: readonly Activity[], settings: BarSettings): ActivityBarPosition | null {
  return entries.length > 0 && settings.mode === 'bar' ? settings.position : null;
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
