// The steps of a running flow as progress (RB-090-84): each `step(name, fn)` reports its start
// and its end, so the Recorder panel follows a played recording step by step. A flow made from a
// recording names its steps `Step <id>: <kind>` (recorder-export, RB-090-76); `id` is that step's
// id in the recording. A failed step carries its error's first line - the lines after an
// `expect` failure quote the values compared, a form field's text -, masked like every result.

import { redact } from './result.ts';

/** The longest message of a failed step. */
export const MAX_STEP_MESSAGE = 300;

export type StepState = 'running' | 'passed' | 'failed';

const STATES: readonly StepState[] = ['running', 'passed', 'failed'];

/** One step's start or end, as the worker writes it and `$/progress` carries it. */
export interface StepProgress {
  name: string;
  /** The recording's step id, for a name `Step <id>: ...`. */
  id?: number;
  state: StepState;
  /** Why a failed step failed. */
  message?: string;
}

/** The recording's step id in `name` (`Step 7: click` is 7); null for any other name. */
export function stepIdOf(name: string): number | null {
  const match = /^Step (\d+):/.exec(name);
  return match?.[1] === undefined ? null : Number(match[1]);
}

/**
 * The first line of `message` that is not blank, masked and cut to [`MAX_STEP_MESSAGE`]; null
 * when there is none. The one place a flow's error is shortened: the step's progress and the
 * result's `summary` carry it, the core and the shell take it as it is (RB-090-84).
 */
export function firstLine(message: string, secrets: readonly string[]): string | null {
  const first = message
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line !== '');
  return first === undefined ? null : redact(first, secrets).slice(0, MAX_STEP_MESSAGE);
}

/** The progress of step `name` in `state`; a failed one with `error`'s first line, masked. */
export function stepProgress(name: string, state: StepState, error: unknown, secrets: readonly string[]): StepProgress {
  const id = stepIdOf(name);
  const progress: StepProgress = { name, ...(id === null ? {} : { id }), state };
  if (state === 'failed' && error !== undefined) {
    const first = firstLine(error instanceof Error ? error.message : String(error), secrets);
    if (first !== null) {
      progress.message = first;
    }
  }
  return progress;
}

/** A worker's step line as [`StepProgress`], only its known fields; null when it is none. */
export function stepProgressOf(value: unknown): StepProgress | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }
  const { name, id, state, message } = value as Record<string, unknown>;
  if (typeof name !== 'string' || !STATES.includes(state as StepState)) {
    return null;
  }
  const progress: StepProgress = { name: name.slice(0, 500), state: state as StepState };
  if (typeof id === 'number' && Number.isInteger(id) && id >= 0) {
    progress.id = id;
  }
  if (typeof message === 'string') {
    progress.message = message.slice(0, MAX_STEP_MESSAGE);
  }
  return progress;
}
