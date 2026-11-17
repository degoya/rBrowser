// The simulators picker in a window of its own, on Nuxt UI since RB-080-86: the bridge messages it
// sends and how it draws the plugin's answers. The panels window is played by spying on
// postMessage and dispatching `message` events from window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import en from '../../locales/en.json';
import App from './panel/App.vue';
import { PREFIX } from './panel/model';

type Sent = { type: string; id?: number; tool?: string; args?: unknown };
let sent: Sent[] = [];
let unmount: () => void = () => undefined;

function flatten(value: unknown, prefix: string, into: Record<string, string>): Record<string, string> {
  if (typeof value === 'string') into[prefix] = value;
  else if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) flatten(child, prefix ? `${prefix}.${key}` : key, into);
  }
  return into;
}

const fromHost = (data: unknown): void => {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
};
const asked = (): number => sent.filter((message) => message.type === 'settings').length;
const calls = (): Sent[] => sent.filter((message) => message.type === 'call');
// Fake timers (the texts' requests) hold back happy-dom's events, so only the tests before the
// texts use them; the others settle on a real timer.
const settle = async (): Promise<void> => {
  if (vi.isFakeTimers()) for (let round = 0; round < 5; round += 1) await Promise.resolve();
  else await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};
const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const status = (): string | null | undefined => $('#status')?.textContent;

function mount(): void {
  unmount = mountPanel(App, { prefix: PREFIX, name: 'simulators panel' }).unmount;
}

beforeEach(() => {
  sent = [];
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    sent.push(message as Sent);
  });
  document.body.innerHTML = '<div id="app"></div>';
  delete document.body.dataset.texts;
});

afterEach(() => {
  unmount();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

// RB-080-59: the picker drew its texts at once, before the plugin panels window answered its one
// `settings` request - as keys, and for good when the answer never came. It shows nothing before
// its texts arrived, asks again until they do, and logs a key its locale lacks.
describe('the picker before its texts', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mount();
  });

  it('shows nothing and starts nothing until its texts arrived', async () => {
    expect(document.body.dataset.texts).toBe('waiting');
    expect(document.body.textContent).not.toMatch(/\bpanel\.[a-z_]+/);
    expect(calls()).toHaveLength(0);
    // Review of RB-080-59: the window's focus refreshed (a `set` call) before the texts.
    window.dispatchEvent(new Event('focus'));
    await settle();
    expect(calls()).toHaveLength(0);
    fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
    await settle();
    expect(document.body.dataset.texts).toBeUndefined();
    expect($('#title')?.textContent).toBe('Simulators');
    expect(document.body.textContent).not.toMatch(/\bpanel\.[a-z_]+/);
    expect(calls()[0]).toMatchObject({ tool: 'set', args: {} });
  });

  it('asks the panels window again until its texts arrive', async () => {
    expect(asked()).toBe(1);
    await vi.advanceTimersByTimeAsync(2000);
    expect(asked()).toBe(3);
    fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
    await vi.advanceTimersByTimeAsync(5000);
    expect(asked()).toBe(3);
  });

  it('gives up after ten requests and says so in the log', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(asked()).toBe(10);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('simulators'));
    expect($('[data-texts-notice]')?.textContent).toContain("The panel's texts did not arrive.");
  });

  it('logs a key its locale lacks, once', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const strings = flatten(en, '', {});
    delete strings['plugin.simulators.panel.strength'];
    fromHost({ type: 'settings', strings, settings: {} });
    await settle();
    fromHost({ type: 'settings', strings, settings: {} });
    await settle();
    const logged = warn.mock.calls.filter(([line]) => String(line).includes('plugin.simulators.panel.strength'));
    expect(logged).toHaveLength(1);
  });
});

const STATE = {
  active: 2,
  views: [
    { view: 1, label: 'Desktop', engine: 'chromium', type: 'none', severity: null, mode: 'off' },
    { view: 2, label: 'Mobile', engine: 'chromium', type: 'protanomaly', severity: 0.4, mode: 'script' },
    { view: 3, label: 'Tablet', engine: 'chromium', type: 'deuteranopia', severity: null, mode: 'native' },
  ],
  catalog: [
    { id: 'protanomaly', group: 'color', severity: 0.6, native: false },
    { id: 'deuteranopia', group: 'color', severity: null, native: true },
    { id: 'dimmed', group: 'environment', severity: 0.6, native: false },
  ],
};
const answer = (id: number | undefined, value: unknown): void => fromHost({ type: 'result', id, ok: true, value });

/** The texts, then STATE as the answer to the first `set`. */
async function ready(): Promise<void> {
  fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
  await settle();
  answer(calls()[0]?.id, STATE);
  await settle();
}

const radio = (type: string): HTMLElement => $(`[role="radio"][value="${type}"]`) as HTMLElement;
async function click(target: HTMLElement, shiftKey = false): Promise<void> {
  target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1, shiftKey }));
  await settle();
}
async function key(target: HTMLElement, name: string, shiftKey = false): Promise<void> {
  target.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, shiftKey }));
  await settle();
}

describe('the picker', () => {
  beforeEach(mount);

  it('lists Off and the simulations in their groups, the active view and every view in words', async () => {
    await ready();
    const groups = [...document.querySelectorAll<HTMLElement>('#groups [data-group]')];
    expect(groups.map((group) => group.querySelector('legend')?.textContent ?? '')).toEqual(['', 'Colour vision', 'Vision impairments', 'Surroundings']);
    expect([...groups[1]!.querySelectorAll('[role="radio"]')].map((one) => one.getAttribute('value'))).toEqual(['protanomaly', 'deuteranopia']);
    expect(groups[1]?.textContent).toContain('Weak red cones: reds look duller and closer to greens.');
    expect(radio('protanomaly').getAttribute('aria-checked')).toBe('true');
    expect($('#active')?.textContent).toBe('Active view: Mobile');
    const rows = [...document.querySelectorAll<HTMLElement>('#views li')];
    expect(rows.map((row) => row.textContent)).toEqual([
      'Desktop: Off (off)',
      'Mobile: Protanomaly (drawn by the page script)',
      'Tablet: Deuteranopia (drawn by Chromium)',
    ]);
    expect(rows.map((row) => row.getAttribute('aria-current'))).toEqual([null, 'true', null]);
    expect(($('[role="slider"]') as HTMLElement).getAttribute('aria-valuenow')).toBe('40');
    expect(document.body.textContent).toContain('40 %');
  });

  it('has Refresh as a header icon button with its name', async () => {
    await ready();
    const refresh = $('#refresh') as HTMLButtonElement;
    expect(refresh.getAttribute('aria-label')).toBe("Read every view's simulation again");
    expect(refresh.textContent).toBe('');
    const before = calls().length;
    await click(refresh);
    expect(calls().slice(before)).toEqual([expect.objectContaining({ tool: 'set', args: {} })]);
  });

  it('applies a choice to the active view at once, without a strength the slider did not get', async () => {
    await ready();
    await click(radio('dimmed'));
    const call = calls().at(-1);
    expect(call).toMatchObject({ tool: 'set', args: { type: 'dimmed' } });
    expect(call?.args).toEqual({ type: 'dimmed' });
    answer(call?.id, { ...STATE, views: STATE.views.map((view) => (view.view === 2 ? { ...view, type: 'dimmed', severity: 0.6 } : view)) });
    await settle();
    expect(status()).toBe('Dimmed screen in Mobile.');
  });

  it('applies a choice to every view with Shift, by click and by Shift+Enter', async () => {
    await ready();
    await click(radio('deuteranopia'), true);
    expect(calls().at(-1)?.args).toEqual({ type: 'deuteranopia', target: 'all' });
    answer(calls().at(-1)?.id, STATE);
    await settle();
    expect(status()).toBe('Deuteranopia in every view.');
    await key(radio('protanomaly'), 'Enter', true);
    expect(calls().at(-1)?.args).toEqual({ type: 'protanomaly', target: 'all' });
    await key(radio('protanomaly'), 'Enter');
    expect(calls().at(-1)?.args).toEqual({ type: 'protanomaly' });
  });

  it('sends the strength once the slider was moved', async () => {
    await ready();
    await key($('[role="slider"]') as HTMLElement, 'ArrowRight');
    expect(calls().at(-1)?.args).toEqual({ type: 'protanomaly', severity: 0.45 });
  });

  it('says a failed choice in one sentence, never the code', async () => {
    await ready();
    await click(radio('dimmed'));
    fromHost({ type: 'result', id: calls().at(-1)?.id, ok: false, error: 'plugin.simulators.errors.no_view' });
    await settle();
    expect(status()).toBe('There is no active view to simulate in. Click into a view and try again.');
    expect($('#status')?.dataset.tone).toBe('error');
    await click(radio('protanomaly'));
    fromHost({ type: 'result', id: calls().at(-1)?.id, ok: false, error: 'plugins.error.busy' });
    await settle();
    expect(status()).toBe('The plugin could not do that.');
  });

  it('follows the state the component posts', async () => {
    await ready();
    fromHost({ type: 'event', message: { type: 'state', ...STATE, active: 3 } });
    await settle();
    expect($('#active')?.textContent).toBe('Active view: Tablet');
    expect(radio('deuteranopia').getAttribute('aria-checked')).toBe('true');
    expect(($('[role="slider"]') as HTMLElement).hasAttribute('data-disabled')).toBe(true);
  });
});

// RB-080-75: "Apply to every view" changed only how the next choice applied; the simulation
// already chosen stayed in its one view until the effect or the strength changed again.
describe('apply to every view', () => {
  const box = (): HTMLElement => $('#all') as HTMLElement;

  beforeEach(async () => {
    mount();
    await ready();
  });

  it('switched on, puts the active view\'s simulation into every view at once', async () => {
    expect(box().getAttribute('role')).toBe('switch');
    await click(box());
    const call = calls().at(-1);
    expect(call).toMatchObject({ tool: 'set', args: { type: 'protanomaly', severity: 0.4, target: 'all' } });
    answer(call?.id, { ...STATE, views: STATE.views.map((view) => ({ ...view, type: 'protanomaly', severity: 0.4, mode: 'script' })) });
    await settle();
    expect(status()).toBe('Protanomaly in every view.');
  });

  it('switched off, keeps it in the active view and takes it out of the views it was put into', async () => {
    await click(box());
    const shared = { ...STATE, views: STATE.views.map((view) => (view.view === 3 ? view : { ...view, type: 'protanomaly', severity: 0.4, mode: 'script' })) };
    answer(calls().at(-1)?.id, shared);
    await settle();
    const before = calls().length;
    await click(box());
    const off = calls().slice(before);
    // View 1 showed the same simulation and loses it; view 3 keeps its own, view 2 is the active one.
    expect(off).toHaveLength(1);
    expect(off[0]).toMatchObject({ tool: 'set', args: { type: 'none', target: 1 } });
    answer(off[0]?.id, STATE);
    await settle();
    expect(status()).toBe('Protanomaly stays in Mobile; removed from the other views.');
  });
});
