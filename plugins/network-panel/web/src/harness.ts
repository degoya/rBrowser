// The network panel in a window of its own, for its tests (RB-080-84): the panels window is
// played by spying on postMessage and dispatching `message` events from window.parent (the test
// window itself). The clocks are fake, so a poll runs only when a test advances them.
import { vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';

export type Posted = { type: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

export const posted: Posted[] = [];
let unmount: () => void = () => undefined;

export async function settle(): Promise<void> {
  for (let i = 0; i < 10; i++) await Promise.resolve();
  await nextTick();
  // The virtualizer draws on the next frame.
  vi.advanceTimersToNextFrame();
  for (let i = 0; i < 4; i++) await Promise.resolve();
  await nextTick();
}

export async function host(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window.parent as Window }));
  await settle();
}

/** Answers the newest call to `name` with `value` (or fails it with `value` as the code). */
export async function answer(name: string, value: unknown, ok = true): Promise<void> {
  const call = [...posted].reverse().find((m) => m.type === 'call' && (m.tool === name || m.command === name));
  if (!call) throw new Error(`no call to ${name}`);
  await host(ok ? { type: 'result', id: call.id, ok: true, value } : { type: 'result', id: call.id, ok: false, error: value });
}

export const $ = <T extends HTMLElement = HTMLElement>(selector: string): T => document.querySelector<T>(selector) as T;

/** Types `value` into the field `selector` (an input or a textarea) as a person would. */
export async function type(selector: string, value: string): Promise<void> {
  const field = $<HTMLInputElement>(selector);
  field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
}

/** Opens tab `name` as a click does. */
export async function openTab(name: string): Promise<void> {
  const trigger = $(`#tabs [role="tab"][id$="-trigger-${name}"]`);
  trigger.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  trigger.click();
  await settle();
}

/** Whether the tab panel holding `selector` is shown. */
export const shownTab = (selector: string): boolean => !($(selector).closest('[role="tabpanel"]') as HTMLElement).hidden;

/** Mounts the panel and sends it `strings`. */
export async function start(strings: Record<string, string>): Promise<void> {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date', 'requestAnimationFrame', 'cancelAnimationFrame'] });
  posted.length = 0;
  // happy-dom lays nothing out: every element answers a frame-sized box, so the request list's
  // virtualizer sees the room it would have in the panel.
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(900);
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (this: HTMLElement) {
    return this.tagName === 'TR' ? 33 : 0;
  });
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window.parent, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.network-panel.', name: 'network panel' }).unmount;
  await host({ type: 'settings', strings, settings: {} });
}

export function stop(): void {
  unmount();
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
}
