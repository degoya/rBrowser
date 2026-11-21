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
    for (const activity of ['video', 'recorder', 'flows', 'checks', 'portal', 'desync', 'time'] as const) {
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

  it('show a fixed clock with Real time, and stopped animations with Step and 100 % (RB-094-07)', () => {
    const state = sampleState();
    state.timeControl = {
      clock: { start: '2026-12-24T10:00:00', local: true, speed: 0 },
      animations: [
        { view: 1, rate: 0 },
        { view: 2, rate: 0 },
        { view: 3, rate: 0.25 },
      ],
    };
    // The key and its arguments' values, so the nested texts read plainly.
    const plain = (key: string, args?: Record<string, unknown>): string =>
      args === undefined ? key : `${key}(${Object.values(args).map(String).join('|')})`;
    const view = entryView('time', data({ state }), plain);
    expect(view?.icon).toBe('i-lucide-clock');
    expect(view?.short).toBe('10:00');
    expect(view?.text).toBe(
      'shell.activity.time.both(shell.activity.time.clock_fixed(2026-12-24 10:00)|' +
        'shell.activity.time.animations(shell.activity.time.rate_views(0|view 1, view 2); ' +
        'shell.activity.time.rate_views(25|view 3)))',
    );
    expect(view?.actions.map((action) => [action.key, action.message, action.primary === true])).toEqual([
      ['step', { type: 'runCommand', id: 'overlays.animations', args: { target: 'all', step: true } }, false],
      ['normal', { type: 'runCommand', id: 'overlays.animations', args: { target: 'all', rate: 1 } }, false],
      ['realTime', { type: 'runCommand', id: 'emulation.clock', args: { reset: true } }, true],
    ]);
  });

  it('show a running clock alone, and slowed animations alone with 100 % as the primary', () => {
    const clock = sampleState();
    clock.timeControl = { clock: { start: '2026-12-24T09:00:00.000Z', local: false, speed: 2 } };
    const running = entryView('time', data({ state: clock }), t);
    expect(running?.text).toBe('shell.activity.time.clock_running {"start":"2026-12-24 09:00 UTC","speed":2}');
    expect(running?.actions.map((action) => action.key)).toEqual(['realTime']);
    const slow = sampleState();
    slow.timeControl = { animations: [{ view: 2, rate: 0.1 }] };
    const slowed = entryView('time', data({ state: slow }), t);
    expect(slowed?.icon).toBe('i-lucide-gauge');
    expect(slowed?.short).toBe('shell.activity.time.percent {"percent":"10"}');
    expect(slowed?.actions.map((action) => [action.key, action.primary === true])).toEqual([['normal', true]]);
  });
});

describe('the presentation over the internet in the portal entry (RB-096-05)', () => {
  const LINK = 'https://quick-brown-fox.trycloudflare.com/__rbrowser/join#t=00';
  const presenting = (phase: 'starting' | 'live', link = true): PortalState => ({
    enabled: false,
    running: false,
    devices: [],
    presentation: {
      enabled: true,
      consented: true,
      phase,
      viewers: 2,
      maxViewers: 3,
      link: link ? { url: LINK, size: 1, rows: ['1'], expiresAtMs: 90_000 } : undefined,
    },
  });

  it('shows the viewers, the link time, Copy link, New link and End', () => {
    const view = entryView('portal', data({ portal: presenting('live') }), t);
    expect(view?.text).toBe('shell.activity.portal.present_status_link {"count":2,"time":"1:30"}');
    expect(view?.short).toBe('2');
    expect(view?.actions.map((action) => action.key)).toEqual(['copy', 'newLink', 'end']);
    expect(view?.actions[0]?.copy).toBe(LINK);
    expect(view?.actions[1]?.message).toEqual({ type: 'runCommand', id: 'portal.present.new_link' });
    expect(view?.actions[2]).toMatchObject({
      primary: true,
      message: { type: 'runCommand', id: 'portal.present.stop' },
    });
  });

  it('copies nothing once the link is spent, and ends a presentation still starting', () => {
    const spent = entryView('portal', data({ portal: presenting('live', false) }), t);
    expect(spent?.text).toBe('shell.activity.portal.present_status {"count":2}');
    expect(spent?.actions.map((action) => action.key)).toEqual(['newLink', 'end']);
    const starting = entryView('portal', data({ portal: presenting('starting', false) }), t);
    expect(starting?.text).toBe('shell.activity.portal.present_starting');
    expect(starting?.actions.map((action) => action.key)).toEqual(['end']);
  });
});
