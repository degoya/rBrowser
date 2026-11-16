// Runs a shipped page script as the plugin runtime does - one function in strict mode with
// `rbPlugin` in scope - against a fake bridge, and hands its closed shadow roots to the test;
// runs a panel's classic scripts as the frame does, one after the other in the global scope.

import { addLayer, removeLayer } from './layers';

export type Message = Record<string, unknown>;

/** The bridge a page script sees, recording what it sent; `answer` is the host's reply. */
export interface FakePlugin {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  sent: Message[];
  /** What the script reported through `rbPlugin.log`: level and values. */
  logged: Array<{ level: string; values: unknown[] }>;
  answer: (message: Message) => unknown;
  /** The overlays' one host (RB-090-51), as the prologue's `rbPlugin.layers`. */
  readonly layers: { add(host: Element): boolean; remove(host: Element): void };
  send(message: unknown): Promise<unknown>;
  emit(message: unknown): void;
  log(level: string, ...values: unknown[]): void;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
  /** Delivers `message` to the script's listeners, as the host would. */
  post(message: unknown): void;
  /** Tells the script to stop, as the host would. */
  stop(): void;
}

/**
 * A fake bridge for plugin `id` (view 1, 64 KiB messages). `answer` replies to every `send`
 * with a copy of the message; an `Error` it returns or throws rejects the send.
 */
export function fakePlugin(id: string, answer: (message: Message) => unknown): FakePlugin {
  const listeners: Array<(message: unknown) => void> = [];
  const stops: Array<() => void> = [];
  const plugin: FakePlugin = {
    id,
    view: 1,
    maxMessageBytes: 65536,
    sent: [],
    logged: [],
    answer,
    layers: { add: (host) => addLayer(host, id), remove: removeLayer },
    send(message) {
      const copy = JSON.parse(JSON.stringify(message)) as Message;
      plugin.sent.push(copy);
      try {
        const value = plugin.answer(copy);
        return value instanceof Error ? Promise.reject(value) : Promise.resolve(value);
      } catch (error) {
        return Promise.reject(error);
      }
    },
    emit() {},
    log(level, ...values) {
      plugin.logged.push({ level, values });
    },
    on(listener) {
      listeners.push(listener);
      return () => undefined;
    },
    onStop(listener) {
      stops.push(listener);
      return () => undefined;
    },
    post(message) {
      for (const listener of listeners) listener(message);
    },
    stop() {
      for (const listener of stops) listener();
    },
  };
  return plugin;
}

/** Runs the page script `source` against `plugin`; answers the shadow roots it made, in order. */
export function runPageScriptRoots(source: string, plugin: FakePlugin): ShadowRoot[] {
  const roots: ShadowRoot[] = [];
  const attach = HTMLElement.prototype.attachShadow;
  HTMLElement.prototype.attachShadow = function (this: HTMLElement, init: ShadowRootInit) {
    const root = attach.call(this, init);
    roots.push(root);
    return root;
  };
  try {
    new Function('rbPlugin', `'use strict';\n${source}`)(plugin);
  } finally {
    HTMLElement.prototype.attachShadow = attach;
  }
  return roots;
}

/** Runs the page script `source` against `plugin`; answers its one shadow root (the test opens what the page cannot). */
export function runPageScript(source: string, plugin: FakePlugin): ShadowRoot {
  const roots = runPageScriptRoots(source, plugin);
  const [root] = roots;
  if (!root || roots.length !== 1) throw new Error(`the page script made ${roots.length} shadow roots, not one`);
  return root;
}

/**
 * Runs classic scripts in order in the global scope, as a page's `<script>` elements would;
 * answers how to take the listeners they added to the window and the document away again, so
 * one test's panel never hears the next's messages.
 */
export function runClassicScripts(sources: string[]): () => void {
  const added: Array<[EventTarget, string, EventListenerOrEventListenerObject]> = [];
  const targets: EventTarget[] = [window, document];
  const originals = targets.map((target) => target.addEventListener);
  targets.forEach((target, index) => {
    const original = originals[index];
    target.addEventListener = function (this: EventTarget, type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | AddEventListenerOptions) {
      if (listener) added.push([target, type, listener]);
      original?.call(this, type, listener, options);
    };
  });
  try {
    new Function(sources.join('\n'))();
  } finally {
    targets.forEach((target, index) => {
      const original = originals[index];
      if (original) target.addEventListener = original;
    });
  }
  return () => {
    for (const [target, type, listener] of added) target.removeEventListener(type, listener);
  };
}

/** Lets the promises of the fake bridge settle: `rounds` turns of the microtask queue. */
export async function settle(rounds = 5): Promise<void> {
  for (let round = 0; round < rounds; round += 1) await Promise.resolve();
}
