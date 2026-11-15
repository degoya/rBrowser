// The panel in a window of its own: the bridge messages it sends and how it renders the
// plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-84.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { CoverageDetail, CoverageSummary } from './panel/model';

type Posted = { type: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

/** The plugin's English texts, flattened as the host sends them. */
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

const SUMMARY: CoverageSummary = {
  mode: 'load',
  running: false,
  started: 1,
  finished: 2,
  files: [
    { id: 0, url: 'http://localhost:8080/coverage/site.css', kind: 'css', inline: false },
    { id: 1, url: 'http://localhost:8080/coverage/app.js', kind: 'js', inline: false },
  ],
  views: [
    { view: 1, label: 'View 1', breakpoint: 'desktop', width: 1280, engine: 'chromium', url: '', status: 'measured', files: [{ file: 0, total: 74, used: 41, unusedPercent: 44.6 }, { file: 1, total: 2048, used: 1024, unusedPercent: 50 }] },
    { view: 2, label: 'View 2', breakpoint: 'tablet', width: 768, engine: 'chromium', url: '', status: 'measured', files: [{ file: 0, total: 74, used: 18, unusedPercent: 75.7 }, { file: 1, total: 2048, used: 1024, unusedPercent: 50 }] },
    { view: 3, label: 'Firefox', breakpoint: 'mobile', width: 375, engine: 'firefox', url: '', status: 'unavailable', error: 'plugins.error.not_available', files: [] },
  ],
  truncated: false,
  findingCount: 571,
  findings: [],
};

const TEXT = 'body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n';

const DETAIL: CoverageDetail = {
  url: 'http://localhost:8080/coverage/site.css',
  kind: 'css',
  inline: false,
  total: 74,
  offset: 0,
  length: 8000,
  next: null,
  views: [
    { view: 1, label: 'View 1', breakpoint: 'desktop', total: 74, used: 41, unusedPercent: 44.6, segments: [[0, 18], [48, 71]] },
    { view: 2, label: 'View 2', breakpoint: 'tablet', total: 74, used: 18, unusedPercent: 75.7, segments: [[0, 18]] },
  ],
  text: TEXT,
  rules: [
    { prelude: 'body', start: 0, end: 18, usedIn: [1, 2] },
    { prelude: '.wide', start: 48, end: 71, usedIn: [1] },
    { prelude: '.never', start: 72, end: 73, usedIn: [] },
  ],
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers every not yet answered call of `tool` with `value`. */
async function answer(tool: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && m.tool === tool)) {
    window.dispatchEvent(new MessageEvent('message', { data: ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value }, source: window }));
  }
  posted = posted.filter((m) => !(m.type === 'call' && m.tool === tool));
  await settle();
}

const fileRows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#files tbody tr[data-selectable="true"]')];

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.coverage.', name: 'coverage panel' }).unmount;
  await fromHost({ type: 'settings', strings: strings(), settings: {} });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the Coverage panel', () => {
  it('asks for its texts, then for the last measurement', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    const get = posted.find((m) => m.type === 'call' && m.tool === 'get');
    expect(get?.args).toEqual({ mode: 'last' });
    expect($('#run-load')?.textContent).toBe('Measure page load');
    expect($('#title')?.textContent).toBe('Coverage');
  });

  it('shows one row per file with one value and a bar, the largest lever first', async () => {
    await answer('get', SUMMARY);
    const all = fileRows();
    expect(all.map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['app.js', 'site.css']);
    expect(all.map((row) => row.querySelector('[data-type]')?.textContent)).toEqual(['JS', 'CSS']);
    expect(all.map((row) => row.querySelector('[data-value]')?.textContent)).toEqual(['1.0 KB (50 %)', '33 B (45 %)']);
    expect(all[1]?.querySelector('[data-span]')?.textContent).toBe('45-76 %');
    expect(all[0]?.querySelector('[data-span]')).toBeNull();
    expect(all[0]?.querySelectorAll('[role="progressbar"]').length).toBe(1);
    expect($('#totals')?.textContent).toContain('1.0 KB of 2.1 KB unused (50 %)');
    expect(($('#export') as HTMLButtonElement).disabled).toBe(false);
    expect(($('#stop') as HTMLButtonElement).disabled).toBe(true);
  });

  it('names a view it could not measure once, and says what it counts', async () => {
    await answer('get', SUMMARY);
    expect($('#not-measured')?.textContent).toBe('Not measured: mobile (Firefox).');
    expect(document.body.textContent?.match(/Firefox/g)).toHaveLength(1);
    expect($('#summary')?.textContent).toBe('Page load measurement, 2 breakpoints measured, 2 files: per file the bytes unused in every breakpoint that loads it.');
    expect($('#summary')?.textContent).not.toContain('571');
  });

  it('filters by type and by address', async () => {
    await answer('get', SUMMARY);
    const tabs = [...document.querySelectorAll<HTMLElement>('#kinds [role="tab"]')];
    expect(tabs.map((tab) => tab.textContent)).toEqual(['All (2)', 'CSS (1)', 'JS (1)']);
    tabs[1]?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    tabs[1]?.click();
    await settle();
    expect(fileRows().map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['site.css']);
    tabs[0]?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    tabs[0]?.click();
    await settle();
    const search = $('#search') as HTMLInputElement;
    search.value = 'APP.JS';
    search.dispatchEvent(new Event('input'));
    await settle();
    expect(fileRows().map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['app.js']);
    search.value = 'nothing';
    search.dispatchEvent(new Event('input'));
    await settle();
    expect($('#files tbody')?.textContent).toContain('No file matches the filter.');
  });

  it('opens a row to its values per breakpoint, then the file with its rules and the unused source marked', async () => {
    await answer('get', SUMMARY);
    const css = fileRows()[1] as HTMLElement;
    css.click();
    await settle();
    const views = [...document.querySelectorAll<HTMLElement>('[data-views] [data-view]')];
    expect(views.map((view) => view.textContent)).toEqual(['desktop33 B of 74 B unused (45 %)', 'tablet56 B of 74 B unused (76 %)']);
    ($('[data-views] [data-open]') as HTMLButtonElement).click();
    const file = posted.find((m) => m.type === 'call' && m.tool === 'file');
    expect(file?.args).toEqual({ url: 'http://localhost:8080/coverage/site.css', kind: 'css', offset: 0 });
    await answer('file', DETAIL);
    expect($('#detail')).not.toBeNull();
    const rules = [...document.querySelectorAll('#items li')].map((li) => li.textContent);
    expect(rules).toEqual(['bodyused at: desktop, tablet', '.wideused at: desktop', '.neverused nowhere']);
    expect(document.querySelector('#items li:last-child [data-where]')?.getAttribute('data-tone')).toBe('warn');
    const unused = [...document.querySelectorAll('#source .cov-unused')].map((span) => span.textContent).join('');
    expect(unused.startsWith('\n@media (min-width: 1024px) { ')).toBe(true);
    expect($('#source')?.textContent).toBe(TEXT);
    expect(document.querySelectorAll('#source [class*="hl-"]').length).toBeGreaterThan(0);
    ($('#close') as HTMLButtonElement).click();
    await settle();
    expect($('#detail')).toBeNull();
  });

  it('moves between the rows with the arrow keys', async () => {
    await answer('get', SUMMARY);
    const [first, second] = fileRows();
    first?.focus();
    first?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement).toBe(second);
  });

  it('runs the commands and shows what the export wrote', async () => {
    await answer('get', SUMMARY);
    ($('#run-load') as HTMLButtonElement).click();
    await settle();
    expect(posted.some((m) => m.type === 'call' && m.command === 'run_load')).toBe(true);
    expect($('#status')?.textContent).toBe('Reloading every view and measuring...');
    expect($('#status')?.dataset.tone).toBe('busy');
    ($('#export') as HTMLButtonElement).click();
    const exported = posted.find((m) => m.type === 'call' && m.command === 'export');
    await fromHost({ type: 'result', id: exported?.id, ok: true, value: { written: ['.rbrowser/out/coverage/coverage.json'] } });
    expect($('#status')?.textContent).toBe('Written: .rbrowser/out/coverage/coverage.json');
    expect($('#status')?.dataset.tone).toBe('ok');
  });

  // Review of RB-080-62 (2): after the panels window stopped waiting for a long load run, the
  // busy line "measuring" stayed with its spinner for good.
  it('drops the busy line once the load run it waited for is in, or a poll fails', async () => {
    await answer('get', SUMMARY);
    ($('#run-load') as HTMLButtonElement).click();
    const run = posted.find((m) => m.type === 'call' && m.command === 'run_load');
    await fromHost({ type: 'result', id: run?.id, ok: false, error: 'plugins.error.host_failed reason=timeout' });
    const line = (): HTMLElement => $('#status') as HTMLElement;
    expect(line().dataset.tone).toBe('busy');
    await answer('get', SUMMARY);
    expect(line().dataset.tone).toBe('busy');
    await fromHost({ type: 'event', message: { refresh: true } });
    await answer('get', { ...SUMMARY, started: 5, finished: 6 });
    expect(line().textContent).toBe('');
    expect(line().dataset.tone).toBeUndefined();
    ($('#run-load') as HTMLButtonElement).click();
    const again = [...posted].reverse().find((m) => m.type === 'call' && m.command === 'run_load');
    await fromHost({ type: 'result', id: again?.id, ok: false, error: 'plugins.error.host_failed reason=timeout' });
    await fromHost({ type: 'event', message: { refresh: true } });
    // The read on its way when the event came is overtaken; the one after it fails.
    await answer('get', 'plugins.error.busy', false);
    await answer('get', 'plugins.error.busy', false);
    expect(line().dataset.tone).toBe('error');
    expect(line().textContent).toBe('The plugin is busy.');
    await fromHost({ type: 'event', message: { refresh: true } });
    await answer('get', { ...SUMMARY, started: 5, finished: 6 });
    expect(line().dataset.tone).toBeUndefined();
  });

  // Review of RB-080-84: a `refresh` event during a read was dropped; the older answer stayed.
  it('reads again after a refresh event that came during a read, and shows only that answer', async () => {
    await fromHost({ type: 'event', message: { refresh: true } });
    const polled = posted.filter((m) => m.type === 'call' && m.tool === 'get');
    expect(polled).toHaveLength(1);
    await fromHost({ type: 'event', message: { refresh: true } });
    await fromHost({ type: 'result', id: polled[0]?.id, ok: true, value: SUMMARY });
    posted = posted.filter((m) => m.id !== polled[0]?.id);
    expect($('#files')).toBeNull();
    await answer('get', { ...SUMMARY, files: [SUMMARY.files[1]] });
    expect(fileRows().map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['app.js']);
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: strings(), settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await fromHost({ type: 'settings', strings: strings(), settings: {} });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('says when nothing was measured yet', async () => {
    await answer('get', 'plugin.coverage.errors.none', false);
    expect($('#empty')?.textContent).toBe('No coverage measured yet.');
    expect($('#files')).toBeNull();
    expect($('#status')?.dataset.tone).toBeUndefined();
  });

  it('ignores messages from anyone but the panels window', async () => {
    const before = posted.filter((m) => m.type === 'call' && m.tool === 'get').length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { refresh: true } }, source: null }));
    await settle();
    expect(posted.filter((m) => m.type === 'call' && m.tool === 'get')).toHaveLength(before);
  });
});
