// The panel in a window of its own: the bridge messages it sends and how it renders the
// plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-84.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { ReportAnswer } from './panel/model';

type Posted = { type: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

/** The plugin's English texts, flattened as the host sends them, and a host text. */
function strings(): Record<string, string> {
  const nested = JSON.parse(readFileSync(join(process.cwd(), '..', 'locales', 'en.json'), 'utf8')) as Record<string, unknown>;
  const flat: Record<string, string> = { 'plugins.error.busy': 'The plugin is busy.' };
  const walk = (value: unknown, path: string): void => {
    if (typeof value === 'string') flat[path] = value;
    else if (value && typeof value === 'object') for (const [key, inner] of Object.entries(value)) walk(inner, path ? path + '.' + key : key);
  };
  walk(nested, '');
  return flat;
}

const VIEWS = [
  { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', width: 375, url: 'https://a.test/', captures: true },
  { id: 2, label: 'Tablet', breakpoint: 'tablet', engine: 'chromium', width: 768, url: 'https://a.test/', captures: true },
  { id: 3, label: 'Laptop', breakpoint: 'laptop', engine: 'chromium', width: 1280, url: 'https://a.test/', captures: true },
  { id: 4, label: 'Desktop', breakpoint: 'desktop', engine: 'firefox', width: 1600, url: 'https://a.test/', captures: true },
];

const ANSWER: ReportAnswer = {
  captured: true,
  total: 2,
  truncated: false,
  summary: { 'js-exception': 1, 'http-not-found': 1 },
  severities: { error: 2, warning: 0, info: 0 },
  views: VIEWS,
  entries: [
    {
      id: 'aaaa', rule: 'js-exception', severity: 'error', text: 'Uncaught Error: boom', count: 4,
      views: [1, 2, 3, 4], perView: { '1': 1, '2': 1, '3': 1, '4': 1 }, first: 1, last: 4,
      source: { url: 'https://a.test/app.js', line: 3, column: 9 }, stack: 'at f (https://a.test/app.js:3:9)',
    },
    {
      id: 'bbbb', rule: 'http-not-found', severity: 'error', text: 'GET https://a.test/hero-375.png 404', count: 1,
      views: [1], perView: { '1': 1 }, onlyIn: ['mobile'], first: 2, last: 2, url: 'https://a.test/hero-375.png',
      request: { method: 'GET', status: 404, resourceType: 'Image' },
    },
  ],
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function host(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the newest call to `name` with `value` (or fails it with the code `value`). */
async function answer(name: string, value: unknown, ok = true): Promise<void> {
  const call = [...posted].reverse().find((m) => m.type === 'call' && (m.tool === name || m.command === name));
  expect(call, `a call to ${name}`).toBeDefined();
  await host(ok ? { type: 'result', id: call?.id, ok: true, value } : { type: 'result', id: call?.id, ok: false, error: value });
}

const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('li[data-entry]')];

beforeEach(async () => {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.error-report.', name: 'error report panel' }).unmount;
  await host({ type: 'settings', strings: strings(), settings: { 'error-report.reloadBeforeReport': true } });
  await answer('get', ANSWER);
});

afterEach(() => {
  unmount();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the error report panel', () => {
  it('asks for its settings, then for the report', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(posted.find((m) => m.tool === 'get')?.args).toEqual({ limit: 200 });
    expect($('#title')?.textContent).toBe('Error Report');
  });

  // Review of RB-080-84: an event (a clear) during a read was dropped; the older report stayed.
  it('reads again after an event that came during a read, and shows only that answer', async () => {
    await host({ type: 'event', message: { refresh: true } });
    const polled = [...posted].reverse().find((m) => m.tool === 'get');
    await host({ type: 'event', message: { cleared: true } });
    await host({ type: 'result', id: polled?.id, ok: true, value: ANSWER });
    const last = [...posted].reverse().find((m) => m.tool === 'get');
    expect(last?.id).not.toBe(polled?.id);
    await answer('get', { ...ANSWER, entries: [] });
    expect(rows()).toHaveLength(0);
  });

  it('lists the entries by category with a dot per view', () => {
    const headings = [...document.querySelectorAll('[data-category] h2')].map((h) => h.textContent);
    expect(headings).toEqual(['Uncaught JavaScript exception1', 'Resource not found (404, 410)1']);
    expect(rows()).toHaveLength(2);
    expect(rows()[0]?.querySelectorAll('[data-dot]')).toHaveLength(4);
    expect(rows()[0]?.querySelector('[data-dot][data-engine="firefox"]')).not.toBeNull();
    expect(rows()[0]?.querySelector('[data-count]')?.textContent).toBe('4 times');
    const dots = [...(rows()[1]?.querySelectorAll('[data-dot]') ?? [])];
    expect(dots.map((dot) => dot.getAttribute('aria-label'))).toEqual(['Mobile (mobile, chromium): 1 time']);
    expect(rows()[1]?.querySelector('[data-only]')?.textContent).toBe('only in: mobile');
    expect([...document.querySelectorAll('#views [data-view]')].map((view) => view.textContent)).toEqual(['Mobile', 'Tablet', 'Laptop', 'Desktop']);
  });

  it('opens an entry to its source, request and stack with Enter and the arrows', async () => {
    // RB-090-23 (WCAG 4.1.2): an entry that opens is a tree item of its category's tree, so its
    // aria-expanded has a role to belong to.
    expect(rows().map((row) => row.getAttribute('role'))).toEqual(['treeitem', 'treeitem']);
    expect(rows()[0]?.parentElement?.getAttribute('role')).toBe('tree');
    expect(rows().map((row) => row.tabIndex)).toEqual([0, -1]);
    expect(rows()[0]?.querySelector('[data-details]')).toBeNull();
    rows()[0]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    const first = rows()[0]?.querySelector<HTMLElement>('[data-details]');
    expect(rows()[0]?.getAttribute('aria-expanded')).toBe('true');
    expect(first?.textContent).toContain('app.js:3:9');
    expect(first?.querySelector('[data-stack]')?.textContent).toBe('at f (https://a.test/app.js:3:9)');
    rows()[0]?.focus();
    rows()[0]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(rows()[1]);
    expect(rows().map((row) => row.tabIndex)).toEqual([-1, 0]);
    rows()[1]?.click();
    await settle();
    expect(rows()[1]?.querySelector('[data-details]')?.textContent).toContain('GET 404 Image');
    // A click inside the details leaves them open.
    (rows()[1]?.querySelector('[data-request]') as HTMLElement).click();
    await settle();
    expect(rows()[1]?.querySelector('[data-details]')).not.toBeNull();
  });

  it('runs Report now through the command run with the reload the setting chose', async () => {
    expect(($('#reload') as HTMLElement).dataset.state).toBe('checked');
    ($('#run') as HTMLButtonElement).click();
    await settle();
    const run = posted.find((m) => m.command === 'run');
    expect(run?.args).toEqual({ reload: true });
    await answer('run', { reloaded: [{ view: 1 }, { view: 2 }], pending: true });
    await answer('get', ANSWER);
    expect($('#status')?.textContent).toBe('Reloaded 2 views; the report shows the fresh load.');
    expect($('#status')?.dataset.tone).toBe('ok');
  });

  it('says a failed command in one sentence, never the code', async () => {
    ($('#run') as HTMLButtonElement).click();
    await settle();
    await answer('run', 'plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The plugin is busy.');
    expect($('#status')?.dataset.tone).toBe('error');
  });

  it('clears through the command clear and asks again', async () => {
    ($('#clear') as HTMLButtonElement).click();
    await settle();
    expect(posted.some((m) => m.command === 'clear')).toBe(true);
    await answer('clear', { clearedAt: 5 });
    await answer('get', { ...ANSWER, total: 0, entries: [] });
    expect(rows()).toHaveLength(0);
    expect($('#empty')?.textContent).toBe('No errors in the views.');
  });

  it('says when the app hands it no capture', async () => {
    vi.advanceTimersByTime(2000);
    await settle();
    await answer('get', { ...ANSWER, captured: false, total: 0, entries: [] });
    expect($('#status')?.textContent).toBe('The report receives nothing yet: the app does not hand this plugin the views\' errors.');
    expect($('#status')?.dataset.tone).toBe('warn');
  });

  it("takes the app's theme from its settings", async () => {
    await host({ type: 'settings', strings: strings(), settings: { 'ui.theme': 'light' } });
    expect(document.documentElement.dataset.theme).toBe('light');
    await host({ type: 'settings', strings: strings(), settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await host({ type: 'settings', strings: strings(), settings: {} });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  // RB-080-62, RB-080-84: Report now the main action, Clear an icon button, the severity a badge.
  it('has Report now as the main action, Clear as an icon button, the severity as a badge', () => {
    expect($('#run')?.textContent).toBe('Report now');
    expect($('#run')?.hasAttribute('data-primary')).toBe(true);
    const clear = $('#clear') as HTMLButtonElement;
    expect(clear.getAttribute('aria-label')).toBe('Clear');
    expect(clear.textContent).toBe('');
    const tag = $('li[data-entry] [data-severity-badge]');
    expect(tag?.textContent).toBe('Error');
    expect(tag?.dataset.tone).toBe('error');
    // Lucide's circle-x: the circle and the cross.
    expect(tag?.querySelector('svg.iconify--lucide circle')).not.toBeNull();
    expect(tag?.querySelector('svg path')?.getAttribute('d')).toBe('m15 9l-6 6m0-6l6 6');
  });

  it('asks again when the plugin posts a reload', async () => {
    const before = posted.filter((m) => m.tool === 'get').length;
    await host({ type: 'event', message: { reloaded: 3 } });
    expect(posted.filter((m) => m.tool === 'get').length).toBe(before + 1);
  });

  it('ignores messages that do not come from the panels window', async () => {
    const before = posted.length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { reloaded: 1 } }, source: null }));
    await settle();
    expect(posted.length).toBe(before);
  });
});
