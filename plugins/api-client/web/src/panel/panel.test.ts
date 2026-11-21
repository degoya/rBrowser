// The API client panel in its frame (RB-096-01): mounted as the panel frame mounts it
// (`harness.ts`).
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $, calls, chooseTab, posted, fromHost, reply, SAVED, settle, start, STATE, stop, type, valueOf } from './harness';
import { HOST_NOT_ALLOWED } from './model';

const STRINGS = {
  'plugin.api-client.panels.api-client': 'API client',
  'plugin.api-client.panel.allow.title': 'Send requests to {host} from this project?',
  'plugin.api-client.errors.host_not_allowed': 'This project does not send to {host} yet.',
  'plugins.panel.call_failed': 'The plugin could not do that.',
  'plugin.api-client.panel.import.summary': '{requests} to import',
  'plugin.api-client.panel.import.done': 'Imported {count} into {folder}.',
  'plugin.api-client.panel.import.notes.script': '{at}: the {detail} script does not run.',
  'plugin.api-client.panel.import.secretItem': '{at}: {name} - set {variable} (Settings > Network > API client)',
  'plugin.api-client.panel.openSetting': 'Open the setting',
  'plugin.api-client.panel.capture.opened': 'Opened {name} from the network panel; not saved yet.',
  'plugin.api-client.panel.capture.missing': 'No value yet for {names}.',
  'plugin.api-client.panel.capture.replace.title': 'Replace your unsaved changes with {name}?',
};

const IMPORTED = {
  preview: true,
  formats: ['curl'],
  collection: null,
  folder: 'import',
  requests: [{ folder: 'import', name: 'GET /ping', method: 'GET', url: 'https://a.example/ping' }],
  environments: [],
  environmentsWritten: false,
  kept: [],
  failed: [],
  report: {
    imported: 1,
    environments: 0,
    notes: [{ kind: 'reported', code: 'script', at: 'GET /ping', detail: 'pre-request' }],
    secrets: [{ at: 'GET /ping', name: 'token', variable: 'RB_SECRET_TOKEN' }],
  },
};

const ANSWER = {
  method: 'GET',
  url: 'https://staging.example.com/me',
  status: 200,
  statusText: 'OK',
  headers: [['content-type', 'application/json']],
  contentType: 'application/json',
  body: '{"name":"Ada","token":"***"}',
  bodyEncoding: 'text',
  size: 28,
  truncated: false,
  timings: { dns: 1.2, connect: 3, tls: 10, firstByte: 20, total: 40 },
  redirects: [],
  environment: 'staging',
};

beforeEach(async () => {
  await start(STRINGS);
});

afterEach(stop);

describe('the API client panel', () => {
  it('lists the collections and opens a request', async () => {
    expect($('[data-id="users/me.http"]')?.textContent).toContain('Me');
    $('[data-id="users/me.http"]')?.click();
    await settle();
    expect(calls('load')).toHaveLength(1);
    await reply('load', { id: 'users/me.http', request: SAVED('Me', '{{baseUrl}}/me', 'POST') });
    expect(valueOf('#url')).toBe('{{baseUrl}}/me');
  });

  it('asks before a new host and sends only after the person allows it', async () => {
    $('#send')?.click();
    await settle();
    expect(calls('send')).toHaveLength(1);
    expect(calls('send')[0]?.args?.op?.environment).toBe('staging');
    await reply('send', null, { error: HOST_NOT_ALLOWED, params: { host: 'staging.example.com' } });
    expect($('#allow-question')?.textContent).toContain('staging.example.com');
    expect(calls('allow')).toHaveLength(0);
    $('#allow')?.click();
    await settle();
    expect(calls('allow')[0]?.args?.op?.host).toBe('staging.example.com');
    await reply('allow', { allowed: 'staging.example.com', allowedHosts: ['staging.example.com'] });
    expect(calls('send')).toHaveLength(2);
    await reply('send', ANSWER);
    expect($('#status-code')?.textContent).toContain('200');
    expect($('#tree-rows')?.textContent).toContain('Ada');
  });

  it('sends when the command says so', async () => {
    await fromHost({ type: 'event', message: { send: true } });
    expect(calls('send')).toHaveLength(1);
  });

  it('imports a pasted command after a preview and shows its report', async () => {
    await chooseTab('side-tabs', 2);
    await type('#import-paste', 'curl https://a.example/ping');
    $('#import-preview')?.click();
    await settle();
    await settle();
    expect(calls('import')).toHaveLength(1);
    expect(calls('import')[0]?.args?.op).toMatchObject({
      preview: true,
      environments: true,
      format: null,
      folder: null,
      files: [{ name: '', text: 'curl https://a.example/ping' }],
    });
    await reply('import', IMPORTED);
    expect($('#import-summary')?.textContent).toContain('1 to import');
    expect($('#import-reported')?.textContent).toContain('the pre-request script does not run');
    expect($('#import-secrets')?.textContent).toContain('RB_SECRET_TOKEN');
    // RB-094-21: the report's text names Settings > Network > API client and links there.
    $('#import-secrets [data-setting]')?.click();
    expect(posted.at(-1)).toEqual({ target: 'host', action: 'open_settings', key: 'api-client.secrets' });
    $('#import-run')?.click();
    await settle();
    await settle();
    expect(calls('import')).toHaveLength(2);
    expect(calls('import')[1]?.args?.op?.preview).toBe(false);
    await reply('import', { ...IMPORTED, preview: false, requests: [{ ...IMPORTED.requests[0], id: 'import/get-ping.http' }] });
    expect(calls('state')).toHaveLength(2);
    expect($('#import-summary')?.textContent).toContain('Imported 1 into import.');
  });
});

describe('checks, collection runs and the benchmark (RB-096-04)', () => {
  const REPORT = {
    at: 1,
    environment: 'staging',
    tally: { requests: 2, passed: 1, failed: 1, errors: 0, checks: 2, checksFailed: 1 },
    requests: [
      { id: 'users/me.http', name: 'Me', method: 'GET', url: '{{baseUrl}}/me', status: 200, passed: true, checks: [{ check: 'status 2xx', passed: true, actual: '200' }] },
      { id: 'users/list.http', name: 'List', method: 'GET', url: '{{baseUrl}}/users', status: 200, passed: false, checks: [{ check: 'json $[0].name == Ada', passed: false, actual: 'Bob' }] },
    ],
    cut: false,
  };

  it('runs the collections on the command and lists what failed', async () => {
    await fromHost({ type: 'event', message: { runCollection: true } });
    expect(calls('runCollection')).toHaveLength(1);
    expect(calls('runCollection')[0]?.args?.op?.environment).toBe('staging');
    await reply('runCollection', REPORT);
    expect($('#run-report')).not.toBeNull();
    expect($('[data-run-id="users/list.http"]')?.textContent).toContain('json $[0].name == Ada');
    expect($('[data-run-id="users/me.http"]')).toBeNull();
  });

  it('shows the verdicts of a send and adds checks to the request', async () => {
    $('#send')?.click();
    await settle();
    await reply('send', { ...ANSWER, checks: [{ check: 'status 2xx', passed: true, actual: '200' }, { check: 'time < 10', passed: false, actual: '40 ms' }] });
    const verdicts = document.querySelectorAll('#verdicts [data-verdict]');
    expect(verdicts).toHaveLength(2);
    expect(verdicts[1]?.getAttribute('data-passed')).toBe('false');
  });

  it('benchmarks the open saved request', async () => {
    $('[data-id="users/me.http"]')?.click();
    await settle();
    await reply('load', { id: 'users/me.http', request: SAVED('Me', '{{baseUrl}}/me') });
    $('#bench-run')?.click();
    await settle();
    expect(calls('bench')).toHaveLength(1);
    expect(calls('bench')[0]?.args?.op).toEqual({ op: 'bench', id: 'users/me.http', environments: ['staging'] });
    const summary = { count: 3, min: 10, median: 12, p95: 20, max: 20, mean: 14 };
    await reply('bench', {
      id: 'users/me.http',
      repeats: 3,
      concurrency: 1,
      series: [{ environment: 'staging', sent: 3, statuses: { '200': 3 }, errors: {}, phases: { dns: null, connect: summary, tls: null, firstByte: summary, total: summary } }],
    });
    expect(document.querySelectorAll('[data-series]')).toHaveLength(1);
    expect($('[data-phase="total"]')?.textContent).toContain('12 ms');
    expect($('[data-phase="tls"]')).toBeNull();
  });

  it('fills the editor with a request the network panel opened, unsaved', async () => {
    await fromHost({ type: 'event', message: { draft: true } });
    const draft = {
      request: { name: 'GET /v1/me', method: 'GET', url: 'https://a.test/v1/me', headers: [{ name: 'Authorization', value: 'Bearer {{a-test-token}}', enabled: true }],
        body: { kind: 'none' }, auth: { type: 'none' }, options: { timeoutMs: 30000, followRedirects: true, verifyTls: true } },
      folder: 'a-test',
      variables: [{ name: 'a-test-token', value: 'missing' }],
    };
    await reply('state', { ...STATE, draft });
    expect(valueOf('#url')).toBe('https://a.test/v1/me');
    expect(document.body.textContent).toContain('Opened GET /v1/me from the network panel');
    expect(document.body.textContent).toContain('a-test-token');
    // RB-094-21: capture.missing names Settings > Network > API client; the line links there.
    $('[data-testid="setting-link-api-client.secrets"]')?.click();
    expect(posted.at(-1)).toEqual({ target: 'host', action: 'open_settings', key: 'api-client.secrets' });
    expect(calls('save')).toHaveLength(0);
  });

  it('asks before a request the network panel opened replaces unsaved edits', async () => {
    const url = (): string | undefined => valueOf('#url');
    await type('#url', 'https://mine.test/draft');
    const draft = {
      request: { name: 'GET /v1/me', method: 'GET', url: 'https://a.test/v1/me', headers: [], body: { kind: 'none' }, auth: { type: 'none' },
        options: { timeoutMs: 30000, followRedirects: true, verifyTls: true } },
      folder: 'a-test',
      variables: [],
    };
    await fromHost({ type: 'event', message: { draft: true } });
    await reply('state', { ...STATE, draft });
    expect($('#draft-question')?.textContent).toContain('Replace your unsaved changes with GET /v1/me?');
    expect(url()).toBe('https://mine.test/draft');
    $('#draft-keep')?.click();
    await settle();
    expect($('#draft-question')).toBeNull();
    expect(url()).toBe('https://mine.test/draft');
    await fromHost({ type: 'event', message: { draft: true } });
    await reply('state', { ...STATE, draft });
    $('#draft-replace')?.click();
    await settle();
    expect($('#draft-question')).toBeNull();
    expect(url()).toBe('https://a.test/v1/me');
  });
});
