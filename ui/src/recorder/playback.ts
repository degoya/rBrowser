// Following a played recording in the Recorder panel (RB-090-84): the core pushes each step of
// the run going on (`flowStep`: the view, the recording's step id, running, passed or failed
// with its reason); the panel marks the step each view runs and every step that failed, red with
// the reason. When the run ends, its `result.json` (`flowRun.last`) names the step each failed
// view stopped at - a step pushed too late to arrive is still marked. The step id and the reason's
// first line come from the sidecar; the panel parses and shortens nothing. With
// `automation.recorder.followPlayback` (on by default) the running step scrolls into sight.

import type { FlowRunResult, FlowStep, JsonValue } from '../ipc/protocol';

/** The setting that scrolls the running step into sight. */
export const FOLLOW_KEY = 'automation.recorder.followPlayback';

/** `value` as the follow setting: on unless it is `false`. */
export function followOf(value: JsonValue | undefined): boolean {
  return value !== false;
}

/** What the panel shows of a run of the chosen recording. */
export interface Playback {
  /** The run followed; null before one started. */
  runId: string | null;
  /** By view: the step id it runs now. */
  running: Record<number, number>;
  /** By step id: why it failed (the first view's reason; '' without one). */
  failed: Record<number, string>;
}

export const NO_PLAYBACK: Playback = { runId: null, running: {}, failed: {} };

function without(running: Record<number, number>, view: number): Record<number, number> {
  return Object.fromEntries(Object.entries(running).filter(([key]) => Number(key) !== view));
}

/** `playback` after `step`; a step of another run starts over. */
export function withStep(playback: Playback, step: FlowStep): Playback {
  const base = step.runId === playback.runId ? playback : { ...NO_PLAYBACK, runId: step.runId };
  const id = step.stepId;
  if (id === undefined) {
    return base;
  }
  if (step.state === 'running') {
    return { ...base, running: { ...base.running, [step.view]: id } };
  }
  const running = base.running[step.view] === id ? without(base.running, step.view) : base.running;
  if (step.state === 'passed' || id in base.failed) {
    return { ...base, running };
  }
  return { ...base, running, failed: { ...base.failed, [id]: step.message ?? '' } };
}

/** `playback` once its run `last` ended: nothing runs; each failed view's step is marked. */
export function withResult(playback: Playback, last: FlowRunResult): Playback {
  const failed = last.runId === playback.runId ? { ...playback.failed } : {};
  for (const view of last.views) {
    const id = view.error?.stepId;
    if (id !== undefined && id !== null && !(id in failed)) {
      failed[id] = view.error?.summary ?? view.error?.message ?? '';
    }
  }
  return { runId: last.runId, running: {}, failed };
}

/** The step ids some view runs now. */
export function runningSteps(playback: Playback): Set<number> {
  return new Set(Object.values(playback.running));
}
