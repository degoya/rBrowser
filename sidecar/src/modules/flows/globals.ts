// The globals a flow gets (RB-040-07): `page` (the view's Playwright page), `view` (its facts),
// `expect` (from @playwright/test), `env` (the environment URL) and `step(name, fn)`, which names
// a part of the flow: its duration lands in the result, and an error inside it names the step.
// A step the flow's timeout cut off is listed as failed (`abort`, RB-090-76). Each step's start
// and end go to `onStep` as they happen (RB-090-84) - none after the timeout.

import type { FlowView, StepResult } from './result.ts';
import type { StepState } from './steps.ts';

/** A step started (`running`) or ended; a failed one with its error. */
export type OnStep = (name: string, state: StepState, error?: unknown) => void;

export interface Steps {
  /** The steps finished so far, in order. */
  readonly done: StepResult[];
  /** The innermost step running now. */
  current(): string | null;
  /** The innermost step `error` came out of; else the one running now. */
  stepOf(error: unknown): string | null;
  /** The flow ran out of time (`reason`): the steps still running are failed, nothing is added later. */
  abort(reason?: unknown): void;
  step<T>(name: string, fn: () => T | Promise<T>): Promise<T>;
}

export function createSteps(now: () => number, onStep: OnStep = () => {}): Steps {
  const done: StepResult[] = [];
  const running: { name: string; start: number }[] = [];
  // The innermost step each error came out of (a thrown value that is no object: the last).
  const origin = new WeakMap<object, string>();
  let lastThrown: { value: unknown; name: string } | null = null;
  let aborted = false;
  const end = (name: string, status: StepResult['status'], start: number, error?: unknown) => {
    if (!aborted) {
      done.push({ name, status, durationMs: now() - start });
      onStep(name, status === 'passed' ? 'passed' : 'failed', error);
    }
  };
  return {
    done,
    current: () => running.at(-1)?.name ?? null,
    stepOf(error) {
      if (typeof error === 'object' && error !== null && origin.has(error)) {
        return origin.get(error) ?? null;
      }
      if (lastThrown !== null && Object.is(lastThrown.value, error)) {
        return lastThrown.name;
      }
      return running.at(-1)?.name ?? null;
    },
    abort(reason) {
      for (const { name, start } of running) {
        end(name, 'failed', start, reason);
      }
      aborted = true;
    },
    async step<T>(name: string, fn: () => T | Promise<T>): Promise<T> {
      if (typeof name !== 'string' || typeof fn !== 'function') {
        throw new TypeError('step(name, fn) takes a name and a function');
      }
      const start = now();
      running.push({ name, start });
      if (!aborted) {
        onStep(name, 'running');
      }
      try {
        const value = await fn();
        end(name, 'passed', start);
        running.pop();
        return value;
      } catch (error) {
        // Off the stack, so a step whose error the flow catches is not reported again; the
        // innermost step stays the error's (`stepOf`).
        end(name, 'failed', start, error);
        running.pop();
        if (typeof error === 'object' && error !== null) {
          if (!origin.has(error)) {
            origin.set(error, name);
          }
        } else if (lastThrown === null || !Object.is(lastThrown.value, error)) {
          lastThrown = { value: error, name };
        }
        throw error;
      }
    },
  };
}

/** The arguments of a compiled flow, in the order of `GLOBALS`. */
export function flowArguments(
  page: unknown,
  view: FlowView,
  expect: unknown,
  env: string | null,
  steps: Steps,
): unknown[] {
  const frozen = Object.freeze({ ...view });
  return [page, frozen, expect, env, steps.step.bind(steps)];
}
