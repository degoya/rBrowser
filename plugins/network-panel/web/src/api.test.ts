// The way into the API client (RB-096-08): Open in API client and Save as request on the selected
// request, Save shown as requests over the filtered list, the commands' posts and the endpoint's
// button - the bridge messages they send and what the status line says.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $, answer, host, openTab, posted, settle, start, stop } from './harness';

const P = 'plugin.network-panel.';
const STRINGS: Record<string, string> = {
  [P + 'panels.network']: 'Network',
  [P + 'api.open']: 'Open in API client',
  [P + 'api.save']: 'Save as request',
  [P + 'api.saveShown']: 'Save shown as requests',
  [P + 'api.opened']: 'Opened {name} in the API client, not saved yet.',
  [P + 'api.saved']: '{saved, plural, =0 {No request saved} one {# request saved} other {# requests saved}}; {skipped} skipped.',
  [P + 'api.missing']: 'No value yet: {names}.',
  [P + 'api.noSelection']: 'Select a request first.',
  [P + 'api.errors.untrusted']: 'Trust this workspace first.',
};

const SEEN = { fromCache: false, count: 1, durationMs: 12, status: 200, size: 30 };
const LIST = {
  total: 2, captured: true, trusted: true, rulesRunning: true, applied: 0, mocksEnabled: true, cacheDisabled: false,
  views: [
    { id: 1, label: 'Mobile', supported: true, intercepting: true },
    { id: 2, label: 'Desktop', supported: true, intercepting: true },
  ],
  rows: [
    { method: 'GET', url: 'http://x.test/api/user', type: 'Fetch', partial: false, differs: false,
      views: { '1': { ...SEEN, id: '1.3', time: 1 }, '2': { ...SEEN, id: '2.3', time: 2 } } },
    { method: 'POST', url: 'http://x.test/api/cart', type: 'XHR', partial: true, differs: false,
      views: { '1': { ...SEEN, id: '1.5', time: 3 } } },
  ],
};

const names = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#rows [data-row]')];
const last = (tool: string) => [...posted].reverse().find((m) => m.tool === tool);

beforeEach(async () => {
  await start(STRINGS);
  await answer('list', LIST);
});

afterEach(stop);

describe('into the API client', () => {
  it('opens the selected request in the API client and says so', async () => {
    (names()[0]?.closest('tr') as HTMLElement).click();
    await settle();
    $('#api-open').click();
    await settle();
    expect(last('api_open')?.args).toEqual({ view: 2, id: '2.3' });
    await answer('api_open', { request: { name: 'GET /api/user' }, folder: 'x-test', variables: [] });
    expect(document.body.textContent).toContain('Opened GET /api/user in the API client, not saved yet.');
  });

  it('saves the selected request and names a variable still without a value', async () => {
    (names()[1]?.closest('tr') as HTMLElement).click();
    await settle();
    $('#api-save').click();
    await settle();
    expect(last('api_save')?.args).toEqual({ requests: [{ view: 1, id: '1.5' }] });
    await answer('api_save', { saved: [{ id: 'x-test/post-api-cart.http', name: 'POST /api/cart' }], skipped: [], variables: [{ name: 'x-test-cookie', value: 'missing' }] });
    expect(document.body.textContent).toContain('1 request saved; 0 skipped. No value yet: x-test-cookie.');
  });

  it('saves every shown request at once', async () => {
    $('#api-save-shown').click();
    await settle();
    expect(last('api_save')?.args).toEqual({ requests: [{ view: 2, id: '2.3' }, { view: 1, id: '1.5' }] });
    await answer('api_save', { saved: [], skipped: [{ name: 'GET /api/user', reason: 'exists' }, { name: 'POST /api/cart', reason: 'duplicate' }], variables: [] });
    expect(document.body.textContent).toContain('No request saved; 2 skipped.');
  });

  it('does what the commands post: open the selected one, save the shown ones', async () => {
    await host({ type: 'event', message: { apiOpen: true } });
    expect(last('api_open')).toBeUndefined();
    expect(document.body.textContent).toContain('Select a request first.');
    await host({ type: 'event', message: { apiSave: true } });
    expect(last('api_save')?.args).toEqual({ requests: [{ view: 2, id: '2.3' }, { view: 1, id: '1.5' }] });
  });

  it('says the API client failure in its own words', async () => {
    $('#api-save-shown').click();
    await settle();
    await answer('api_save', 'plugin.api-client.errors.untrusted', false);
    expect(document.body.textContent).toContain('Trust this workspace first.');
  });

  it('opens an endpoint newest request in the API client', async () => {
    await openTab('endpoints');
    const KEY = 'GET http://x.test/api/user';
    await answer('endpoints', {
      total: 1, captured: true, sort: 'p95', views: [{ id: 1, label: 'Mobile' }], findings: [],
      endpoints: [{
        key: KEY, method: 'GET', origin: 'http://x.test', path: '/api/user', calls: 1, failed: 0, fromCache: 0,
        statuses: { '200': 1 }, encodings: {}, cacheControl: [], validators: 0, views: [{ view: 1, calls: 1 }],
        requests: [{ view: 1, id: '1.3', url: 'http://x.test/api/user' }], issues: [],
        apiClient: { method: 'GET', url: 'http://x.test/api/user', view: 1, id: '1.3' },
      }],
    });
    $(`#endpoints [data-endpoint="${KEY}"]`).click();
    await settle();
    $('#endpoint-api-open').click();
    await settle();
    expect(last('api_open')?.args).toEqual({ view: 1, id: '1.3' });
  });
});
