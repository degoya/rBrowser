import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fromHost, mountEditor, settle, strings, type Sent } from './mounted';

// RB-080-59: the panel showed its raw keys (`panels.editor`, `panel.open`, ...) when the panels
// window did not answer its one `settings` request. It shows nothing before its texts
// arrived, asks again until they do, and logs a key its locale lacks.

let sent: Sent[] = [];
let stop: () => void = () => undefined;

const asked = (): number => sent.filter((message) => message.type === 'settings').length;
const calls = (): Sent[] => sent.filter((message) => message.type === 'call');
/** Every text the page shows, its controls' names included. */
const shown = (): string => [document.body.textContent ?? '', ...[...document.querySelectorAll('[aria-label]')].map((node) => node.getAttribute('aria-label'))].join(' ');

beforeEach(() => {
  vi.useFakeTimers();
  sent = [];
  stop = mountEditor(sent);
});

afterEach(() => {
  stop();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('the panel before its texts', () => {
  it('shows nothing and starts nothing until its texts arrived', async () => {
    expect(document.body.dataset.texts).toBe('waiting');
    expect(shown()).not.toMatch(/\bpanels?\.[a-z_]+/);
    expect(calls()).toHaveLength(0);
    fromHost({ type: 'event', message: { open: '.rbrowser/out/screenshots/a.png' } });
    await settle();
    fromHost({ type: 'settings', strings: strings(), settings: {} });
    await settle();
    expect(document.body.dataset.texts).toBeUndefined();
    expect(document.getElementById('title')?.textContent).toBe('Screenshot editor');
    expect(shown()).not.toMatch(/\bpanels?\.[a-z_]+/);
    // Review of RB-080-59: one list and one load - the image a command asked for before the texts
    // were there, not the newest racing it.
    expect(calls().map((call) => call.tool)).toEqual(['list', 'load']);
    expect(calls().at(-1)?.args).toEqual({ image: '.rbrowser/out/screenshots/a.png' });
  });

  it('asks the panels window again until its texts arrive', async () => {
    expect(asked()).toBe(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(asked()).toBe(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(asked()).toBe(3);
    fromHost({ type: 'settings', strings: strings(), settings: {} });
    await vi.advanceTimersByTimeAsync(5000);
    expect(asked()).toBe(3);
  });

  it('gives up after ten requests and says so in the log', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(asked()).toBe(10);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('screenshot-editor'));
    expect(document.querySelector('[data-texts-notice]')?.textContent).toContain("The panel's texts did not arrive.");
  });

  it('logs a key its locale lacks, once', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const texts = strings();
    delete texts['plugin.screenshot-editor.panel.open'];
    fromHost({ type: 'settings', strings: texts, settings: {} });
    await settle();
    const logged = warn.mock.calls.filter(([line]) => String(line).includes('plugin.screenshot-editor.panel.open'));
    expect(logged).toHaveLength(1);
  });
});
