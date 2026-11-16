import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import en from '../../locales/en.json';
import { settle as rounds } from './support';

// RB-080-59: the panel showed its raw keys (`panels.guides`, `panel.show_rulers`, ...) when the
// panels window did not answer its one `settings` request. It shows nothing before its texts
// arrived, asks again until they do, and logs a key its locale lacks. On Nuxt UI since
// RB-080-86: the texts are panel-ui's (the shared texts.js bundled into the panel).

type Sent = { type: string; id?: number; tool?: string; args?: Record<string, unknown> };
let sent: Sent[] = [];
let unmount: () => void = () => undefined;

const STATE = { visible: { rulers: true, columns: false, baseline: false }, guides: [{ id: 1, axis: 'x', unit: 'px', value: 5 }], columns: {}, baseline: { lineHeight: 8, offset: 0 } };

const settle = async (): Promise<void> => {
  await rounds();
  await nextTick();
};

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
/** Every text the page shows, its controls' names included. */
const shown = (): string => [document.body.textContent ?? '', ...[...document.querySelectorAll('[aria-label]')].map((node) => node.getAttribute('aria-label'))].join(' ');

beforeEach(() => {
  vi.useFakeTimers();
  sent = [];
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    sent.push(message as Sent);
  });
  document.body.innerHTML = '<div id="app"></div>';
  unmount = mountPanel(App, { prefix: 'plugin.rulers-guides.', name: 'rulers-guides panel' }).unmount;
});

afterEach(() => {
  unmount();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the panel before its texts', () => {
  it('shows nothing and starts nothing until its texts arrived', async () => {
    expect(document.body.dataset.texts).toBe('waiting');
    expect(shown()).not.toMatch(/\bpanels?\.[a-z_]+/);
    expect(calls()).toHaveLength(0);
    fromHost({ type: 'event', message: { state: STATE } });
    await settle();
    fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
    await settle();
    expect(document.body.dataset.texts).toBeUndefined();
    expect(document.getElementById('title')?.textContent).toBe('Rulers and guides');
    expect(shown()).not.toMatch(/\bpanels?\.[a-z_]+/);
    expect(calls()[0]).toMatchObject({ tool: 'set', args: {} });
  });

  it('asks the panels window again until its texts arrive', async () => {
    expect(asked()).toBe(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(asked()).toBe(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(asked()).toBe(3);
    fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
    await vi.advanceTimersByTimeAsync(5000);
    expect(asked()).toBe(3);
  });

  it('gives up after ten requests and says so in the log', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(asked()).toBe(10);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('rulers-guides'));
    // Review of RB-080-59: not blank for good - one line and a Retry button.
    expect(document.querySelector('[data-texts-notice]')?.textContent).toContain("The panel's texts did not arrive.");
  });

  it('logs a key its locale lacks, once', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const strings = flatten(en, '', {});
    delete strings['plugin.rulers-guides.panel.columns_off'];
    fromHost({ type: 'settings', strings, settings: {} });
    await settle();
    // The setup that names the key draws with the state.
    const call = calls().at(-1);
    fromHost({ type: 'result', id: call?.id, ok: true, value: { state: STATE, breakpoints: ['mobile'] } });
    await settle();
    expect(document.getElementById('columns-off')).not.toBeNull();
    const logged = warn.mock.calls.filter(([line]) => String(line).includes('plugin.rulers-guides.panel.columns_off'));
    expect(logged).toHaveLength(1);
  });
});
