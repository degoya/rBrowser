import { describe, expect, it } from 'vitest';
import type { FlowRunResult, FlowStep } from '../ipc/protocol';
import { NO_PLAYBACK, followOf, runningSteps, withResult, withStep } from './playback';

const RUN = '20261118-185842-a1b2c3';

function step(view: number, stepId: number, state: FlowStep['state'], message?: string): FlowStep {
  return { runId: RUN, flow: 'login', view, name: `Step ${stepId}: click`, stepId, state, ...(message ? { message } : {}) };
}

describe('following a played recording (RB-090-84)', () => {
  it('follows by default', () => {
    expect(followOf(undefined)).toBe(true);
    expect(followOf(true)).toBe(true);
    expect(followOf(false)).toBe(false);
  });

  it('marks the step each view runs and lets go of it when it passed', () => {
    let playback = withStep(NO_PLAYBACK, step(1, 3, 'running'));
    playback = withStep(playback, step(2, 2, 'running'));
    expect(runningSteps(playback)).toEqual(new Set([2, 3]));
    playback = withStep(playback, step(1, 3, 'passed'));
    expect(playback.running).toEqual({ 2: 2 });
    expect(playback.failed).toEqual({});
  });

  it("marks a failed step with the first view's reason as the sidecar shortened it", () => {
    let playback = withStep(NO_PLAYBACK, step(1, 5, 'running'));
    playback = withStep(playback, step(1, 5, 'failed', 'locator.click: Timeout 10000ms exceeded.'));
    playback = withStep(playback, step(2, 5, 'failed', 'another reason'));
    expect(playback.failed).toEqual({ 5: 'locator.click: Timeout 10000ms exceeded.' });
    expect(playback.running).toEqual({});
  });

  it('starts over with a step of another run and ignores a step without an id', () => {
    const before = withStep(NO_PLAYBACK, step(1, 5, 'failed', 'x'));
    const next = withStep(before, { ...step(1, 1, 'running'), runId: 'other' });
    expect(next).toEqual({ runId: 'other', running: { 1: 1 }, failed: {} });
    // A flow's own step (no recording's id) marks nothing; the name is never parsed.
    expect(withStep(next, { ...step(1, 1, 'running'), runId: 'other', name: 'Step 9: x', stepId: undefined })).toEqual(
      next,
    );
  });

  it("marks what the run's result names once it ended", () => {
    const last: FlowRunResult = {
      version: 1,
      runId: RUN,
      flow: 'login',
      file: 'login.json',
      startedAt: '2026-11-18T18:58:42Z',
      durationMs: 23_400,
      passed: false,
      views: [
        {
          viewId: 1,
          label: 'W1600',
          engine: 'chromium',
          status: 'failed',
          durationMs: 23_000,
          steps: [],
          error: {
            message: '\nlocator.click: Timeout 10000ms exceeded.\nCall log:',
            step: 'Step 5: click',
            stepId: 5,
            summary: 'locator.click: Timeout 10000ms exceeded.',
          },
        },
        { viewId: 2, label: 'Mobile', engine: 'chromium', status: 'passed', durationMs: 9_000, steps: [] },
      ],
    };
    const ended = withResult(withStep(NO_PLAYBACK, step(1, 5, 'running')), last);
    expect(ended).toEqual({ runId: RUN, running: {}, failed: { 5: 'locator.click: Timeout 10000ms exceeded.' } });
  });
});
