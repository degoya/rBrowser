import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { ChecksProgress, FlowRunState, PortalState } from '../ipc/protocol';
import type { ActivityInputs } from './activities';
import { entryView, type EntryData } from './entries';

/** The key and its arguments, so the entries are tested apart from the catalogue's wording. */
const t = (key: string, args?: Record<string, unknown>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function data(inputs: Partial<ActivityInputs>): EntryData {
  return {
    inputs: { state: sampleState(), flowRun: null, checks: null, portal: null, ...inputs },
    nowMs: 0,
    viewLabel: (view) => `view ${view}`,
  };
}

const FLOW_RUN: FlowRunState = {
  project: true,
  running: {
    runId: 'r',
    flow: 'login',
    views: [{ view: 1, status: 'passed' }, { view: 2, status: 'running' }],
    startedAtMs: 0,
  },
};

const CHECKS: ChecksProgress = {
  runId: 'c',
  elapsedMs: 0,
  jobs: [{ plugin: 'lighthouse', state: 'running' }],
  finished: false,
};

const PORTAL: PortalState = { enabled: true, running: true, devices: [] };

describe('the entries', () => {
  it('are none while their activity does not run', () => {
    for (const activity of ['video', 'recorder', 'flows', 'checks', 'portal', 'desync'] as const) {
      expect(entryView(activity, data({}), t), activity).toBeNull();
    }
    expect(entryView('checks', data({ checks: { ...CHECKS, finished: true } }), t)).toBeNull();
  });

  it('show a flow run by views, with Results and Stop', () => {
    const view = entryView('flows', data({ flowRun: FLOW_RUN }), t);
    expect(view?.short).toBe('1/2');
    expect(view?.text).toBe('shell.activity.flows.status {"flow":"login","done":1,"total":2}');
    expect(view?.actions.map((action) => [action.key, action.message, action.primary === true])).toEqual([
      ['results', { type: 'runCommand', id: 'flows.open_results' }, false],
      ['cancel', { type: 'runCommand', id: 'flows.cancel' }, true],
    ]);
    expect(view?.actions.at(-1)?.label).toBe('shell.activity.flows.stop');
  });

  it('say that closing a flow run only moves it to the toolbar, the run going on (RB-090-71)', () => {
    const view = entryView('flows', data({ flowRun: FLOW_RUN }), t);
    expect(view?.closeLabel).toBe('shell.activity.flows.fold');
    expect(view?.actions.some((action) => action.closes === true)).toBe(false);
  });

  it('name the flows of Run all still waiting (RB-090-71)', () => {
    const running = FLOW_RUN.running;
    if (running === undefined) throw new Error('a run');
    const queued: FlowRunState = { project: true, running: { ...running, queued: ['cart', 'search'] } };
    const view = entryView('flows', data({ flowRun: queued }), t);
    expect(view?.text).toBe('shell.activity.flows.status_queued {"flow":"login","done":1,"total":2,"queued":2}');
  });

  it('show a check run by jobs, with Cancel', () => {
    const view = entryView('checks', data({ checks: CHECKS }), t);
    expect(view?.short).toBe('0/1');
    expect(view?.actions.map((action) => action.message)).toEqual([{ type: 'runCommand', id: 'checks.cancel' }]);
  });

  it('show the portal by devices, with Devices, QR and Disconnect, which stops it', () => {
    const view = entryView('portal', data({ portal: PORTAL }), t);
    expect(view?.short).toBe('0');
    expect(view?.actions.map((action) => action.message)).toEqual([
      { type: 'runCommand', id: 'portal.devices' },
      { type: 'runCommand', id: 'portal.show_qr' },
      { type: 'runCommand', id: 'portal.toggle', args: { enabled: false } },
    ]);
    expect(view?.actions.at(-1)?.primary).toBe(true);
  });

  it('show a paused recording with Resume and a stopped one with Play last', () => {
    const state = sampleState();
    state.recorder = { phase: 'paused', name: 'login', steps: 2 };
    const paused = entryView('recorder', data({ state }), t);
    expect(paused?.actions.map((action) => action.label)).toEqual([
      'shell.activity.recorder.resume',
      'shell.activity.recorder.check',
      'shell.activity.recorder.stop',
    ]);
    const stopped = sampleState();
    stopped.activityBar = { recorded: 'login' };
    const offer = entryView('recorder', data({ state: stopped }), t);
    expect(offer?.actions).toMatchObject([{ key: 'play', closes: true, primary: true }]);
    expect(offer?.closeLabel).toBe('shell.activity.recorder.dismiss {"name":"login"}');
  });
});
