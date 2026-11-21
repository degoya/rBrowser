// A Nuxt UI panel's link to the plugin panels window (RB-080-83), the same protocol as every
// panel (docs/plugins.md, "Panels"): it asks for its texts with `{ type: 'settings' }` through
// texts.js (`PanelTexts`, again every second until they arrive), takes `settings` (the texts,
// the plugin's settings and `ui.theme`), `result` (the answer to a `call`) and the host's
// `event`s, and posts `call`s. Only the panels window - `window.parent` - is listened to. The
// state is reactive, so a component re-renders when the texts arrive or change.

import { ref, type Ref } from 'vue';
import { errorText, format, type Params, type Strings } from './format';

export type CallTarget = { tool: string; command?: never } | { command: string; tool?: never };

/** A failed call: `message` is the code, `params` its parameters (the reason among them). */
export interface CallError extends Error {
  params?: Params;
}

export interface Panel {
  /** The texts by full key. */
  readonly strings: Ref<Strings>;
  /** The settings of the last `settings` message (the plugin's keys and `ui.theme`). */
  readonly settings: Ref<Record<string, unknown>>;
  /** Whether the texts arrived. */
  readonly ready: Ref<boolean>;
  /** The host said a call waits for a running call of the plugin (`plugins.panel.waiting`). */
  readonly waiting: Ref<boolean>;
  /** The text of `key` below the plugin's prefix, formatted; the key when the locale lacks it. */
  t(key: string, params?: Params): string;
  /** The sentence for a failed call (a `CallError` or anything else thrown). */
  error(failure: unknown): string;
  /** Runs the plugin's tool or command; rejects with a `CallError`. */
  call(target: CallTarget, args?: unknown): Promise<unknown>;
  /** Posts a message to the panels window as it is (a host action such as `open_link`). */
  post(message: Record<string, unknown>): void;
  /** Opens the settings window at `key` - a key, a section or a category, the core's too - on a
   * user's click (the host's `open_settings`, RB-096-11). */
  openSettings(key: string): void;
  /** Calls `listener` on every `settings` message, `first` the first time. */
  onSettings(listener: (first: boolean) => void): void;
}

interface TextsOf {
  ask(): void;
  take(next: unknown): boolean;
  missing(key: string): void;
}

type Waiting = { resolve: (value: unknown) => void; reject: (error: Error) => void };

export interface PanelOptions {
  /** The plugin's text prefix, `plugin.<id>.`. */
  prefix: string;
  /** The panel's name in the log. */
  name: string;
  /** Called with the settings of every `settings` message (the theme and the text size). */
  onTheme?: (settings: Record<string, unknown>) => void;
  win?: Window;
}

/** The panel's link; `start()` listens and asks for the texts, and returns what stops listening. */
export function createPanel(options: PanelOptions): Panel & { start(): () => void } {
  const win = options.win ?? window;
  const texts: TextsOf = (globalThis as unknown as { PanelTexts: { create(prefix: string, name: string): TextsOf } }).PanelTexts.create(
    options.prefix,
    options.name,
  );
  const strings = ref<Strings>({});
  const settings = ref<Record<string, unknown>>({});
  const ready = ref(false);
  const waiting = ref(false);
  const pending = new Map<number, Waiting>();
  const listeners: ((first: boolean) => void)[] = [];
  let next = 1;

  function t(key: string, params: Params = {}): string {
    const template = strings.value[options.prefix + key];
    if (template !== undefined) return format(template, params);
    texts.missing(options.prefix + key);
    return format(key, params);
  }

  function error(failure: unknown): string {
    if (failure instanceof Error) return errorText(strings.value, failure.message, (failure as CallError).params);
    return errorText(strings.value, String(failure));
  }

  function call(target: CallTarget, args: unknown = {}): Promise<unknown> {
    const id = next++;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      win.parent.postMessage({ type: 'call', id, ...target, args }, '*');
    });
  }

  function receive(event: MessageEvent): void {
    if (event.source !== win.parent) return;
    const data = event.data as Record<string, unknown> | null;
    if (!data || typeof data !== 'object') return;
    if (data.type === 'settings') {
      strings.value = (data.strings as Strings | undefined) ?? {};
      settings.value = (data.settings as Record<string, unknown> | undefined) ?? {};
      const first = texts.take(strings.value);
      ready.value = true;
      options.onTheme?.(settings.value);
      for (const listener of listeners) listener(first);
    } else if (data.type === 'result' && typeof data.id === 'number') {
      const one = pending.get(data.id);
      if (!one) return;
      pending.delete(data.id);
      if (pending.size === 0) waiting.value = false;
      if (data.ok) one.resolve(data.value);
      else {
        const failure: CallError = new Error(String(data.error));
        if (data.params && typeof data.params === 'object') failure.params = data.params as Params;
        one.reject(failure);
      }
    } else if (data.type === 'event') {
      const message = data.message as { host?: unknown } | null;
      if (message?.host === 'waiting' && pending.size > 0) waiting.value = true;
    }
  }

  return {
    strings,
    settings,
    ready,
    waiting,
    t,
    error,
    call,
    post: (message) => win.parent.postMessage(message, '*'),
    openSettings: (key) => win.parent.postMessage({ target: 'host', action: 'open_settings', key }, '*'),
    onSettings: (listener) => {
      listeners.push(listener);
    },
    start() {
      win.addEventListener('message', receive);
      texts.ask();
      return () => win.removeEventListener('message', receive);
    },
  };
}
