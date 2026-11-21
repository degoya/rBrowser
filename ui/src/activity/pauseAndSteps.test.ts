// RB-090-68: the video entry pauses and resumes, its time stands still while paused and leaves
// the pauses out; the flow run's entry names the step each running view is at, "step n/m".
import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { FlowRunState, FlowStep } from '../ipc/protocol';
import { runningSteps, videoSeconds, withStep, type ActivityInputs } from './activities';
import { entryView, type EntryData } from './entries';

const t = (key: string, args?: Record<string, unknown>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function data(inputs: Partial<ActivityInputs>, nowMs = 0): EntryData {
  return {
    inputs: { state: sampleState(), flowRun: null, checks: null, portal: null, ...inputs },
    nowMs,
    viewLabel: (view) => `view ${view}`,
  };
}

const RUN: FlowRunState = {
  project: true,
  running: {
    runId: 'r1',
    flow: 'login',
    views: [
      { view: 1, status: 'running' },
      { view: 2, status: 'queued' },
    ],
    startedAtMs: 0,
  },
};

function step(view: number, state: FlowStep['state'], extra: Partial<FlowStep> = {}): FlowStep {
  return { runId: 'r1', flow: 'login', view, name: 'fill', state, ...extra };
}

describe('the video time (RB-090-68)', () => {
  it('stands still at the pause and leaves the pauses that ended out', () => {
    const video = { views: [1], startedAtMs: 1000, maxDurationSec: 600 };
    expect(videoSeconds(video, 11_000)).toBe(10);
    expect(videoSeconds({ ...video, pausedMs: 4000 }, 11_000)).toBe(6);
    expect(videoSeconds({ ...video, pausedAtMs: 5000 }, 99_000)).toBe(4);
    expect(videoSeconds({ ...video, pausedAtMs: 9000, pausedMs: 2000 }, 99_000)).toBe(6);
  });
});

describe('the video entry (RB-090-68)', () => {
  it('records with Pause and Stop', () => {
    const state = sampleState();
    state.video = { views: [1], startedAtMs: 0, maxDurationSec: 600 };
    const view = entryView('video', data({ state }, 65_000), t);
    expect(view?.text).toBe('shell.activity.video.status {"time":"1:05","views":"view 1"}');
    expect(view?.color).toBe('error');
    expect(view?.actions.map((action) => [action.key, action.label, action.message, action.primary === true])).toEqual([
      ['pause', 'shell.activity.video.pause', { type: 'runCommand', id: 'capture.video.pause' }, false],
      ['stop', 'shell.activity.video.stop', { type: 'runCommand', id: 'capture.video.toggle' }, true],
    ]);
  });

  it('says paused, at the time of the pause, with Resume - in the bar and in the toolbar alike', () => {
    const state = sampleState();
    state.video = { views: [1, 2], startedAtMs: 0, maxDurationSec: 600, pausedAtMs: 30_000, pausedMs: 10_000 };
    const view = entryView('video', data({ state }, 90_000), t);
    expect(view?.text).toBe('shell.activity.video.paused {"time":"0:20","views":"view 1, view 2"}');
    expect(view?.short).toBe('0:20');
    expect(view?.icon).toBe('i-lucide-pause');
    expect(view?.color).toBe('neutral');
    expect(view?.actions[0]).toMatchObject({ key: 'pause', label: 'shell.activity.video.resume', icon: 'i-lucide-play' });
    expect(view?.actions.find((action) => action.primary === true)?.key).toBe('stop');
  });
});

describe('the flow steps (RB-090-68)', () => {
  it('take a pushed step into the run it belongs to', () => {
    const next = withStep(RUN, step(1, 'running', { number: 3, total: 7, name: 'Step 3: click' }));
    expect(next?.running?.views[0]).toEqual({ view: 1, status: 'running', step: { number: 3, name: 'Step 3: click' } });
    expect(next?.running?.totalSteps).toBe(7);
    expect(RUN.running?.views[0]?.step).toBeUndefined();
    // A step that ends changes no number; a step of another run or of no run changes nothing.
    expect(withStep(next, step(1, 'passed', { total: 7 }))?.running?.views[0]?.step?.number).toBe(3);
    expect(withStep(RUN, step(1, 'running', { runId: 'other', number: 1 }))).toBe(RUN);
    expect(withStep(null, step(1, 'running', { number: 1 }))).toBeNull();
  });

  it('list the running views at their step, n/m while n is within the total', () => {
    const at = (number: number, totalSteps?: number): FlowRunState => ({
      project: true,
      running: {
        ...RUN.running!,
        views: [
          { view: 1, status: 'running', step: { number, name: 'fill' } },
          { view: 2, status: 'passed', step: { number: 9, name: 'done' } },
        ],
        ...(totalSteps === undefined ? {} : { totalSteps }),
      },
    });
    expect(runningSteps(at(3, 7))).toEqual([{ view: 1, number: 3, total: 7, name: 'fill' }]);
    expect(runningSteps(at(8, 7))).toEqual([{ view: 1, number: 8, total: null, name: 'fill' }]);
    expect(runningSteps(at(2))).toEqual([{ view: 1, number: 2, total: null, name: 'fill' }]);
    expect(runningSteps(null)).toEqual([]);
  });

  it('name each running view\'s step in the flow entry', () => {
    const flowRun = withStep(RUN, step(1, 'running', { number: 3, total: 7, name: 'Step 3: click' }));
    const view = entryView('flows', data({ flowRun }), t);
    expect(view?.text).toBe(
      'shell.activity.flows.with_steps {"status":"shell.activity.flows.status {\\"flow\\":\\"login\\",\\"done\\":0,\\"total\\":2}",' +
        '"steps":"shell.activity.flows.step_of {\\"view\\":\\"view 1\\",\\"number\\":3,\\"total\\":7,\\"name\\":\\"Step 3: click\\"}"}',
    );
    expect(view?.short).toBe('3/7');
    const plain = entryView('flows', data({ flowRun: RUN }), t);
    expect(plain?.text).toBe('shell.activity.flows.status {"flow":"login","done":0,"total":2}');
    expect(plain?.short).toBe('0/2');
  });
});
