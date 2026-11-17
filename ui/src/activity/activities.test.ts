import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { ChecksProgress, FlowRunState, PortalState } from '../ipc/protocol';
import {
  ACTIVITIES,
  barEntries,
  barSettings,
  checkCounts,
  drawnAt,
  elapsedSeconds,
  ended,
  flowCounts,
  running,
  wantedPlace,
  type ActivityInputs,
} from './activities';

const FLOW_RUN: FlowRunState = {
  project: true,
  running: {
    runId: 'r1',
    flow: 'login',
    views: [
      { view: 1, status: 'passed' },
      { view: 2, status: 'running' },
      { view: 3, status: 'failed' },
    ],
    startedAtMs: 0,
  },
};

const CHECKS: ChecksProgress = {
  runId: 'c1',
  elapsedMs: 900,
  jobs: [
    { plugin: 'seo', state: 'done' },
    { plugin: 'lighthouse', state: 'running' },
    { plugin: 'a11y', state: 'queued' },
  ],
  finished: false,
};

const PORTAL: PortalState = { enabled: true, running: true, devices: [] };

function everything(): ActivityInputs {
  const state = sampleState();
  state.video = { views: [1], startedAtMs: 1000, maxDurationSec: 600 };
  state.recorder = { phase: 'recording', name: 'login', steps: 2 };
  state.sync.desynced = [{ view: 2, reason: 'notFound', selector: 'nav', event: 'click' }];
  return { state, flowRun: FLOW_RUN, checks: CHECKS, portal: PORTAL };
}

describe('the activity bar settings', () => {
  it('default to the bar below the toolbar with every activity', () => {
    const settings = barSettings({});
    expect(settings.position).toBe('top');
    expect(settings.mode).toBe('bar');
    expect(ACTIVITIES.every((activity) => settings.show[activity])).toBe(true);
  });

  it('read the position, the mode and the switches', () => {
    const settings = barSettings({
      'ui.activityBar.position': 'bottom',
      'ui.activityBar.mode': 'indicator',
      'ui.activityBar.show.portal': false,
    });
    expect(settings).toMatchObject({ position: 'bottom', mode: 'indicator' });
    expect(settings.show.portal).toBe(false);
    expect(settings.show.video).toBe(true);
  });
});

describe('the running activities', () => {
  it('are none while nothing runs', () => {
    expect(running({ state: null, flowRun: null, checks: null, portal: null })).toEqual([]);
    expect(running({ state: sampleState(), flowRun: { project: true }, checks: null, portal: null })).toEqual([]);
  });

  it('come from the snapshot, the flow run, the checks and the portal, in the bar order', () => {
    expect(running(everything())).toEqual(['video', 'recorder', 'flows', 'checks', 'portal', 'desync']);
  });

  it('end with a finished check run and a stopped portal', () => {
    const inputs = { ...everything(), checks: { ...CHECKS, finished: true }, portal: { ...PORTAL, running: false } };
    expect(running(inputs)).not.toContain('checks');
    expect(running(inputs)).not.toContain('portal');
  });

  it('keep the recorder while it offers the recording just stopped', () => {
    const state = sampleState();
    state.activityBar = { recorded: 'login' };
    expect(running({ state, flowRun: null, checks: null, portal: null })).toEqual(['recorder']);
  });
});

describe('the bar entries and the room', () => {
  const run = running(everything());

  it('hold every running activity in bar mode, none in indicator mode', () => {
    expect(barEntries(run, barSettings({}), [])).toEqual(run);
    expect(barEntries(run, barSettings({ 'ui.activityBar.mode': 'indicator' }), [])).toEqual([]);
  });

  it('leave out an activity switched off or folded', () => {
    const settings = barSettings({ 'ui.activityBar.show.desync': false });
    expect(barEntries(run, settings, ['video'])).toEqual(['recorder', 'flows', 'checks', 'portal']);
  });

  it('ask for the settings position while there are entries, nothing otherwise', () => {
    expect(wantedPlace(['video'], barSettings({}))).toBe('top');
    expect(wantedPlace(['video'], barSettings({ 'ui.activityBar.position': 'bottom' }))).toBe('bottom');
    expect(wantedPlace([], barSettings({}))).toBeNull();
  });

  it('are drawn only by the page where the core made room', () => {
    expect(drawnAt('top', undefined, ['video'])).toEqual([]);
    expect(drawnAt('top', 'top', ['video'])).toEqual(['video']);
    expect(drawnAt('bottom', 'top', ['video'])).toEqual([]);
  });

  it('open a folded entry again once its activity ended', () => {
    expect(ended(['video', 'portal'], ['portal'])).toEqual(['video']);
    expect(ended([], ['portal'])).toEqual([]);
  });
});

describe('the counters', () => {
  it('count the video time in whole seconds', () => {
    expect(elapsedSeconds(1000, 62_999)).toBe(61);
    expect(elapsedSeconds(5000, 1000)).toBe(0);
  });

  it('count a flow run by views and a check run by jobs', () => {
    expect(flowCounts(FLOW_RUN)).toEqual({ flow: 'login', done: 2, total: 3 });
    expect(flowCounts({ project: true })).toBeNull();
    expect(checkCounts(CHECKS)).toEqual({ done: 1, total: 3 });
    expect(checkCounts(null)).toBeNull();
  });
});
