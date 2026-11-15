// The panel in a window of its own: the bridge messages it sends and how it renders the
// plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-84.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { $, answer, host, posted, settle, start, stop, type } from './harness';

const STRINGS: Record<string, string> = {
  'plugin.console.panels.console': 'Console',
  'plugin.console.levels.error': 'Errors',
  'plugin.console.levels.warning': 'Warnings',
  'plugin.console.panel.viewDot': '{label} ({breakpoint}, {engine}): {count} times',
  'plugin.console.panel.noCapture': 'No capture.',
  'plugin.console.panel.truncated': 'Showing the newest {shown} of {total} messages.',
  'plugin.console.panel.count': '{count} times',
  'plugin.console.panel.resultError': 'Error: {text}',
  'plugin.console.panel.clear': 'Clear',
  'plugin.console.panel.empty': 'No messages.',
  'plugin.console.errors.no_active_view': 'No view is active; run the command in all views instead.',
  'plugins.error.busy': 'The plugin is busy.',
};

const SETTINGS = { 'console.levels': ['error', 'warning'] };

const ANSWER = {
  captured: true,
  total: 2,
  shown: 2,
  truncated: false,
  levels: { error: 1, warning: 1, info: 0, log: 0, debug: 0, network: 0 },
  views: [
    { id: 1, label: 'Small', breakpoint: 'small', engine: 'chromium', width: 320 },
    { id: 2, label: 'Mobile', breakpoint: 'mobile', engine: 'firefox', width: 375 },
    { id: 3, label: 'Tablet', breakpoint: 'tablet', engine: 'webkit', width: 768 },
  ],
  entries: [
    {
      id: 'aaaa', level: 'warning', kind: 'console', text: 'deprecated layout API (viewport 320)', count: 3,
      views: [1, 2, 3], perView: { '1': 1, '2': 1, '3': 1 }, first: 1, last: 3,
      source: { url: 'http://x.test/console.html', line: 14 },
    },
    {
      id: 'bbbb', level: 'error', kind: 'exception', text: 'Uncaught Error: layout failed at 320 px', count: 2,
      views: [1, 2], perView: { '1': 1, '2': 1 }, first: 4, last: 5,
      source: { url: 'http://x.test/console.html', line: 18 }, stack: 'at http://x.test/console.html:18:37',
    },
  ],
};

const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#entries [data-entry]')];
const stackOf = (id: string): HTMLElement | null => {
  const row = document.querySelector(`#entries [data-id="${id}"]`)?.closest('tr');
  const next = row?.nextElementSibling;
  return next?.querySelector<HTMLElement>('.stack') ?? null;
};

async function runCommand(text: string): Promise<void> {
  await type('#expression', text);
  $('#expression').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await settle();
}

beforeEach(async () => {
  await start(STRINGS, SETTINGS);
  await answer('grouped', ANSWER);
});

afterEach(() => {
  stop();
});

describe('the console panel', () => {
  it('asks for its settings, then for the list with the levels of the setting', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    const grouped = posted.find((m) => m.tool === 'grouped');
    expect(grouped?.args).toEqual({ levels: ['error', 'warning'], limit: 200 });
    expect($('#title').textContent).toBe('Console');
    expect($('#clear').getAttribute('aria-label')).toBe('Clear');
  });

  it('shows two entries, the error with the dots of the two narrow views', () => {
    const all = rows();
    expect(all).toHaveLength(2);
    const tr = (row: HTMLElement | undefined) => row?.closest('tr') as HTMLElement;
    const errorDots = tr(all[1]).querySelectorAll('[data-dot]');
    expect([...errorDots].map((dot) => dot.getAttribute('aria-label'))).toEqual([
      'Small (small, chromium): 1 times',
      'Mobile (mobile, firefox): 1 times',
    ]);
    expect(tr(all[0]).querySelectorAll('[data-dot]')).toHaveLength(3);
    expect(all[0]?.querySelector('[data-count]')?.textContent).toBe('3 times');
    expect(all[1]?.querySelector('[data-source]')?.textContent).toBe('console.html:18');
    expect(all[1]?.querySelector('[data-source]')?.getAttribute('title')).toBe('http://x.test/console.html:18');
    expect(tr(all[1]).querySelector('[data-level]')?.textContent).toBe('Errors');
  });

  it('gives Firefox and WebKit views a dot of their own shape', () => {
    const dots = rows()[0]?.closest('tr')?.querySelectorAll<HTMLElement>('[data-dot]') ?? [];
    expect([...dots].map((dot) => dot.dataset.engine)).toEqual(['chromium', 'firefox', 'webkit']);
    expect([...dots].map((dot) => dot.className.includes('rotate-45') ? 'diamond' : dot.className.includes('rounded-full') ? 'round' : 'square')).toEqual(['round', 'square', 'diamond']);
  });

  it('switches a level off and asks again', async () => {
    const warnings = $<HTMLButtonElement>('#levels button[data-level="warning"]');
    expect(warnings.getAttribute('aria-pressed')).toBe('true');
    expect($('#levels button[data-level="debug"]').getAttribute('aria-pressed')).toBe('false');
    warnings.click();
    await settle();
    const last = [...posted].reverse().find((m) => m.tool === 'grouped');
    expect(last?.args).toEqual({ levels: ['error'], limit: 200 });
    expect(warnings.getAttribute('aria-pressed')).toBe('false');
  });

  // Review of RB-080-84 (2): a level switched during a poll was dropped.
  it('never drops a level switched while a read is on its way', async () => {
    await host({ type: 'event', message: { cleared: true } });
    const polled = [...posted].reverse().find((m) => m.tool === 'grouped');
    $<HTMLButtonElement>('#levels button[data-level="warning"]').click();
    await settle();
    await host({ type: 'result', id: polled?.id, ok: true, value: ANSWER });
    const last = [...posted].reverse().find((m) => m.tool === 'grouped');
    expect(last?.id).not.toBe(polled?.id);
    expect(last?.args).toEqual({ levels: ['error'], limit: 200 });
  });

  it('filters by text and by view', async () => {
    await type('#search', ' layout ');
    expect([...posted].reverse().find((m) => m.tool === 'grouped')?.args).toEqual({ levels: ['error', 'warning'], search: 'layout', limit: 200 });
    await answer('grouped', ANSWER);
    $<HTMLButtonElement>('#views button[data-view="2"]').click();
    await settle();
    expect([...posted].reverse().find((m) => m.tool === 'grouped')?.args).toEqual({ levels: ['error', 'warning'], views: [2], search: 'layout', limit: 200 });
    expect($('#views button[data-view="2"]').getAttribute('aria-pressed')).toBe('true');
  });

  it('moves through the rows with the arrows and opens a stack trace with Enter', async () => {
    let all = rows();
    expect(all.map((row) => row.tabIndex)).toEqual([0, -1]);
    expect(all[0]?.hasAttribute('aria-expanded')).toBe(false);
    all[0]?.focus();
    all[0]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    all = rows();
    expect(document.activeElement).toBe(all[1]);
    expect(all.map((row) => row.tabIndex)).toEqual([-1, 0]);
    expect(stackOf('bbbb')).toBeNull();
    expect(all[1]?.getAttribute('aria-expanded')).toBe('false');
    all[1]?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(stackOf('bbbb')).not.toBeNull();
    expect(rows()[1]?.getAttribute('aria-expanded')).toBe('true');
  });

  it('runs a command through the command exec and joins equal results', async () => {
    await runCommand('document.title');
    const call = posted.find((m) => m.command === 'exec');
    expect(call?.args).toEqual({ expression: 'document.title', target: 'all' });
    expect(call?.tool).toBeUndefined();
    await answer('exec', { results: [{ ok: true, type: 'string', text: '"Console fixture"', views: [1, 2, 3] }] });
    const outcomes = document.querySelectorAll('#results .outcome');
    expect(outcomes).toHaveLength(1);
    expect(outcomes[0]?.querySelectorAll('[data-dot]')).toHaveLength(3);
    expect(outcomes[0]?.textContent).toContain('"Console fixture"');
    expect($<HTMLTextAreaElement>('#expression').value).toBe('');
  });

  it('runs in the active view when the setting says so, and says a failure as a sentence', async () => {
    stop();
    await start(STRINGS, { ...SETTINGS, 'console.execTarget': 'active' });
    await answer('grouped', ANSWER);
    await runCommand('1');
    expect([...posted].reverse().find((m) => m.command === 'exec')?.args).toEqual({ expression: '1', target: 'active' });
    await answer('exec', 'plugin.console.errors.no_active_view', false);
    expect($('#results .outcome.failed').textContent).toBe('Error: No view is active; run the command in all views instead.');
    await runCommand('x');
    await answer('exec', { results: [{ ok: false, type: 'error', text: 'ReferenceError: x is not defined', views: [1] }] });
    expect([...document.querySelectorAll('#results .outcome.failed')].map((one) => one.textContent)).toContain('Error: ReferenceError: x is not defined');
  });

  it('says when the app hands it no capture, or the list is cut, in the status line', async () => {
    vi.advanceTimersByTime(1500);
    await answer('grouped', { ...ANSWER, captured: false, total: 0, shown: 0, entries: [] });
    expect($('#status').textContent).toBe('No capture.');
    expect($('#empty').textContent).toBe('No messages.');
    vi.advanceTimersByTime(1500);
    await answer('grouped', { ...ANSWER, truncated: true, shown: 2, total: 900 });
    expect($('#status').textContent).toBe('Showing the newest 2 of 900 messages.');
    vi.advanceTimersByTime(1500);
    await answer('grouped', 'plugins.error.busy', false);
    expect($('#status').textContent).toBe('The plugin is busy.');
    expect($('#status').dataset.tone).toBe('error');
  });

  it('asks again after a clear and on a cleared event', async () => {
    $<HTMLButtonElement>('#clear').click();
    await settle();
    expect(posted.some((m) => m.command === 'clear')).toBe(true);
    await answer('clear', {});
    const count = posted.filter((m) => m.tool === 'grouped').length;
    expect(count).toBeGreaterThanOrEqual(2);
    await answer('grouped', ANSWER);
    await host({ type: 'event', message: { cleared: true } });
    expect(posted.filter((m) => m.tool === 'grouped').length).toBe(count + 1);
  });

  // RB-080-65: values, stacks, objects and the command line in syntax colours.
  it('colours stacks, JSON messages, the command line and its results', async () => {
    rows()[1]?.click();
    await settle();
    const stack = stackOf('bbbb');
    expect(stack?.querySelector('.hl-keyword')?.textContent).toBe('at');
    expect(stack?.querySelector('.hl-string')?.textContent).toBe('http://x.test/console.html');
    expect(stack?.textContent).toBe('at http://x.test/console.html:18:37');
    expect(rows()[0]?.querySelector('[data-message]')?.children).toHaveLength(0);

    vi.advanceTimersByTime(1500);
    const object = { ...ANSWER.entries[0], id: 'cccc', text: '{"width":320,"ok":true}' };
    await answer('grouped', { ...ANSWER, entries: [object] });
    expect(document.querySelector('#entries [data-message] .hl-key')?.textContent).toBe('"width"');

    await type('#expression', 'JSON.stringify({ a: 1 })');
    const editor = $('#expression').closest('[data-highlight]') as HTMLElement;
    expect(editor.dataset.highlight).toBe('on');
    expect(editor.querySelector('pre .hl-fn')?.textContent).toBe('stringify');
    $('#expression').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    await answer('exec', { results: [{ ok: true, type: 'object', text: '{"a":1}', views: [1] }] });
    expect($('#results .expression').textContent).toBe('> JSON.stringify({ a: 1 })');
    expect($('#results .expression .hl-number').textContent).toBe('1');
    expect($('#results .outcome .hl-key').textContent).toBe('"a"');
  });

  it('breaks the command line with Shift+Enter instead of running it', async () => {
    await type('#expression', 'a');
    $('#expression').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', shiftKey: true, bubbles: true }));
    await settle();
    expect(posted.some((m) => m.command === 'exec')).toBe(false);
  });

  // Review of RB-080-65: a string value is text, not JavaScript.
  it('shows a string result as text, a function as JavaScript', async () => {
    await runCommand('x');
    await answer('exec', { results: [
      { ok: true, type: 'string', text: '"don\'t do this"', views: [1] },
      { ok: true, type: 'function', text: 'function f()', views: [2] },
    ] });
    const outcomes = document.querySelectorAll('#results .outcome');
    expect(outcomes[0]?.textContent).toContain('"don\'t do this"');
    expect(outcomes[0]?.querySelectorAll('[class^="hl-"]')).toHaveLength(0);
    expect(outcomes[1]?.querySelector('.hl-keyword')?.textContent).toBe('function');
  });

  it('shows code in one colour when ui.syntaxHighlight is off', async () => {
    rows()[1]?.click();
    await settle();
    await host({ type: 'settings', strings: STRINGS, settings: { ...SETTINGS, 'ui.syntaxHighlight': false } });
    const stack = stackOf('bbbb');
    expect(stack?.textContent).toBe('at http://x.test/console.html:18:37');
    expect(stack?.children).toHaveLength(0);
    expect(($('#expression').closest('[data-highlight]') as HTMLElement).dataset.highlight).toBe('off');
  });

  it('draws only the rows in sight of a long list', async () => {
    vi.advanceTimersByTime(1500);
    const many = Array.from({ length: 3000 }, (_, at) => ({ ...ANSWER.entries[0], id: 'e' + at, text: 'message ' + at }));
    await answer('grouped', { ...ANSWER, total: 3000, shown: 3000, entries: many });
    expect(rows().length).toBeGreaterThan(0);
    expect(rows().length).toBeLessThan(100);
  });

  it('ignores messages that do not come from the panels window', async () => {
    const before = posted.length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'settings', strings: {} }, source: null }));
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { cleared: true } }, source: null }));
    await settle();
    expect(posted.length).toBe(before);
  });
});
