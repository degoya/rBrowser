// The panel in a window of its own: the bridge messages it sends and how it renders the
// plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-84.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { StorageList, StoragePin, StorageView } from './panel/model';

type Posted = { type: string; id?: number; command?: string; tool?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

/** The plugin's English texts, flattened as the host sends them. */
function strings(): Record<string, string> {
  const nested = JSON.parse(readFileSync(join(process.cwd(), '..', 'locales', 'en.json'), 'utf8')) as Record<string, unknown>;
  const flat: Record<string, string> = {};
  const walk = (value: unknown, path: string): void => {
    if (typeof value === 'string') flat[path] = value;
    else if (value && typeof value === 'object') for (const [key, inner] of Object.entries(value)) walk(inner, path ? path + '.' + key : key);
  };
  walk(nested, '');
  return flat;
}
const STRINGS = strings();

const VIEWS: StorageView[] = [
  { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', session: 'default', url: 'http://127.0.0.1:18767/', origin: 'http://127.0.0.1:18767', status: 'ok' },
  { id: 3, label: 'Desktop', breakpoint: 'desktop', engine: 'chromium', session: 'admin', url: 'http://127.0.0.1:18767/', origin: 'http://127.0.0.1:18767', status: 'ok' },
  { id: 2, label: 'Tablet', breakpoint: 'tablet', engine: 'chromium', session: 'default', url: 'http://127.0.0.1:18767/', origin: 'http://127.0.0.1:18767', status: 'ok' },
  { id: 4, label: 'Firefox', breakpoint: 'fx', engine: 'firefox', session: 'default', url: 'http://127.0.0.1:18767/', origin: 'http://127.0.0.1:18767', status: 'unsupported' },
];

const COOKIES: StorageList = {
  kind: 'cookies',
  views: VIEWS,
  total: 1,
  shown: 1,
  truncated: false,
  rows: [
    {
      id: 'c1', scope: '127.0.0.1', path: '/', name: 'consent', state: 'missing', pinned: false,
      cells: { '1': { value: 'yes', length: 3, domain: '127.0.0.1', path: '/', httpOnly: false, secure: false }, '2': { value: 'yes', length: 3 }, '3': null },
    },
  ],
};

const PIN: StoragePin = { id: 'a1b2c3d4e5f60718', kind: 'localStorage', origin: 'http://127.0.0.1:18767', key: 'featureFlag', value: 'on', session: null, enabled: true };

const $ = (selector: string): HTMLElement => document.querySelector<HTMLElement>(selector) as HTMLElement;
const settle = async (): Promise<void> => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
  await nextTick();
};

function host(data: unknown): void {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
}

/** Answers the newest call with `op` with `value`. */
async function answer(op: string, value: unknown, ok = true): Promise<void> {
  const call = [...posted].reverse().find((m) => m.type === 'call' && m.command === 'panel' && m.args?.op === op);
  expect(call, `a call of ${op}`).toBeDefined();
  host(ok ? { type: 'result', id: call?.id, ok: true, value } : { type: 'result', id: call?.id, ok: false, error: value });
  await settle();
}

const cells = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#grid [data-cell][data-view]')];
const press = (target: Element, key: string): void => {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
};
const choose = (tab: HTMLElement): void => {
  tab.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  tab.click();
};

beforeEach(async () => {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.storage-panel.', name: 'storage panel' }).unmount;
  host({ type: 'settings', strings: STRINGS, settings: { 'storage-panel.pollMs': 1000 } });
  await settle();
  await answer('list', COOKIES);
  await answer('pins', { pins: [] });
});

afterEach(() => {
  unmount();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the storage panel', () => {
  it('keeps its tabs in one row without a vertical scroll bar', () => {
    // RB-090-48: the tab row scrolls sideways only, never a vertical scroll bar beside the tabs.
    const row = [...(document.querySelector('#tabs [role="tablist"]')?.classList ?? [])];
    expect(row).toContain('overflow-y-hidden');
    expect(row.filter((name) => /^overflow-(y-)?(auto|scroll)$/.test(name))).toEqual([]);
  });

  it('asks for its texts, the first tab and the pins - all through the command panel', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    const calls = posted.filter((m) => m.type === 'call');
    expect(calls.every((m) => m.command === 'panel' && !('tool' in m))).toBe(true);
    expect(calls.map((m) => m.args?.op)).toEqual(['list', 'pins']);
    expect($('#title').textContent).toBe('Storage');
  });

  it('groups the views under their sessions and greys an unread engine', () => {
    const sessions = [...document.querySelectorAll('#grid [data-session]')].map((span) => [span.textContent, (span.closest('th') as HTMLTableCellElement).colSpan]);
    expect(sessions).toEqual([['Session default', 3], ['Session admin', 1]]);
    const heads = [...document.querySelectorAll<HTMLElement>('#grid [data-view-head]')];
    expect(heads.map((head) => head.querySelector('[data-label]')?.textContent)).toEqual(['Mobile', 'Tablet', 'Firefox', 'Desktop']);
    expect(heads[2]?.hasAttribute('data-off')).toBe(true);
    expect(heads[2]?.dataset.hint).toBe('firefox views are not read by the storage panel yet.');
    expect(heads[0]?.dataset.hint).toBe('http://127.0.0.1:18767/');
  });

  it('shows the cookie in default and marks it missing in admin', () => {
    expect(cells().map((cell) => cell.dataset.state)).toEqual(['value', 'value', 'unsupported', 'missing']);
    expect(cells()[3]?.textContent).toBe('missing');
    expect(cells()[2]?.textContent).toBe('not read');
    expect($('#grid [data-name] [data-state]').textContent).toBe('missing');
    expect(cells()[0]?.title).toBe('127.0.0.1/ · session');
  });

  it('moves through the grid with the arrow keys, one tab stop at a time', async () => {
    const first = $('#grid [data-name]');
    expect(first.tabIndex).toBe(0);
    first.focus();
    press(first, 'ArrowRight');
    await settle();
    expect(document.activeElement).toBe(cells()[0]);
    expect($('#grid [data-name]').tabIndex).toBe(-1);
    expect(cells()[0]?.tabIndex).toBe(0);
    press(cells()[0] as HTMLElement, 'End');
    await settle();
    expect(document.activeElement).toBe(cells()[3]);
  });

  it('opens the editor with Enter, loads the whole value and saves it in that view', async () => {
    const cell = cells()[0] as HTMLElement;
    cell.focus();
    press(cell, 'Enter');
    await settle();
    expect($('#editor').dataset.mode).toBe('edit');
    expect($('#editor-title').textContent?.trim()).toBe('consent in Mobile');
    await answer('value', { value: 'yes', length: 3 });
    const value = $('#ed-value') as HTMLTextAreaElement;
    expect(value.value).toBe('yes');
    expect(($('#ed-name') as HTMLInputElement).readOnly).toBe(true);
    value.value = 'no';
    $('#ed-save').click();
    await settle();
    const save = posted.filter((m) => m.args?.op === 'set').pop();
    expect(save?.args).toMatchObject({ op: 'set', kind: 'cookies', target: 1, name: 'consent', value: 'no', cookie: { domain: '127.0.0.1', path: '/' } });
    await answer('set', { results: [{ view: 1, ok: true }] });
    expect($('#ed-status').textContent).toBe('1 of 1 view written.');
    press($('#ed-save'), 'Escape');
    await settle();
    expect(document.querySelector('#editor')).toBeNull();
  });

  it('applies to all views and names the views that refused', async () => {
    cells()[0]?.click();
    await settle();
    $('#ed-all').click();
    await settle();
    await answer('set', { results: [{ view: 1, ok: true }, { view: 4, ok: false, error: 'plugin.storage-panel.errors.other_origin' }] });
    expect($('#ed-status').textContent).toBe('1 of 2 views written. The view shows another origin.');
    expect(posted.filter((m) => m.args?.op === 'set').pop()?.args?.target).toBe('all');
  });

  it('deletes in every view only after an inline confirmation with the count', async () => {
    cells()[0]?.click();
    await settle();
    $('#ed-delete-all').focus();
    $('#ed-delete-all').click();
    await settle();
    expect(posted.some((m) => m.args?.op === 'delete')).toBe(false);
    expect($('#ed-confirm').textContent).toContain('Delete consent in all 3 views?');
    // RB-090-23 (WCAG 2.4.3): the question takes the focus on its safe answer, and gives it back.
    expect(document.activeElement).toBe($('#ed-confirm-no'));
    $('#ed-confirm-no').click();
    await settle();
    expect(document.activeElement).toBe($('#ed-delete-all'));
    $('#ed-delete-all').click();
    await settle();
    ($('#ed-confirm').querySelector('#ed-confirm-yes') as HTMLElement).click();
    await settle();
    expect(document.activeElement).toBe($('#ed-delete-all'));
    expect(posted.filter((m) => m.args?.op === 'delete').pop()?.args).toMatchObject({ op: 'delete', target: 'all', name: 'consent' });
  });

  it('deletes a value in its view with the Delete key', async () => {
    const cell = cells()[1] as HTMLElement;
    cell.focus();
    press(cell, 'Delete');
    await settle();
    expect(posted.filter((m) => m.args?.op === 'delete').pop()?.args).toMatchObject({ op: 'delete', kind: 'cookies', target: 2, name: 'consent', cookie: { domain: '127.0.0.1', path: '/' } });
    expect(document.querySelector('#editor')).toBeNull();
  });

  it('adds an entry in a chosen view or all', async () => {
    $('#add').click();
    await settle();
    expect($('#editor').dataset.mode).toBe('add');
    expect(($('#ed-name') as HTMLInputElement).readOnly).toBe(false);
    expect($('#ed-target-row')).not.toBeNull();
    expect(document.querySelector('#ed-delete')).toBeNull();
    $('#ed-save').click();
    await settle();
    expect(posted.filter((m) => m.args?.op === 'set').pop()?.args?.target).toBe('all');
  });

  it('switches tabs and filters through list', async () => {
    choose($('#tabs').querySelectorAll<HTMLElement>('[role="tab"]')[1] as HTMLElement);
    await settle();
    expect(posted.filter((m) => m.args?.op === 'list').pop()?.args).toEqual({ op: 'list', kind: 'local' });
    await answer('list', { ...COOKIES, kind: 'local' });
    const filter = $('#filter-origin') as HTMLInputElement;
    filter.value = ' 127.0.0.1 ';
    filter.dispatchEvent(new Event('input'));
    await settle();
    expect(posted.filter((m) => m.args?.op === 'list').pop()?.args).toEqual({ op: 'list', kind: 'local', origin: '127.0.0.1' });
  });

  // Review of RB-080-84 (1): a tab switch during a poll was dropped and the poll's older answer -
  // cookies - stayed under Local Storage.
  it('never drops a tab switch made while a read is on its way, and shows only the newest answer', async () => {
    host({ type: 'event', message: { type: 'changed' } });
    await settle();
    const polled = posted.filter((m) => m.args?.op === 'list').pop();
    choose($('#tabs').querySelectorAll<HTMLElement>('[role="tab"]')[1] as HTMLElement);
    await settle();
    host({ type: 'result', id: polled?.id, ok: true, value: { ...COOKIES, rows: [{ ...COOKIES.rows[0], name: 'stale' }] } });
    await settle();
    expect(document.body.textContent).not.toContain('stale');
    const again = posted.filter((m) => m.args?.op === 'list').pop();
    expect(again?.id).not.toBe(polled?.id);
    expect(again?.args).toEqual({ op: 'list', kind: 'local' });
    await answer('list', { ...COOKIES, kind: 'local', rows: [{ ...COOKIES.rows[0], name: 'flag' }] });
    expect(document.body.textContent).toContain('flag');
  });

  it('reads again when a view changes and on the poll', async () => {
    const before = posted.filter((m) => m.args?.op === 'list').length;
    host({ type: 'event', message: { type: 'changed', view: 1 } });
    await settle();
    await answer('list', COOKIES);
    vi.advanceTimersByTime(1000);
    expect(posted.filter((m) => m.args?.op === 'list').length).toBe(before + 2);
  });

  it('says a truncated list and a failed read in the status line', async () => {
    host({ type: 'event', message: { type: 'changed' } });
    await settle();
    await answer('list', { ...COOKIES, truncated: true, shown: 1, total: 40 });
    expect($('#status').textContent).toBe('Showing 1 of 40 entries; narrow the filter to see the rest.');
    host({ type: 'event', message: { type: 'changed' } });
    await settle();
    await answer('list', 'plugin.storage-panel.errors.quota', false);
    expect($('#status').textContent).toBe("Error: The view's storage is full.");
    expect($('#status').dataset.tone).toBe('error');
  });

  it('lists pins and switches or removes them', async () => {
    cells()[0]?.click();
    await settle();
    $('#ed-pin').click();
    await settle();
    expect(posted.filter((m) => m.args?.op === 'pin').pop()?.args).toMatchObject({ op: 'pin', sessionOnly: false, kind: 'cookies', view: 1, name: 'consent' });
    await answer('pin', { pins: [PIN] });
    expect($('#pins').textContent).toContain('featureFlag = on');
    expect(document.querySelector('#pins-note')).toBeNull();
    const toggle = $('#pins [role="switch"]');
    expect(toggle.getAttribute('aria-label')).toBe('Pinned value on: featureFlag');
    toggle.click();
    await settle();
    expect(posted.filter((m) => m.args?.op === 'enable').pop()?.args).toEqual({ op: 'enable', id: PIN.id, enabled: false });
    ($('#pins [data-pin]').querySelector('button[aria-label^="Remove pin"]') as HTMLElement).click();
    await settle();
    expect(posted.filter((m) => m.args?.op === 'unpin').pop()?.args).toEqual({ op: 'unpin', id: PIN.id });
  });

  it('says why no pin applies in an untrusted workspace', async () => {
    cells()[0]?.click();
    await settle();
    $('#ed-pin').click();
    await settle();
    await answer('pin', { pins: [PIN] });
    ($('#pins [data-pin]').querySelector('button[aria-label^="Remove pin"]') as HTMLElement).click();
    await settle();
    await answer('unpin', 'plugins.error.untrusted_workspace', false);
    await answer('pins', { pins: [], untrusted: true });
    expect($('#pins-note').textContent).toBe('Pinned values apply only in a trusted workspace; trust the workspace to use them.');
  });

  // RB-080-65: JSON values in syntax colours, in the grid and in the editor.
  it('colours JSON values in the grid and the editor, and leaves other values plain', async () => {
    expect(document.querySelector('#grid span.value')?.children).toHaveLength(0);
    host({ type: 'event', message: { type: 'changed', view: 1 } });
    await settle();
    const json = '{"cart":["<img src=x onerror=alert(1)>"],"n":2}';
    const row = COOKIES.rows[0];
    if (!row) throw new Error('no row');
    await answer('list', { ...COOKIES, rows: [{ ...row, cells: { ...row.cells, '1': { value: '{"cart":[', length: 40, truncated: true }, '2': { value: json, length: json.length } } }] });
    const values = [...document.querySelectorAll('#grid span.value')];
    expect(values.map((value) => value.querySelector('.hl-key')?.textContent)).toEqual(['"cart"', '"cart"']);
    expect(document.querySelector('#grid img')).toBeNull();
    expect(values[1]?.textContent).toBe(json);
    expect(cells()[0]?.querySelector('[data-more]')?.textContent).toBe('…');
    expect(cells()[1]?.hasAttribute('data-differs')).toBe(true);
    expect(cells()[1]?.textContent).toContain('(differs)');
    expect(cells()[0]?.hasAttribute('data-differs')).toBe(false);
    const cell = cells()[0] as HTMLElement;
    cell.focus();
    press(cell, 'Enter');
    await settle();
    await answer('value', { value: json, length: json.length });
    const editor = ($('#ed-value') as HTMLTextAreaElement).parentElement as HTMLElement;
    expect(editor.className).toBe('hl-editor');
    expect(editor.querySelector('.hl-backdrop .hl-number')?.textContent).toBe('2');
  });

  it('shows values in one colour when ui.syntaxHighlight is off', async () => {
    host({ type: 'settings', strings: STRINGS, settings: { 'storage-panel.pollMs': 1000, 'ui.syntaxHighlight': false } });
    await settle();
    const row = COOKIES.rows[0];
    if (!row) throw new Error('no row');
    await answer('list', { ...COOKIES, rows: [{ ...row, cells: { ...row.cells, '1': { value: '{"a":1}', length: 7 } } }] });
    expect(document.querySelector('#grid span.value')?.textContent).toBe('{"a":1}');
    expect(document.querySelector('#grid span.value')?.children).toHaveLength(0);
  });

  it("takes the app's theme from its settings", async () => {
    host({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    await settle();
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('ignores messages that do not come from the panels window', async () => {
    const before = posted.length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { type: 'changed' } }, source: null }));
    await settle();
    expect(posted.length).toBe(before);
  });
});
