// The Requests tab and the panel's frame (RB-070-02, on Nuxt UI since RB-080-84): the bridge
// messages it sends and how it renders the plugin's answers.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { $, answer, host, openTab, posted, settle, shownTab, start, stop, type } from './harness';

const P = 'plugin.network-panel.';
const STRINGS: Record<string, string> = {
  [P + 'panels.network']: 'Network',
  [P + 'list.partial']: 'only some views',
  [P + 'list.differs']: 'differs',
  [P + 'list.viewSeen']: '{label}: {status}, {size}',
  [P + 'list.viewMissing']: '{label}: not loaded',
  [P + 'list.blocked']: 'blocked',
  [P + 'status.unsupported']: 'Not intercepted: {views}.',
  [P + 'status.untrusted']: 'Mocks need workspace trust.',
  [P + 'mocks.blocked']: 'blocked',
  [P + 'mocks.untrusted']: 'Trust the workspace to load the mocks.',
  [P + 'mocks.hits']: '{count, plural, one {# hit} other {# hits}}',
  [P + 'editor.badStatus']: 'Status 100 to 599.',
  [P + 'done.cleared']: 'Site data cleared.',
  [P + 'toolbar.mocksOn']: 'Mocks on',
  [P + 'errors.untrusted']: 'Trust the project first.',
  'plugins.error.busy': 'The plugin is busy.',
};

const SEEN = { fromCache: false, time: 1, count: 1, durationMs: 12 };
const LIST = {
  total: 3,
  captured: true,
  trusted: false,
  rulesRunning: true,
  applied: 1,
  mocksEnabled: true,
  cacheDisabled: false,
  views: [
    { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', width: 390, supported: true, intercepting: true },
    { id: 2, label: 'Desktop', breakpoint: 'desktop', engine: 'firefox', width: 1440, supported: false, intercepting: false },
  ],
  rows: [
    { method: 'GET', url: 'http://x.test/img/hero-480.png', type: 'Image', partial: true, differs: false,
      views: { '1': { ...SEEN, id: '1.2', status: 200, size: 4000 } } },
    { method: 'GET', url: 'http://x.test/api/user', type: 'Fetch', partial: false, differs: true,
      views: { '1': { ...SEEN, id: '1.3', status: 500, size: 30 }, '2': { ...SEEN, id: '2.3', status: 200, size: 30 } } },
    { method: 'GET', url: 'http://x.test/tracking.js', type: 'Script', partial: false, differs: false,
      views: { '1': { ...SEEN, id: '1.4', failed: 'net::ERR_BLOCKED_BY_CLIENT' }, '2': { ...SEEN, id: '2.4', failed: 'net::ERR_BLOCKED_BY_CLIENT' } } },
  ],
};
const MOCKS = {
  trusted: false, mocksEnabled: true, cacheDisabled: false, dir: '.rbrowser/mocks',
  files: [{ path: '.rbrowser/mocks/api.json', blocked: true, rules: [] }],
  block: [{ id: 'settings:block-0', urlPattern: '*/tracking.js', hits: 2 }],
};

/** The name buttons of the drawn rows. */
const names = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#rows [data-row]')];
const rowOf = (index: number): HTMLElement => names()[index]?.closest('tr') as HTMLElement;

beforeEach(async () => {
  await start(STRINGS);
  await answer('list', LIST);
  await answer('mocks', MOCKS);
});

afterEach(stop);

describe('the network panel', () => {
  it('asks for its settings, then for the list and the mocks', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(posted.find((m) => m.tool === 'list')?.args).toEqual({ limit: 150 });
    expect(posted.some((m) => m.tool === 'mocks')).toBe(true);
    expect($('#title').textContent).toBe('Network');
  });

  // Review of RB-080-84: a filter typed during a read was dropped; the older list stayed.
  it('never drops a filter typed while a read is on its way, and shows only the newest list', async () => {
    await host({ type: 'event', message: { refresh: true } });
    const polled = [...posted].reverse().find((m) => m.tool === 'list');
    await type('#search', 'user');
    await host({ type: 'result', id: polled?.id, ok: true, value: { ...LIST, rows: [] } });
    expect(names().length).toBe(3);
    const last = [...posted].reverse().find((m) => m.tool === 'list');
    expect(last?.id).not.toBe(polled?.id);
    expect(last?.args).toEqual({ limit: 150, search: 'user' });
  });

  it('marks the image only one view loaded and the request the views saw differently', () => {
    expect(names().map((name) => name.textContent?.trim())).toEqual(['hero-480.png', 'user', 'tracking.js']);
    expect([0, 1, 2].map((at) => [...rowOf(at).querySelectorAll('[data-mark]')].map((mark) => mark.textContent))).toEqual([['only some views'], ['differs'], []]);
    expect(rowOf(0).className).toContain('bg-warning/10');
    expect(rowOf(1).className).toContain('bg-info/10');
    const dots = [...rowOf(0).querySelectorAll('[data-dot]')].map((dot) => dot.getAttribute('aria-label'));
    expect(dots).toEqual(['Mobile: 200, 3.9 kB', 'Desktop: not loaded']);
    expect(rowOf(0).querySelector('[data-dot][data-engine="firefox"]')?.hasAttribute('data-missing')).toBe(true);
    expect(rowOf(2).querySelector('[data-status]')?.textContent).toBe('blocked');
  });

  it('names the views it cannot intercept and the missing trust, each once', async () => {
    const notes = $('#notes').textContent ?? '';
    expect(notes).toContain('Not intercepted: Desktop.');
    expect(notes).toContain('Mocks need workspace trust.');
    await openTab('mocks');
    expect($('#mocks [data-blocked]').textContent).toBe('blocked');
    expect($('#mocks-note').textContent).toBe('Trust the workspace to load the mocks.');
    expect($('#mocks [data-block] [data-hits]').textContent).toBe('2 hits');
  });

  it('says the session switches in words and as pressed', () => {
    const mocks = $('#mocks-toggle');
    expect(mocks.textContent).toBe('Mocks on');
    expect(mocks.getAttribute('aria-pressed')).toBe('true');
    expect($('#cache-toggle').getAttribute('aria-pressed')).toBe('false');
  });

  it('moves through the rows with the keyboard and opens a request with Enter', async () => {
    expect(names().map((name) => name.tabIndex)).toEqual([0, -1, -1]);
    names()[0]?.focus();
    names()[0]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(names()[1]);
    expect(names().map((name) => name.tabIndex)).toEqual([-1, 0, -1]);
    names()[1]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(names()[2]);
    names()[2]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
    await settle();
    names()[1]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect($('#detail')).not.toBeNull();
    expect($('#detail-title').textContent).toBe('GET user');
    expect(names()[1]?.getAttribute('aria-current')).toBe('true');
    expect(document.querySelectorAll('#detail-views tbody tr')).toHaveLength(2);
  });

  it('draws only the rows in sight of a long list', async () => {
    vi.advanceTimersByTime(2000);
    const rows = Array.from({ length: 500 }, (_, at) => ({ ...LIST.rows[0], url: `http://x.test/img/${at}.png`, partial: false }));
    await answer('list', { ...LIST, total: 500, rows });
    expect(names().length).toBeGreaterThan(0);
    expect(names().length).toBeLessThan(100);
  });

  it('makes a mock from a request and refuses a bad status', async () => {
    rowOf(1).click();
    await settle();
    $('#mock-this').click();
    await settle();
    expect(shownTab('#panel-mocks')).toBe(true);
    expect($<HTMLInputElement>('#pattern').value).toBe('http://x.test/api/user');
    await type('#code', '42');
    $('#save').click();
    await settle();
    expect($('#editor-error').textContent).toBe('Status 100 to 599.');
    await type('#code', '500');
    $('#save').click();
    await settle();
    expect(posted.find((m) => m.tool === 'mock_add')?.args).toEqual({
      match: { urlPattern: 'http://x.test/api/user', method: 'GET' }, mock: { status: 500, delayMs: 0 },
    });
  });

  it('clears site data only after the confirmation', async () => {
    $('#clear-site').click();
    await settle();
    expect($('#confirm')).not.toBeNull();
    expect($('#confirm').getAttribute('role')).toBe('alertdialog');
    expect(posted.some((m) => m.command === 'site_data_clear')).toBe(false);
    $('#confirm-yes').click();
    await settle();
    expect(posted.find((m) => m.command === 'site_data_clear')?.args).toEqual({ confirmed: true });
    expect($('#confirm')).toBeNull();
    await answer('site_data_clear', { cleared: true });
    expect($('#status').textContent).toBe('Site data cleared.');
    expect($('#status').dataset.tone).toBe('ok');
  });

  it('asks for the confirmation when the shortcut ran the command, and Cancel asks nothing', async () => {
    await host({ type: 'event', message: { confirmClear: true } });
    expect($('#confirm')).not.toBeNull();
    $('#confirm-no').click();
    await settle();
    expect($('#confirm')).toBeNull();
    expect(posted.some((m) => m.command === 'site_data_clear')).toBe(false);
  });

  it('says a failed call in a sentence, never the code', async () => {
    vi.advanceTimersByTime(2000);
    await answer('list', 'plugins.error.busy', false);
    expect($('#status').textContent).toBe('The plugin is busy.');
    expect($('#status').dataset.tone).toBe('error');
    $('#mocks-toggle').click();
    await answer('mocks_toggle', 'plugin.network-panel.errors.untrusted', false);
    expect($('#status').textContent).toBe('Trust the project first.');
  });

  // RB-080-65: a body in the syntax of its MIME type; page content stays text.
  it('shows a response body in the syntax colours of its MIME type, never as markup', async () => {
    vi.advanceTimersByTime(2000);
    const json = { ...SEEN, id: '1.5', status: 200, size: 40, mimeType: 'application/json' };
    await answer('list', { ...LIST, rows: [{ method: 'GET', url: 'http://x.test/api/me', type: 'Fetch', partial: false, differs: false, views: { '1': json } }] });
    rowOf(0).click();
    await settle();
    $('#detail-views [data-show-body]').click();
    expect(posted.find((m) => m.tool === 'body')?.args).toEqual({ view: 1, id: '1.5' });
    const text = '{"name": "</pre><img src=x onerror=alert(1)>"}';
    await answer('body', { text, base64: false, truncated: true });
    const body = $('#body pre');
    expect(body.querySelector('.hl-key')?.textContent).toBe('"name"');
    expect(body.querySelector('img')).toBeNull();
    expect(body.textContent).toBe(text);
    expect($('#body-cut').textContent).toBe('detail.cut');
    // Switched off while it shows: one colour at once.
    await host({ type: 'settings', strings: STRINGS, settings: { 'ui.syntaxHighlight': false } });
    expect($('#body pre').querySelector('.hl-key')).toBeNull();
    expect($('#body pre').textContent).toBe(text);
  });

  it('shows a body in one colour when ui.syntaxHighlight is off', async () => {
    await host({ type: 'settings', strings: STRINGS, settings: { 'ui.syntaxHighlight': false } });
    rowOf(1).click();
    await settle();
    $('#detail-views [data-show-body]').click();
    await answer('body', { text: '{"a": 1}', base64: false, truncated: false });
    expect($('#body pre').textContent).toBe('{"a": 1}');
    expect($('#body pre').children).toHaveLength(0);
  });

  it("takes the app's theme from its settings", async () => {
    await host({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await host({ type: 'settings', strings: STRINGS, settings: {} });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('ignores messages that do not come from the panels window', async () => {
    const before = posted.length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { confirmClear: true } }, source: null }));
    await settle();
    expect(posted.length).toBe(before);
    expect($('#confirm')).toBeNull();
  });
});
