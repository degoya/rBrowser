// The globals a flow gets (RB-040-07): `page` (the view's Playwright page), `view` (its facts),
// `expect` (from @playwright/test), `env` (the environment URL) and `step(name, fn)`, which names
// a part of the flow: its duration lands in the result, and an error inside it names the step.

import type { FlowView, StepResult } from './result.ts';

export interface Steps {
  /** The steps finished so far, in order. */
  readonly done: StepResult[];
  /** The innermost step running now. */
  current(): string | null;
  step<T>(name: string, fn: () => T | Promise<T>): Promise<T>;
}

export function createSteps(now: () => number): Steps {
  const done: StepResult[] = [];
  const running: string[] = [];
  return {
    done,
    current: () => running.at(-1) ?? null,
    async step<T>(name: string, fn: () => T | Promise<T>): Promise<T> {
      if (typeof name !== 'string' || typeof fn !== 'function') {
        throw new TypeError('step(name, fn) takes a name and a function');
      }
      const start = now();
      running.push(name);
      try {
        const value = await fn();
        done.push({ name, status: 'passed', durationMs: now() - start });
        running.pop();
        return value;
      } catch (error) {
        // Left on the stack: the run reads `current()` for the error's step.
        done.push({ name, status: 'failed', durationMs: now() - start });
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
