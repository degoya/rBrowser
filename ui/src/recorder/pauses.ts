// The pause before each step at the pace "As recorded" (RB-090-84). The rule lives in the exporter
// (`plugins/recorder-export/src/pace.rs`): the core answers `getRecording` with the pauses the
// flow waits, and the panel shows them beside each step (read-only).

import type { StepPause } from '../ipc/protocol';

/** The pauses by step id. */
export function pauseMap(pauses: readonly StepPause[]): Map<number, number> {
  return new Map(pauses.map((pause) => [pause.step, pause.ms]));
}

/** A pause as seconds with one decimal (`2.5`). */
export function seconds(ms: number): string {
  return (Math.round(ms / 100) / 10).toFixed(1);
}
