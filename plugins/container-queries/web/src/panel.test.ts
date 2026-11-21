// The container-queries panel in a window of its own: the bridge messages it sends and how it
// shows the analysis. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself).
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown> };

const here = dirname(fileURLToPath(import.meta.url));
const locale = JSON.parse(readFileSync(join(here, '..', '..', 'locales', 'en.json'), 'utf8')) as Record<string, unknown>;

/** The plugin's texts as the host sends them: every key flat. */
function flat(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, inner]) => ({ ...all, ...flat(inner, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}

const STRINGS = { ...flat(locale), 'plugins.error.busy': 'The plugin is busy.', 'plugins.error.host_failed': 'The view did not answer.' };

const card = (inline: number, matching: number[], resized: boolean, view: number, label: string) => ({
  view,
  label,
  breakpoint: label.toLowerCase(),
  inline: { size: inline, below: inline >= 500 ? 500 : 300, above: inline >= 500 ? null : 500 },
  block: { size: 300, below: null, above: null },
  matching,
  resized,
});

const ANALYSIS = {
  conditions: [
    { name: 'card', query: '(min-width: 500px)', supported: true, bounds: [{ axis: 'inline', op: '>=', value: '500px', px: 500 }] },
    { name: 'card', query: '(max-width: 300px)', supported: true, bounds: [{ axis: 'inline', op: '<=', value: '300px', px: 300 }] },
    {
      name: 'shell',
      query: '(400px <= width < 900px)',
      supported: true,
      bounds: [
        { axis: 'inline', op: '>=', value: '400px', px: 400 },
        { axis: 'inline', op: '<', value: '900px', px: 900 },
      ],
    },
    { name: '', query: '(min-height: 200px)', supported: true, bounds: [{ axis: 'block', op: '>=', value: '200px', px: 200 }] },
  ],
  containers: [
    { path: 'html > body > main', selector: 'main.shell', names: ['shell'], type: 'inline-size', conditions: [2], views: [], switches: [] },
    {
      path: 'html > body > main > article',
      selector: 'article#hero',
      names: ['card'],
      type: 'inline-size',
      conditions: [0, 1],
      views: [card(280, [1], false, 1, 'Phone'), card(400, [], true, 2, 'Desktop')],
      switches: [{ from: 1, to: 2, gained: [], lost: [1] }],
    },
    { path: 'aside', selector: 'aside.side', names: [], type: 'size', conditions: [3], views: [], switches: [] },
  ],
  views: [
    { view: 1, label: 'Phone', breakpoint: 'phone', width: 375, containers: 3, unreadableSheets: 0, truncated: false },
    { view: 2, label: 'Desktop', breakpoint: 'desktop', width: 1280, containers: 3, unreadableSheets: 1, truncated: false },
  ],
  overlay: true,
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const one = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const all = (selector: string): HTMLElement[] => Array.from(document.querySelectorAll<HTMLElement>(selector));
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

const calls = (): Posted[] => posted.filter((message) => message.type === 'call');

async function answer(value: unknown, ok = true): Promise<void> {
  const call = calls().at(-1);
  await fromHost(ok ? { type: 'result', id: call?.id, ok, value } : { type: 'result', id: call?.id, ok, error: value });
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.container-queries.', name: 'container-queries panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light' } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('the container-queries panel', () => {
  it('analyzes when it opens and lists the 4 conditions and the 3 containers', async () => {
    expect(calls()[0]).toMatchObject({ tool: 'analyze', args: {} });
    expect(one('#status')?.dataset.tone).toBe('busy');
    await answer(ANALYSIS);
    expect(all('[data-condition]').map((cell) => cell.textContent)).toEqual([
      'card (min-width: 500px)',
      'card (max-width: 300px)',
      'shell (400px <= width < 900px)',
      '(min-height: 200px)',
    ]);
    expect(one('#conditions')?.textContent).toContain('≥ 400px, < 900px');
    expect(all('[data-container]')).toHaveLength(3);
    expect(one('#status')?.textContent).toContain('4 conditions, 3 containers in 2 views.');
    expect(one('#unreadable')?.textContent).toContain('1 stylesheet from another origin');
    expect(one('#overlay')?.getAttribute('aria-checked')).toBe('true');
  });

  it('shows a container dragged to 400 px without (min-width: 500px), and resets it in its view', async () => {
    await answer(ANALYSIS);
    const hero = all('[data-container]')[1] as HTMLElement;
    expect(hero.textContent).toContain('400 × 300');
    expect(hero.textContent).toContain('Resized');
    expect(hero.querySelectorAll('[data-match]')).toHaveLength(1);
    expect(hero.querySelector('[data-switches]')?.textContent).toContain('Switches between Phone and Desktop:');
    expect(hero.querySelector('[data-switches]')?.textContent).toContain('loses card (max-width: 300px).');
    (hero.querySelector('[data-reset]') as HTMLElement).click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ tool: 'resize', args: { reset: true, path: 'html > body > main > article', target: 2 } });
  });

  it('tries a width typed into a row, the keyboard way to the drag handle', async () => {
    await answer(ANALYSIS);
    const field = all('[data-container]')[1]?.querySelectorAll<HTMLInputElement>('input[data-width]')[1] as HTMLInputElement;
    expect(field.value).toBe('400');
    expect(field.getAttribute('aria-label')).toBe('Width to try in Desktop (px); Enter applies it');
    field.value = '360';
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(calls().at(-1)).toMatchObject({ tool: 'resize', args: { path: 'html > body > main > article', width: 360, target: 2 } });
  });

  it('switches the overlay and resets every width from its header', async () => {
    await answer(ANALYSIS);
    (one('#overlay') as HTMLElement).click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ tool: 'overlay', args: { on: false } });
    await answer({ on: false, target: 'all' });
    (one('#reset-all') as HTMLElement).click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ tool: 'resize', args: { reset: true } });
  });

  it('analyzes again after a page script or the component says a width changed', async () => {
    await answer(ANALYSIS);
    vi.useFakeTimers();
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { type: 'resized', path: 'html > body > main > article', width: 400 }, view: 2 }, source: window }));
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { type: 'refresh' } }, source: window }));
    vi.advanceTimersByTime(300);
    vi.useRealTimers();
    await settle();
    expect(calls().filter((call) => call.tool === 'analyze')).toHaveLength(2);
  });

  it('says a failed analysis as one sentence with a retry, never as an empty page', async () => {
    await answer('plugins.error.busy', false);
    expect(one('#status')?.dataset.tone).toBe('error');
    expect(one('#failed')?.textContent).toContain('The plugin is busy.');
    expect(one('#empty')).toBeNull();
    (one('#retry') as HTMLElement).click();
    await settle();
    expect(calls().filter((call) => call.tool === 'analyze')).toHaveLength(2);
  });

  it('says an empty page is empty and names a view that failed', async () => {
    await answer({
      conditions: [],
      containers: [],
      views: [{ view: 3, label: 'Firefox', breakpoint: 'd', width: 1280, containers: 0, unreadableSheets: 0, truncated: false, error: 'plugins.error.host_failed' }],
      overlay: false,
    });
    expect(one('#empty')?.textContent).toContain('This page has no container queries');
    expect(one('[data-view-error]')?.textContent?.trim()).toBe('Firefox: The view did not answer.');
  });
});
