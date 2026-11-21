// The steps of the recording that runs, live in the Recorder panel (RB-090-89,
// `automation.recorder.showLiveSteps`): the core pushes `recordingSteps` - the steps from the
// first place that changed, a new step or one cleaned up - and says when the list ends: `ended`
// once the recording is saved, `hidden` when the setting went off. The pause beside each step is the one the person took before it, as Play
// keeps it at the pace As recorded (at most 10 s); the exporter's finer rule (the hover dwell,
// the least pause) shows once the recording is saved.

import type { RecordingSteps } from '../ipc/protocol';
import type { Step } from './recording';

/** The longest pause Play keeps (the exporter's `MAX_PAUSE_MS`). */
export const MAX_PAUSE_MS = 10_000;

/** The recording that runs and its steps so far. */
export interface LiveRecording {
  name: string;
  steps: Step[];
  /** It continues the stored recording after this step (RB-092-01). */
  continueAfter?: number;
}

/** `live` after the push `steps`: its steps from `from` on replaced; an end lets it go. */
export function withLiveSteps(live: LiveRecording | null, steps: RecordingSteps): LiveRecording | null {
  if (steps.state !== 'recording') {
    return null;
  }
  const kept = live !== null && live.name === steps.name ? live.steps.slice(0, steps.from) : [];
  const next = { name: steps.name, steps: [...kept, ...steps.steps] };
  return steps.continueAfter === undefined ? next : { ...next, continueAfter: steps.continueAfter };
}

/** By step id: the pause recorded before it, at most [`MAX_PAUSE_MS`]; none before the first. */
export function livePauses(steps: readonly Step[]): Map<number, number> {
  const pauses = new Map<number, number>();
  steps.forEach((step, index) => {
    const before = index === 0 ? undefined : steps[index - 1];
    const pause = before === undefined ? 0 : Math.min(Math.max(step.atMs - before.atMs, 0), MAX_PAUSE_MS);
    pauses.set(step.id, pause);
  });
  return pauses;
}
