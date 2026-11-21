// The load test panel in its frame (RB-096-07): mounted as the panel frame mounts it; the panels
// window is played by spying on postMessage and dispatching `message` events from window.parent
// (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import { HOST_NOT_CONFIRMED } from './model';

type Op = { op?: string } & Record<string, unknown>;
type Posted = { type?: string; id?: number; tool?: string; args?: { op?: Op } };

const STRINGS = {
  'plugin.load-test.panels.load-test': 'Load test',
  'plugin.load-test.panel.ownSystems': 'Only test systems you own or may test.',
  'plugin.load-test.panel.summary.confirmBox': '{host} is mine, or I may test it',
  'plugin.load-test.errors.host_not_confirmed': 'Confirm {host} first.',
  'plugin.load-test.panel.live.usersValue': '{users} of {target}',
  'plugin.load-test.panel.live.report': 'Report: {id}',
  'plugins.panel.call_failed': 'The plugin could not do that.',
  'plugin.load-test.errors.over_limit': 'More than the settings allow ({setting}).',
  'plugin.load-test.panel.setting_link': 'Open the setting',
};

const SCENARIO = {
  name: 'Smoke',
  order: 'sequence',
  steps: [{ request: 'ping.http', weight: 1, thinkMs: { min: 0, max: 0 } }],
  profile: { users: 50, rampUpS: 10, holdS: 40, rampDownS: 10 },
  abort: { windowS: 10, minRequests: 20 },
};

const STATE = {
  trusted: true,
  environment: null,
  environments: ['local'],
  requests: [{ id: 'ping.http', name: 'Ping', method: 'GET', url: 'http://localhost:3000/ping' }],
  scenarios: [{ id: 'smoke.json', name: 'Smoke' }],
  limits: { maxUsers: 50, maxRps: 200, maxDurationS: 300 },
  hosts: { own: ['localhost'], local: ['localhost', '127.0.0.1', '[::1]'], confirmed: [] },
  running: false,
  run: null,
  reports: [],
};

const confirm = { targets: ['localhost', 'perf.example'], users: 50, rps: 200, durationS: 60 };
const SUMMARY = {
  scenario: 'smoke.json',
  name: 'Smoke',
  environment: null,
  targets: [
    { host: 'localhost', standing: 'local' },
    { host: 'perf.example', standing: 'unconfirmed' },
  ],
  unconfirmed: ['perf.example'],
  users: 50,
  rps: 200,
  durationS: 60,
  rampUpS: 10,
  holdS: 40,
  rampDownS: 10,
  steps: 1,
  confirm,
};

const snapshot = (running: boolean) => ({
  running,
  scenario: 'smoke.json',
  name: 'Smoke',
  targets: ['localhost'],
  elapsedS: 12,
  durationS: 60,
  users: 20,
  targetUsers: 21,
  maxUsers: 50,
  rpsLimit: 200,
  totals: { requests: 480, errors: 3, errorRate: 0.00625, rps: 40, latency: { p50: 12, p95: 48, p99: 90, min: 2, max: 120, mean: 15 } },
  errors: { http5xx: 2, timeout: 1 },
  series: [
    { t: 11, requests: 40, errors: 1, users: 19, p50: 12, p95: 40, p99: 80 },
    { t: 12, requests: 44, errors: 0, users: 20, p50: 13, p95: 48, p99: 90 },
  ],
  end: running ? null : 'stopped',
  abort: null,
  reportId: running ? null : '1700000000-smoke',
});

let posted: Posted[] = [];
let unmount: () => void = () => undefined;
const answered = new Set<number>();

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

const calls = (op: string): Posted[] => posted.filter((m) => m.type === 'call' && m.tool === 'panel' && m.args?.op?.op === op);

/** Answers the calls of op `op` not answered yet. */
async function reply(op: string, value: unknown, failure?: { error: string; params: Record<string, string> }): Promise<void> {
  for (const message of calls(op).filter((m) => !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(
      failure
        ? { type: 'result', id: message.id, ok: false, error: failure.error, params: failure.params }
        : { type: 'result', id: message.id, ok: true, value },
    );
  }
}

async function openSmoke(): Promise<void> {
  $('[data-scenario="smoke.json"]')?.click();
  await settle();
  await reply('load', { id: 'smoke.json', scenario: SCENARIO });
}

async function review(): Promise<void> {
  $('#review')?.click();
  await settle();
  await reply('save', { id: 'smoke.json' });
  await reply('state', STATE);
  await reply('preview', SUMMARY);
}

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.load-test.', name: 'Load test panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
  await reply('state', STATE);
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the load test panel', () => {
  it('says to test only own systems and opens a scenario', async () => {
    expect($('#own-systems')?.textContent).toContain('Only test systems you own or may test.');
    await openSmoke();
    expect(calls('load')[0]?.args?.op?.id).toBe('smoke.json');
    expect($('#scenario-editor')).not.toBeNull();
  });

  it('starts only after the summary, a foreign host only after the person confirmed it', async () => {
    await openSmoke();
    await review();
    expect(calls('save')).toHaveLength(1);
    expect(calls('preview')[0]?.args?.op?.id).toBe('smoke.json');
    expect($('#summary-users')?.textContent).toContain('50');
    expect($('[data-host="perf.example"]')?.getAttribute('data-standing')).toBe('unconfirmed');
    const startButton = $('#start') as HTMLButtonElement | null;
    expect(startButton?.disabled).toBe(true);
    const confirmButton = $('[data-unconfirmed="perf.example"] [data-confirm]') as HTMLButtonElement | null;
    expect(confirmButton?.disabled).toBe(true);
    $('[data-unconfirmed="perf.example"] [role="checkbox"]')?.click();
    await settle();
    expect(calls('allow')).toHaveLength(0);
    ($('[data-unconfirmed="perf.example"] [data-confirm]') as HTMLButtonElement | null)?.click();
    await settle();
    expect(calls('allow')[0]?.args?.op).toEqual({ op: 'allow', host: 'perf.example', confirmed: true });
    await reply('allow', { allowed: 'perf.example', allowedHosts: ['perf.example'] });
    await reply('preview', { ...SUMMARY, targets: [SUMMARY.targets[0], { host: 'perf.example', standing: 'confirmed' }], unconfirmed: [] });
    expect(calls('start')).toHaveLength(0);
    ($('#start') as HTMLButtonElement | null)?.click();
    await settle();
    expect(calls('start')[0]?.args?.op).toEqual({ op: 'start', id: 'smoke.json', confirm });
    await reply('start', { started: true });
    expect($('#start-summary')).toBeNull();
  });

  it('shows the confirmation again when the core refuses an unconfirmed host', async () => {
    await openSmoke();
    $('#review')?.click();
    await settle();
    await reply('save', { id: 'smoke.json' });
    await reply('state', STATE);
    // As if the host's confirmation was taken back in the settings after this summary.
    await reply('preview', { ...SUMMARY, unconfirmed: [] });
    ($('#start') as HTMLButtonElement | null)?.click();
    await settle();
    await reply('start', null, { error: HOST_NOT_CONFIRMED, params: { host: 'perf.example' } });
    await reply('preview', SUMMARY);
    expect($('[data-unconfirmed="perf.example"]')).not.toBeNull();
    expect(($('#start') as HTMLButtonElement | null)?.disabled).toBe(true);
  });

  // RB-094-21: "raise it in Settings > Network > Load test" links there, and a refused start
  // links to the very setting it names.
  it('links the limits and a start over them to their settings', async () => {
    $('[data-scenario="smoke.json"]')?.click();
    await settle();
    await reply('load', { id: 'smoke.json', scenario: { ...SCENARIO, profile: { ...SCENARIO.profile, users: 500 } } });
    ($('#over-limits-setting [data-testid="setting-link-load-test"]') as HTMLElement).click();
    expect(posted).toContainEqual({ target: 'host', action: 'open_settings', key: 'load-test' });
    $('#review')?.click();
    await settle();
    await reply('save', { id: 'smoke.json' });
    await reply('state', STATE);
    await reply('preview', { ...SUMMARY, unconfirmed: [] });
    ($('#start') as HTMLButtonElement | null)?.click();
    await settle();
    await reply('start', null, { error: 'plugin.load-test.errors.over_limit', params: { setting: 'load-test.maxUsers' } });
    ($('#status [data-testid="setting-link-load-test.maxUsers"]') as HTMLElement).click();
    expect(posted).toContainEqual({ target: 'host', action: 'open_settings', key: 'load-test.maxUsers' });
  });

  it('shows the live numbers, stops on Stop and names the end', async () => {
    await fromHost({ type: 'event', message: { loadTest: snapshot(true) } });
    expect($('#live')?.getAttribute('data-running')).toBe('true');
    expect($('#live-users')?.textContent).toContain('20');
    expect($('#live-p95')?.textContent).toContain('48 ms');
    expect($('[data-kind="http5xx"]')).not.toBeNull();
    expect($('#load-chart polyline[data-line="rate"]')?.getAttribute('points')).toBeTruthy();
    $('#stop')?.click();
    await settle();
    expect(calls('stop')).toHaveLength(1);
    await reply('stop', { stopping: true });
    await fromHost({ type: 'event', message: { loadTest: snapshot(false) } });
    expect($('#live')?.getAttribute('data-running')).toBe('false');
    expect($('#stop')).toBeNull();
    expect($('#live-report')?.textContent).toContain('1700000000-smoke');
    expect(calls('state').length).toBeGreaterThan(1);
  });
});

describe('without a saved request (RB-096-09)', () => {
  async function emptyEditor(): Promise<void> {
    $('#refresh')?.click();
    await settle();
    await reply('state', { ...STATE, requests: [] });
    $('#new')?.click();
    await settle();
  }

  it('says where requests are made and opens the API client', async () => {
    await emptyEditor();
    expect($('#no-requests')?.getAttribute('role')).toBe('status');
    expect($('#no-requests')?.textContent).toContain('panel.editor.noRequests');
    $('#open-api-client')?.click();
    await settle();
    expect(calls('openApiClient')[0]?.args?.op).toEqual({ op: 'openApiClient' });
  });

  it('reads the saved requests again when the panel gets the focus back', async () => {
    await emptyEditor();
    const before = calls('state').length;
    window.dispatchEvent(new FocusEvent('focus'));
    await settle();
    expect(calls('state').length).toBe(before + 1);
    await reply('state', STATE);
    expect($('#no-requests')).toBeNull();
  });
});
