import { describe, expect, it } from 'vitest';

import { MAX_STEP_MESSAGE, firstLine, stepIdOf, stepProgress, stepProgressOf } from './steps.ts';

const TOKEN = 'c0ffee'.repeat(11);

describe('stepIdOf (RB-090-84)', () => {
  it("reads the recording's step id from the name the exporter writes", () => {
    expect(stepIdOf('Step 7: click')).toBe(7);
    expect(stepIdOf('Step 12: fill - Email')).toBe(12);
    expect(stepIdOf('log in')).toBeNull();
    expect(stepIdOf('Step x: click')).toBeNull();
    expect(stepIdOf(' Step 3: click')).toBeNull();
  });
});

describe('stepProgress', () => {
  it('names a running and a passed step with its id', () => {
    expect(stepProgress('Step 3: hover', 'running', undefined, [])).toEqual({ name: 'Step 3: hover', id: 3, state: 'running' });
    expect(stepProgress('log in', 'passed', undefined, [])).toEqual({ name: 'log in', state: 'passed' });
  });

  it("gives a failed step its error's first line, masked and cut", () => {
    const error = new Error(`\n  locator.click: Timeout 10000ms exceeded with Bearer ${TOKEN}\nCall log:\n  - waiting`);
    expect(stepProgress('Step 4: click', 'failed', error, [TOKEN])).toEqual({
      name: 'Step 4: click',
      id: 4,
      state: 'failed',
      message: 'locator.click: Timeout 10000ms exceeded with Bearer <redacted>',
    });
    const long = stepProgress('Step 4: click', 'failed', 'x'.repeat(MAX_STEP_MESSAGE + 20), []);
    expect(long.message).toHaveLength(MAX_STEP_MESSAGE);
    expect(stepProgress('Step 4: click', 'failed', undefined, []).message).toBeUndefined();
  });
});

describe('stepProgressOf', () => {
  it("takes a worker's step line as it is meant and nothing else", () => {
    expect(stepProgressOf({ name: 'Step 3: click', id: 3, state: 'failed', message: 'boom', extra: 1 })).toEqual({
      name: 'Step 3: click',
      id: 3,
      state: 'failed',
      message: 'boom',
    });
    expect(stepProgressOf({ name: 'log in', state: 'running' })).toEqual({ name: 'log in', state: 'running' });
    expect(stepProgressOf({ name: 'log in', state: 'done' })).toBeNull();
    expect(stepProgressOf({ name: 3, state: 'running' })).toBeNull();
    expect(stepProgressOf({ name: 'x', id: -1, state: 'running' })).toEqual({ name: 'x', state: 'running' });
    expect(stepProgressOf(null)).toBeNull();
    expect(stepProgressOf('Step 3')).toBeNull();
  });
});

describe('firstLine', () => {
  it('is the first line that is not blank, masked and cut, or none', () => {
    expect(firstLine(`\n  locator.click: no ${TOKEN}\nExpected: "x"`, [TOKEN])).toBe('locator.click: no <redacted>');
    expect(firstLine('x'.repeat(MAX_STEP_MESSAGE + 5), [])).toHaveLength(MAX_STEP_MESSAGE);
    expect(firstLine(' \n \n', [])).toBeNull();
  });
});
