// The Breakpoints, Compose and Sessions tabs (RB-080-37, on Nuxt UI since RB-080-84): the bridge
// messages they send and how they render the plugin's `debug`, `sessions_list` and
// `session_open` answers.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { $, answer, openTab, posted, settle, shownTab, start, stop, type } from './harness';

const P = 'plugin.network-panel.';
const STRINGS: Record<string, string> = {
  [P + 'breakpoints.enabled']: 'Breakpoints on',
  [P + 'breakpoints.disabled']: 'Breakpoints off',
  [P + 'breakpoints.heldCount']: '{count} held',
  [P + 'breakpoints.heldIn']: '{label} ({stage})',
  [P + 'breakpoints.stageRequest']: 'Request',
  [P + 'breakpoints.stageResponse']: 'Response',
  [P + 'breakpoints.allViews']: 'all',
  [P + 'breakpoints.timeout']: 'Held requests go on after {seconds} s.',
  [P + 'breakpoints.badPattern']: 'Enter a pattern.',
  [P + 'breakpoints.queued']: 'Sent to the app.',
  [P + 'breakpoints.masked']: 'Cookies show as ***.',
  [P + 'compose.result']: '{method} {url}: {status} in {time} ms',
  [P + 'compose.badUrl']: 'Enter an http(s) URL.',
  [P + 'compose.noView']: 'no view',
  [P + 'sessions.viewing']: 'Showing session {path}.',
  [P + 'sessions.saved']: 'Saved: {path}',
};

const SEEN = { fromCache: false, time: 1, count: 1, durationMs: 12 };
const LIST = {
  total: 1, captured: true, trusted: true, rulesRunning: true, applied: 0, mocksEnabled: true, cacheDisabled: false,
  views: [
    { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', width: 390, supported: true, intercepting: true },
    { id: 2, label: 'Desktop', breakpoint: 'desktop', engine: 'chromium', width: 1440, supported: true, intercepting: true },
  ],
  rows: [
    { method: 'GET', url: 'http://x.test/api/user', type: 'Fetch', partial: false, differs: false,
      views: { '1': { ...SEEN, id: '1.3', status: 200, size: 30 }, '2': { ...SEEN, id: '2.3', status: 200, size: 30 } } },
  ],
};
const MOCKS = { trusted: true, mocksEnabled: true, cacheDisabled: false, dir: '.rbrowser/mocks', files: [], block: [] };
const DEBUG = {
  held: [
    { id: 'h1', key: 'request POST http://x.test/api/user', view: 1, stage: 'request', method: 'POST', url: 'http://x.test/api/user',
      headers: [['cookie', '***'], ['content-type', 'application/json']], body: '{"a":1}', bodyBase64: false, bodyTruncated: false,
      resourceType: 'Fetch', breakpoint: 'bp-1', since: 1 },
    { id: 'h2', key: 'response GET http://x.test/page', view: 2, stage: 'response', method: 'GET', url: 'http://x.test/page',
      headers: [], bodyTruncated: false, resourceType: 'Document', breakpoint: 'bp-2', since: 2,
      response: { status: 200, headers: [['content-type', 'text/html']] } },
  ],
  breakpoints: [{ id: 'bp-1', urlPattern: '*/api/*', regex: false, method: 'POST', stage: 'request', views: [1], enabled: true }],
  breakpointsEnabled: true, breakpointsActive: 1,
  composed: [{ method: 'GET', url: 'http://x.test/api', view: 1, status: 201, headers: [], body: 'ok', bodyBase64: false, bodyTruncated: false, timeMs: 7, at: 1 }],
  answers: {}, pending: 0, lastError: null, skipped: {}, timeoutMs: 30000, maxBodyBytes: 4194304, rulesRunning: true,
};

const held = (at: number): HTMLElement => document.querySelectorAll<HTMLElement>('#held > li')[at] as HTMLElement;
/** The text field of a code editor and the backdrop that colours it. */
const editor = (id: string) => {
  const area = $<HTMLTextAreaElement>(id);
  return { area, wrap: area.closest('[data-highlight]') as HTMLElement };
};

beforeEach(async () => {
  await start(STRINGS);
  await answer('list', LIST);
  await answer('mocks', MOCKS);
});

afterEach(stop);

describe('the breakpoints tab', () => {
  it('opens on its tab, asks for the debug state and renders breakpoints and held requests', async () => {
    await openTab('breakpoints');
    expect(shownTab('#panel-breakpoints')).toBe(true);
    expect(shownTab('#panel-requests')).toBe(false);
    await answer('debug', DEBUG);
    expect($('#breakpoints-toggle').textContent).toBe('Breakpoints on');
    expect($('#breakpoints-toggle').getAttribute('aria-pressed')).toBe('true');
    expect($('#held-count').textContent).toBe('2 held');
    expect($('#breakpoints-timeout').textContent?.trim()).toBe('Held requests go on after 30 s.');
    const breakpoint = $('#breakpoints > li');
    expect(breakpoint.querySelector('[data-pattern]')?.textContent).toBe('*/api/*');
    expect(breakpoint.querySelector('[data-views]')?.textContent).toBe('Mobile');
    expect([0, 1].map((at) => held(at).querySelector('[data-where]')?.textContent)).toEqual(['Mobile (Request)', 'Desktop (Response)']);
    $('#breakpoints [data-enable="bp-1"]').click();
    await settle();
    expect([...posted].reverse().find((m) => m.tool === 'breakpoint_enable')?.args).toEqual({ id: 'bp-1', enabled: false });
  });

  it('adds a breakpoint from the form and refuses an empty pattern', async () => {
    await openTab('breakpoints');
    await answer('debug', DEBUG);
    $('#bp-add').click();
    await settle();
    expect($('#debug-note').textContent).toBe('Enter a pattern.');
    await type('#bp-pattern', '*/config.json');
    // RB-090-48: the tab row scrolls sideways only, never a vertical scroll bar beside the tabs.
    const row = [...(document.querySelector('#bp-stage [role="tablist"]')?.classList ?? [])];
    expect(row).toContain('overflow-y-hidden');
    expect(row.filter((name) => /^overflow-(y-)?(auto|scroll)$/.test(name))).toEqual([]);
    const response = $('#bp-stage [role="tab"]:last-child');
    response.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    response.click();
    $('#bp-views [role="checkbox"][data-view="2"]').click();
    $('#bp-views [role="checkbox"][data-view="1"]').click();
    await settle();
    $('#bp-add').click();
    await settle();
    expect(posted.find((m) => m.tool === 'breakpoint_add')?.args).toEqual({ urlPattern: '*/config.json', stage: 'response', views: [1, 2] });
  });

  it('edits a held request and continues it with the changes, or for all views', async () => {
    await openTab('breakpoints');
    await answer('debug', DEBUG);
    held(0).click();
    await settle();
    expect($('#held-editor')).not.toBeNull();
    expect($<HTMLTextAreaElement>('#held-headers').value).toBe('cookie: ***\ncontent-type: application/json');
    expect($('#held-body-note').textContent).toBe('Cookies show as ***.');
    await type('#held-body', '{"a":2}');
    await type('#held-headers', 'cookie: ***\ncontent-type: text/plain');
    $('#held-continue').click();
    await settle();
    expect(posted.find((m) => m.tool === 'continue')?.args).toEqual({
      id: 'h1', allViews: false, edits: { headers: [['cookie', '***'], ['content-type', 'text/plain']], body: '{"a":2}' },
    });
    await answer('continue', { ask: 'a1', queued: true });
    expect($('#debug-note').textContent).toBe('Sent to the app.');
    await answer('debug', DEBUG);
    held(1).click();
    await settle();
    expect($('#held-status-row')).not.toBeNull();
    await type('#held-status', '500');
    $('#held-continue-all').click();
    await settle();
    const all = [...posted].reverse().find((m) => m.tool === 'continue');
    expect(all?.args).toEqual({ id: 'h2', allViews: true, edits: { status: 500 } });
    held(0).click();
    await settle();
    $('#held-abort-all').click();
    await settle();
    expect([...posted].reverse().find((m) => m.tool === 'abort')?.args).toEqual({ id: 'h1', allViews: true });
  });

  it('opens a held request with Enter', async () => {
    await openTab('breakpoints');
    await answer('debug', DEBUG);
    held(1).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(held(1).getAttribute('aria-selected')).toBe('true');
    expect($('#held-status-row')).not.toBeNull();
  });
});

describe('the held editor', () => {
  it('keeps the edits in progress across the polls', async () => {
    await openTab('breakpoints');
    await answer('debug', DEBUG);
    held(0).click();
    await settle();
    await type('#held-body', '{"a":2}');
    vi.advanceTimersByTime(2500);
    await answer('debug', DEBUG);
    expect($<HTMLTextAreaElement>('#held-body').value).toBe('{"a":2}');
    held(1).click();
    await settle();
    expect($<HTMLInputElement>('#held-status').value).toBe('200');
    held(0).click();
    await settle();
    // Another selection fills the editor anew.
    expect($<HTMLTextAreaElement>('#held-body').value).toBe('{"a":1}');
  });
});

describe('the compose tab', () => {
  it('sends a composed request from the chosen view and lists the results', async () => {
    await openTab('compose');
    await answer('debug', DEBUG);
    expect($('#composed > li [data-line]').textContent?.trim()).toBe('GET http://x.test/api: 201 in 7 ms');
    expect($('#compose-view').textContent).toBe('Mobile');
    $('#compose-send').click();
    await settle();
    expect($('#compose-note').textContent).toBe('Enter an http(s) URL.');
    await type('#compose-url', 'http://x.test/api/user');
    await type('#compose-method', 'post');
    await type('#compose-headers', 'content-type: application/json');
    await type('#compose-body', '{}');
    await type('#compose-times', '3');
    $('#compose-send').click();
    await settle();
    expect(posted.find((m) => m.tool === 'compose')?.args).toEqual({
      request: { method: 'POST', url: 'http://x.test/api/user', headers: [['content-type', 'application/json']], body: '{}' },
      view: 1, times: 3,
    });
  });

  it('repeats a request of the list from its first view', async () => {
    ($('#rows [data-row]').closest('tr') as HTMLElement).click();
    await settle();
    $('#repeat-ten').click();
    await settle();
    expect(posted.find((m) => m.tool === 'repeat')?.args).toEqual({ view: 1, requestId: '1.3', times: 10 });
    expect(shownTab('#panel-compose')).toBe(true);
  });
});

describe('the sessions tab', () => {
  it('saves a session, opens one for display and returns to the live list', async () => {
    await openTab('sessions');
    await answer('sessions_list', { sessions: [{ name: 'a.har', path: '.rbrowser/out/network-panel/sessions/a.har' }] });
    expect($('#sessions > li [data-name]').textContent).toBe('a.har');
    $('#session-save').click();
    await answer('session_save', { path: '.rbrowser/out/network-panel/sessions/b.har', entries: 2 });
    expect($('#sessions-note').textContent).toBe('Saved: .rbrowser/out/network-panel/sessions/b.har');
    $('#sessions [data-open]').click();
    await answer('session_open', {
      path: '.rbrowser/out/network-panel/sessions/a.har', total: 1, readOnly: true,
      views: [{ id: 1, label: 'Mobile' }],
      rows: [{ method: 'GET', url: 'http://x.test/old', type: 'Fetch', partial: false, differs: false, views: { '1': { ...SEEN, id: 'x', status: 404 } } }],
    });
    expect(shownTab('#panel-requests')).toBe(true);
    expect($('#rows [data-row]').textContent?.trim()).toBe('old');
    expect($('#status').textContent).toBe('Showing session .rbrowser/out/network-panel/sessions/a.har.');
    expect($('#session-close')).not.toBeNull();
    const before = posted.filter((m) => m.tool === 'list').length;
    vi.advanceTimersByTime(2500);
    await settle();
    expect(posted.filter((m) => m.tool === 'list').length).toBe(before);
    $('#session-close').click();
    await settle();
    expect($('#rows [data-row]').textContent?.trim()).toBe('user');
    expect($('#session-close')).toBeNull();
  });
});

// RB-080-65: headers and bodies in syntax colours, in the editors as in the results.
describe('the syntax colours', () => {
  it('colour the held request: headers as HTTP, the body in the syntax its content type names', async () => {
    await openTab('breakpoints');
    await answer('debug', DEBUG);
    held(0).click();
    await settle();
    const headers = editor('#held-headers');
    const body = editor('#held-body');
    expect(headers.wrap.dataset.highlight).toBe('on');
    expect(headers.wrap.querySelector('.hl-key')?.textContent).toBe('cookie');
    expect(body.wrap.querySelector('.hl-key')?.textContent).toBe('"a"');
    // Typing colours again; content that is no code, of a type that names none, stays one colour.
    await type('#held-headers', 'content-type: text/plain');
    await type('#held-body', 'plain words');
    expect(editor('#held-body').wrap.dataset.highlight).toBe('plain');
  });

  // Review of RB-080-65: the body follows a content type typed into the headers.
  it('colour the body again when its headers name another content type', async () => {
    await openTab('compose');
    await type('#compose-body', 'a { color: red }');
    expect(editor('#compose-body').wrap.dataset.highlight).toBe('plain');
    await type('#compose-headers', 'content-type: text/css');
    expect(editor('#compose-body').wrap.dataset.highlight).toBe('on');
    expect(editor('#compose-body').wrap.querySelector('.hl-key')?.textContent).toBe('color');
  });

  it('colour a composed result: its response headers and its body', async () => {
    await openTab('compose');
    const composed = { ...DEBUG.composed[0], headers: [['content-type', 'application/json']], body: '{"id": 7}' };
    await answer('debug', { ...DEBUG, composed: [composed] });
    const item = $('#composed > li');
    expect(item.querySelector('[data-headers] .hl-key, pre .hl-key')?.textContent).toBe('content-type');
    const pres = item.querySelectorAll('pre');
    expect(pres[1]?.querySelector('.hl-key')?.textContent).toBe('"id"');
    expect(pres[1]?.textContent).toBe('{"id": 7}');
  });
});
