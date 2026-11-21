// The Recorder panel's editor edits of a recording (RB-092-01) - a step added at a place, a step
// changed, switched off or on, given a pause, duplicated - beside the edits of `recording.ts`
// (RB-090-60). Every edit answers a new recording and leaves the one given alone; the core checks
// the result on save (`rb_sync::recorder::edit::check`) as it does for the MCP tools.

import type { Action, Recording, Step } from './recording';
import { MAX_WAIT_MS } from './stepKinds';

/** Where a step goes: after the step with this id, `0` before the first, `null` at the end. */
export type After = number | null;

/** The id a new step gets: one above the highest. */
export function nextId(recording: Recording): number {
  return recording.steps.reduce((highest, step) => Math.max(highest, step.id), 0) + 1;
}

function indexAfter(recording: Recording, after: After): number {
  if (after === null) {
    return recording.steps.length;
  }
  if (after === 0) {
    return 0;
  }
  const at = recording.steps.findIndex((step) => step.id === after);
  return at < 0 ? recording.steps.length : at + 1;
}

/**
 * A step doing `action` for the place `after`: the next id, the view and time of the step
 * before it (the first step's at the start; the first breakpoint's when there is none), the
 * pause `pauseMs`.
 */
export function newStep(recording: Recording, after: After, action: Action, pauseMs: number | null): Step {
  const at = indexAfter(recording, after);
  const before = at > 0 ? recording.steps[at - 1] : recording.steps[0];
  const step: Step = {
    id: nextId(recording),
    action,
    view: before?.view ?? recording.breakpoints[0]?.id ?? '',
    atMs: before?.atMs ?? 0,
  };
  return pauseMs === null ? step : { ...step, pauseMs };
}

/** The recording with `step` at the place `after`. */
export function insertStep(recording: Recording, after: After, step: Step): Recording {
  const steps = [...recording.steps];
  steps.splice(indexAfter(recording, after), 0, step);
  return { ...recording, steps };
}

/** Step `id` replaced by `step` (its id kept). */
export function replaceStep(recording: Recording, id: number, step: Step): Recording {
  return {
    ...recording,
    steps: recording.steps.map((known) => (known.id === id ? { ...step, id } : known)),
  };
}

/** Step `id` switched off (`true`) or on; on is written as no field at all. */
export function setDisabled(recording: Recording, id: number, disabled: boolean): Recording {
  return {
    ...recording,
    steps: recording.steps.map((step) => {
      if (step.id !== id) {
        return step;
      }
      const { disabled: _old, ...rest } = step;
      return disabled ? { ...rest, disabled: true } : rest;
    }),
  };
}

/** A pause before a step as the editor takes it: whole ms from 0 to 10 s; `null` the recorded one. */
export function pauseOf(value: number | null | undefined): number | null {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return null;
  }
  return Math.min(MAX_WAIT_MS, Math.max(0, Math.round(value)));
}

/** `step` with the pause `pauseMs` before it; `null` takes the editor's pause away. */
export function withPause(step: Step, pauseMs: number | null): Step {
  const { pauseMs: _old, ...rest } = step;
  const pause = pauseOf(pauseMs);
  return pause === null ? rest : { ...rest, pauseMs: pause };
}

/** `step` running in the breakpoints `onlyIn`; none (or all of them) means every one. */
export function withOnlyIn(step: Step, onlyIn: string[], all: string[]): Step {
  const { onlyIn: _old, ...rest } = step;
  const every = all.length > 0 && all.every((id) => onlyIn.includes(id));
  return onlyIn.length === 0 || every ? rest : { ...rest, onlyIn };
}

/** The recording with a copy of step `id` right after it, under a new id, without its pointer way. */
export function duplicateStep(recording: Recording, id: number): Recording {
  const step = recording.steps.find((known) => known.id === id);
  if (step === undefined) {
    return recording;
  }
  const { path: _path, ...rest } = step;
  return insertStep(recording, id, { ...rest, id: nextId(recording) });
}
