// The API client panel in its frame, for the panel's tests (RB-096-01, RB-096-11): mounted as the
// panel frame mounts it; the panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). Not part of the bundle.
//
// The frame is played as Chromium runs it (RB-096-13): postMessage clones its message - a Vue
// reactive value in it throws `DataCloneError` -, and the sandbox without `allow-forms` blocks a
// form's submission before any `submit` handler runs (`blocked` counts them).
import { vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';

export type Op = { op?: string } & Record<string, unknown>;
export type Posted = { type?: string; id?: number; tool?: string; target?: string; action?: string; key?: string; args?: { op?: Op } };

export const STATE = {
  environment: 'staging',
  environments: ['local', 'staging'],
  variables: [{ name: 'token', secret: true }],
  requests: [{ id: 'users/me.http', name: 'Me', method: 'GET', url: '{{baseUrl}}/me' }],
  folders: ['users'],
  allowedHosts: [],
};

export const VARIABLES = {
  file: { $shared: { user: 'ada' }, staging: { path: '/v1' } },
  private: { staging: ['token'] },
  baseUrls: { staging: 'https://staging.example.com', local: 'http://localhost:3000' },
};

export const SAVED = (name: string, url: string, method = 'GET') => ({
  name,
  method,
  url,
  headers: [],
  body: { kind: 'none' },
  auth: { type: 'none' },
  options: { timeoutMs: 30000, followRedirects: true, verifyTls: true },
});

export const posted: Posted[] = [];
const answered = new Set<number>();
let unmount: () => void = () => undefined;
/** The form submissions the sandbox blocked. */
export let blocked = 0;

/** `sandbox="allow-scripts"`: a submission ends before its `submit` event reaches the form. */
function sandboxed(event: Event): void {
  blocked += 1;
  event.preventDefault();
  event.stopImmediatePropagation();
}

export const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);

export const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

export async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

export const calls = (op: string): Posted[] => posted.filter((m) => m.type === 'call' && m.tool === 'panel' && m.args?.op?.op === op);

/** Answers the calls of op `op` not answered yet. */
export async function reply(op: string, value: unknown, failure?: { error: string; params: Record<string, string> }): Promise<void> {
  for (const message of calls(op).filter((m) => !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(
      failure
        ? { type: 'result', id: message.id, ok: false, error: failure.error, params: failure.params }
        : { type: 'result', id: message.id, ok: true, value },
    );
  }
}

/** Mounts the panel and answers its first `state`, `history` and `variables`. */
export async function start(strings: Record<string, string>, state: unknown = STATE): Promise<void> {
  posted.length = 0;
  answered.clear();
  blocked = 0;
  document.body.innerHTML = '<div id="app"></div>';
  window.addEventListener('submit', sandboxed, true);
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(structuredClone(message) as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.api-client.', name: 'API client panel' }).unmount;
  await fromHost({ type: 'settings', strings, settings: {} });
  await reply('state', state);
  await reply('history', { entries: [] });
  await reply('variables', VARIABLES);
}

export function stop(): void {
  unmount();
  window.removeEventListener('submit', sandboxed, true);
  vi.restoreAllMocks();
}

/** Presses Enter in the input of `selector`. */
export async function enter(selector: string): Promise<void> {
  const field = ($(`${selector} input`) ?? $(selector)) as HTMLInputElement;
  field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  await settle();
}

/** Chooses tab `index` of the tab row `#id`, as a click does. */
export async function chooseTab(id: string, index: number): Promise<void> {
  const tab = document.querySelectorAll<HTMLElement>(`#${id} [role="tab"]`)[index];
  tab?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  await settle();
}

/** The label of the side column's tab shown. */
export function selectedSide(): string | undefined {
  const tab = document.querySelector<HTMLElement>('#side-tabs [role="tab"][aria-selected="true"]');
  return tab?.textContent?.trim();
}

/** Types `text` into the input of `selector` (a Nuxt UI input or the input itself). */
export async function type(selector: string, text: string): Promise<void> {
  const field = ($(`${selector} input`) ?? $(`${selector} textarea`) ?? $(selector)) as HTMLInputElement;
  field.value = text;
  field.dispatchEvent(new Event('input'));
  await settle();
}

/** The value of the input of `selector`. */
export function valueOf(selector: string): string | undefined {
  return (($(`${selector} input`) ?? $(selector)) as HTMLInputElement | null)?.value;
}
