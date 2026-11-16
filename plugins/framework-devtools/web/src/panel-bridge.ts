// The panels' side of the plugin bridge (docs/plugins.md, "Page scripts and panels"): a panel
// runs in a sandboxed frame of the plugin panels window and talks to `window.parent` with
// `postMessage` only - `call` and `settings` out, `result`, `event` and `settings` in. The
// texts come from the plugin's locales in the `settings` message.

import { isObject } from '../../../web-common/src/is-object';

/** The longest wait for a call's answer, in ms (the shell's IPC timeout is 10 s). */
const CALL_TIMEOUT_MS = 12_000;

/** A panel's bridge. */
export interface PanelBridge {
  /** Runs one of the plugin's own tools; rejects with an `Error` whose message is the code. */
  call(tool: string, args?: unknown): Promise<unknown>;
  /** A text of the plugin's locales, `{name}` placeholders filled; the key itself before the
   *  texts arrived. */
  text(key: string, params?: Record<string, string | number>): string;
  /** The effective value of one of the plugin's settings (`<plugin-id>.<key>`); undefined
   *  before the settings arrived or for a key the schema does not declare. */
  setting(key: string): unknown;
  /** Called whenever the texts and settings arrive. */
  onSettings(listener: () => void): () => void;
  /** Called for every event of the plugin's page scripts (`view` the page's view) and posts. */
  onEvent(listener: (view: number | null, message: unknown) => void): () => void;
  /** Asks the window for the texts and settings. */
  requestSettings(): void;
}

interface Pending {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}


/** The bridge of the panel in `win` for the plugin whose locale keys start with `prefix`
 *  (`plugin.<id>.`). */
export function createPanelBridge(win: Window, prefix: string): PanelBridge {
  let strings: Record<string, string> = {};
  let settings: Record<string, unknown> = {};
  let next = 1;
  const pending = new Map<number, Pending>();
  const settingsListeners = new Set<() => void>();
  const eventListeners = new Set<(view: number | null, message: unknown) => void>();

  win.addEventListener('message', (event) => {
    // Only the panels window speaks to this frame.
    if (event.source !== win.parent) return;
    const message: unknown = event.data;
    if (!isObject(message)) return;
    if (message.type === 'settings') {
      strings = isObject(message.strings) ? (message.strings as Record<string, string>) : {};
      settings = isObject(message.settings) ? message.settings : {};
      for (const listener of settingsListeners) listener();
    } else if (message.type === 'result' && typeof message.id === 'number') {
      const waiting = pending.get(message.id);
      if (!waiting) return;
      pending.delete(message.id);
      clearTimeout(waiting.timer);
      if (message.ok === true) waiting.resolve(message.value);
      else waiting.reject(new Error(String(message.error ?? 'plugins.error.host_failed')));
    } else if (message.type === 'event') {
      const view = typeof message.view === 'number' ? message.view : null;
      for (const listener of eventListeners) listener(view, message.message);
    }
  });

  return {
    call(tool, args) {
      const id = next++;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(id);
          reject(new Error('plugins.error.timed_out'));
        }, CALL_TIMEOUT_MS);
        pending.set(id, { resolve, reject, timer });
        win.parent.postMessage(args === undefined ? { type: 'call', id, tool } : { type: 'call', id, tool, args }, '*');
      });
    },
    text(key, params = {}) {
      const template = strings[prefix + key] ?? key;
      return template.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? ''));
    },
    setting(key) {
      return settings[key];
    },
    onSettings(listener) {
      settingsListeners.add(listener);
      return () => settingsListeners.delete(listener);
    },
    onEvent(listener) {
      eventListeners.add(listener);
      return () => eventListeners.delete(listener);
    },
    requestSettings() {
      win.parent.postMessage({ type: 'settings' }, '*');
    },
  };
}
