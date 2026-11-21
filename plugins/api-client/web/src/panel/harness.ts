// The API client panel in its frame, for the panel's tests (RB-096-01, RB-096-11): mounted as the
// panel frame mounts it; the panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). Not part of the bundle.
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
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.api-client.', name: 'API client panel' }).unmount;
  await fromHost({ type: 'settings', strings, settings: {} });
  await reply('state', state);
  await reply('history', { entries: [] });
  await reply('variables', VARIABLES);
}

export function stop(): void {
  unmount();
  vi.restoreAllMocks();
}

/** Chooses tab `index` of the tab row `#id`, as a click does. */
export async function chooseTab(id: string, index: number): Promise<void> {
  const tab = document.querySelectorAll<HTMLElement>(`#${id} [role="tab"]`)[index];
  tab?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  await settle();
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
