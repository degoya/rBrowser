// What the toolbar shows while a recording runs (RB-050-01): recording or paused, the name and
// the step count. The core's snapshot carries `recorder: { phase, name, steps }` only while a
// recording runs.

import type { StateSnapshot } from '../ipc/protocol';

export interface RecorderIndicator {
  /** Steps are suppressed until it resumes. */
  paused: boolean;
  /** The recording's name. */
  name: string;
  /** The steps so far. */
  steps: number;
}

/** The indicator for `state`; `null` while no recording runs. */
export function recorderIndicator(state: StateSnapshot | null): RecorderIndicator | null {
  const recorder = state?.recorder;
  if (recorder === undefined) {
    return null;
  }
  return { paused: recorder.phase === 'paused', name: recorder.name, steps: recorder.steps };
}
