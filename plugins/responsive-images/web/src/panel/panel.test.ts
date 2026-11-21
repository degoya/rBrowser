// The responsive images panel in its frame (RB-093-02): mounted as the panel frame mounts it; the
// panels window is played by spying on postMessage and dispatching `message` events from
// window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import { ANSWER, STRINGS as TEXTS } from './fixtures';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

const STRINGS = {
  ...TEXTS,
  'plugins.panel.call_failed': 'The plugin could not do that.',
  'plugins.error.not_available': 'This view cannot be checked in {engine}.',
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;
const answered = new Set<number>();

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const all = (selector: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(selector)];
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the calls of `name` (a tool or a command) not answered yet. */
async function reply(name: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && (m.tool ?? m.command) === name && !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

const tabButton = (index: number): HTMLElement => all('#tabs [role="tab"]')[index] as HTMLElement;
const openTab = async (index: number): Promise<void> => {
  tabButton(index).dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  await settle();
};
const visibleRows = (): HTMLElement[] => all('[role="tabpanel"]:not([hidden]) tbody tr');

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.responsive-images.', name: 'responsive images panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'plugins.panelMessageMaxBytes': 1024 * 1024 } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('the responsive images panel', () => {
  it('checks every view at once, within its message limit, and says so', () => {
    expect($('#status')?.textContent).toBe('Checking the images of every view...');
    expect(posted.find((m) => m.tool === 'check')?.args).toEqual({ budget: 1024 * 1024 - 5536 });
  });

  it('shows a tab per view with its density and problems, and a table of its images', async () => {
    await reply('check', ANSWER);
    expect(all('#tabs [role="tab"]').map((one) => one.textContent?.trim())).toEqual(['Mobile · 3x4', 'Desktop · 1x4', 'Firefox · 1x']);
    expect($('#status')?.textContent).toBe('0 errors, 8 warnings, 0 notes');
    const rows = visibleRows();
    expect(rows).toHaveLength(5);
    expect(rows[1]?.textContent).toContain('large-1800.jpg');
    expect(rows[1]?.textContent).toContain('900 × 600');
    expect(rows[1]?.querySelector('[data-issue]')?.textContent).toContain('2× the 900 × 600 px that 300 × 200 CSS px need at 3x');
    expect(rows[0]?.textContent).toContain('The LCP image loads lazily');
    expect($('[role="tabpanel"]:not([hidden]) [data-page]')?.textContent).toBe('https://example.test/ at 390 × 844 CSS pixels, 3x');
  });

  it('keeps only the images with problems when asked', async () => {
    await reply('check', ANSWER);
    ($('#only-problems') as HTMLButtonElement).click();
    await settle();
    expect(visibleRows()).toHaveLength(4);
  });

  it('says a view it could not check', async () => {
    await reply('check', ANSWER);
    await openTab(2);
    expect($('[role="tabpanel"]:not([hidden]) [data-failed]')?.textContent).toBe('This view could not be checked: This view cannot be checked in firefox.');
  });

  it('marks a hovered or focused image in every view and clears on leave', async () => {
    await reply('check', ANSWER);
    vi.useFakeTimers();
    const name = $('[role="tabpanel"]:not([hidden]) [data-image]') as HTMLButtonElement;
    expect(name.getAttribute('aria-label')).toBe('Show hero-1170.jpg in every view');
    name.dispatchEvent(new FocusEvent('focus'));
    await vi.advanceTimersByTimeAsync(200);
    const mark = posted.find((m) => m.tool === 'mark');
    expect(mark?.args).toEqual({ targets: [{ view: 1, selector: '#hero' }, { view: 2, selector: '#hero' }] });
    vi.useRealTimers();
    await reply('mark', { marked: [] });
    vi.useFakeTimers();
    name.dispatchEvent(new FocusEvent('blur'));
    await vi.advanceTimersByTimeAsync(200);
    expect(posted.filter((m) => m.tool === 'mark').at(-1)?.args).toEqual({ clear: true });
  });

  it('sends the findings to the findings panel without reading twice', async () => {
    await reply('check', ANSWER);
    ($('#report') as HTMLButtonElement).click();
    await settle();
    expect(posted.some((m) => m.command === 'check')).toBe(true);
    // The command's post arrives while it runs: no second read.
    await fromHost({ type: 'event', message: { type: 'checked' } });
    await reply('check', { findings: 8 });
    expect($('#status')?.textContent).toBe('8 findings sent to the findings panel.');
    expect(posted.filter((m) => m.tool === 'check')).toHaveLength(1);
    // A check started elsewhere (the shortcut, the context menu) reads again.
    await fromHost({ type: 'event', message: { type: 'checked' } });
    expect(posted.filter((m) => m.tool === 'check')).toHaveLength(2);
  });

  it('says a failed call in one sentence', async () => {
    await reply('check', 'plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The plugin could not do that.');
  });
});
