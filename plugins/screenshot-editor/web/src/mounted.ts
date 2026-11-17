// The screenshot editor panel mounted on Nuxt UI for its tests, the panels window played by
// spying on postMessage and dispatching `message` events from window.parent (the test window
// itself); the English texts as the window sends them.
import { vi } from 'vitest';
import { nextTick } from 'vue';
import { settle as rounds } from '../../../web-common/src/harness';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import en from '../../locales/en.json';
import App from './panel/App.vue';
import { fakeContext } from './support';

/** Lets the promises and Vue's updates settle (the editor draws in several steps). */
export async function settle(): Promise<void> {
  await rounds(60);
  await nextTick();
}

function flatten(value: unknown, prefix: string, into: Record<string, string>): Record<string, string> {
  if (typeof value === 'string') into[prefix] = value;
  else if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) flatten(child, prefix ? `${prefix}.${key}` : key, into);
  }
  return into;
}

/** The plugin's English texts by full key. */
export const strings = (): Record<string, string> => flatten(en, '', {});

export type Sent = { type: string; id?: number; tool?: string; args?: Record<string, unknown>; dirty?: boolean };

/** A message of the panels window. */
export function fromHost(data: unknown): void {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
}

/** The panel mounted on `#app`, its posts recorded in `sent`; answers what takes it down. */
export function mountEditor(sent: Sent[]): () => void {
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    sent.push(message as Sent);
  });
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => fakeContext() as never);
  vi.stubGlobal('createImageBitmap', vi.fn(async () => ({ width: 100, height: 80 })));
  return mountPanel(App, { prefix: 'plugin.screenshot-editor.', name: 'screenshot-editor panel' }).unmount;
}

export const SHOT = '.rbrowser/out/screenshots/2026-11-12_10-00-00/mobile.png';
export const FIRST = { image: SHOT, bytes: 3, offset: 0, next: null, hash: 'h1', data: btoa('png'), width: 100, height: 80, annotations: null };

/** The calls the panel posted. */
export const callsOf = (sent: Sent[]): Sent[] => sent.filter((message) => message.type === 'call');
/** The last call of `tool`. */
export const lastCall = (sent: Sent[], tool: string): Sent | undefined => callsOf(sent).filter((call) => call.tool === tool).at(-1);

/** Answers the last call of `tool` with `value` (or fails it with the code `value`). */
export async function answer(sent: Sent[], tool: string, value: unknown, ok = true): Promise<void> {
  const call = lastCall(sent, tool);
  fromHost(ok ? { type: 'result', id: call?.id, ok, value } : { type: 'result', id: call?.id, ok, error: value });
  await settle();
}

export function el<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector(selector);
  if (!found) throw new Error(`no ${selector}`);
  return found as T;
}

/** A key pressed on `target`; Vue's updates follow with `settle()`. */
export function key(target: EventTarget, name: string, init: KeyboardEventInit = {}): void {
  target.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...init }));
}

export const canvas = (): HTMLCanvasElement => el<HTMLCanvasElement>('#canvas');
/** The chosen tool, by its pressed button. */
export const pressed = (): string | null | undefined => document.querySelector('[data-tool][aria-pressed="true"]')?.getAttribute('data-tool');
/** The names of the annotations in the list. */
export const listed = (): (string | null)[] => [...document.querySelectorAll('#annotations li [data-name]')].map((row) => row.textContent);
/** The options shown beside the tools. */
export const visible = (): (string | undefined)[] => [...document.querySelectorAll<HTMLElement>('#context [data-option]')].map((node) => node.dataset.option);

/** Draws a rectangle by keyboard: Enter, 30 px right, 20 px down, Enter. */
export async function keyboardRect(): Promise<void> {
  canvas().focus();
  key(canvas(), 'r');
  key(canvas(), 'Enter');
  for (let i = 0; i < 3; i += 1) key(canvas(), 'ArrowRight', { shiftKey: true });
  for (let i = 0; i < 2; i += 1) key(canvas(), 'ArrowDown', { shiftKey: true });
  key(canvas(), 'Enter');
  await settle();
}

/** Draws a crop frame by keyboard: C, back 30 px left and 20 px up, Enter, 30 px right, 20 px
 * down, Enter - a draft, the same from wherever the cursor was. */
export async function keyboardCrop(): Promise<void> {
  canvas().focus();
  key(canvas(), 'c');
  for (let i = 0; i < 3; i += 1) key(canvas(), 'ArrowLeft', { shiftKey: true });
  for (let i = 0; i < 2; i += 1) key(canvas(), 'ArrowUp', { shiftKey: true });
  key(canvas(), 'Enter');
  for (let i = 0; i < 3; i += 1) key(canvas(), 'ArrowRight', { shiftKey: true });
  for (let i = 0; i < 2; i += 1) key(canvas(), 'ArrowDown', { shiftKey: true });
  key(canvas(), 'Enter');
  await settle();
}

/** The panel with its texts, the settings' colour and line width, the list and the newest
 * screenshot loaded. */
export async function openEditor(sent: Sent[]): Promise<() => void> {
  const stop = mountEditor(sent);
  fromHost({
    type: 'settings',
    strings: strings(),
    settings: { 'screenshot-editor.color': '#00ff00', 'screenshot-editor.strokeWidth': 6 },
  });
  await settle();
  await answer(sent, 'list', { images: [SHOT, '.rbrowser/out/checks/run/diff.png'], truncated: false });
  await answer(sent, 'load', FIRST);
  return stop;
}
