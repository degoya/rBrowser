// The page script of the Framework DevTools plugin (RB-080-24), built into ../page/bridge.js. It
// runs in the plugin's isolated world of every view at `document-start` and relays, in the top
// frame only: what the Vue and React backends in the main world dispatch on `document`
// (`outEvent`) to the panels over `rbPlugin.emit` - chunked under the bridge's message limit -
// and the probe's report the same way; what the panels post back (`rbPlugin.on`) to the
// backends (`inEvent`). When the host stops it, both backends get `disconnect`.

import type { Framework } from './protocol';
import { FRAMEWORKS, FRAMEWORK_EVENT, inEvent, isPost, outEvent, relays } from './protocol';

/** Starts the relay in `win` with `bridge` (the host's `rbPlugin`); false in a child frame. */
export function installRelay(win: Window, bridge: RbPluginBridge): boolean {
  if (win !== win.top) return false;
  const doc = win.document;
  const CustomEventCtor = (win as Window & typeof globalThis).CustomEvent;
  const stringify = JSON.stringify;
  let nextId = 1;
  const listeners: [string, (event: Event) => void][] = [];

  const detailOf = (event: Event): string | null => {
    const detail: unknown = (event as CustomEvent<unknown>).detail;
    return typeof detail === 'string' ? detail : null;
  };
  const toPage = (framework: Framework, text: string): void => {
    doc.dispatchEvent(new CustomEventCtor(inEvent(framework), { detail: text }));
  };
  const emit = (message: unknown): void => {
    try {
      bridge.emit(message);
    } catch {
      // Over the limit even as a chunk, or the bridge is gone: the message is lost, the
      // next one goes.
    }
  };

  for (const framework of FRAMEWORKS) {
    const listener = (event: Event): void => {
      const text = detailOf(event);
      if (text === null) return;
      for (const relay of relays(framework, text, nextId++)) emit(relay);
    };
    doc.addEventListener(outEvent(framework), listener);
    listeners.push([outEvent(framework), listener]);
  }
  const probe = (event: Event): void => {
    const text = detailOf(event);
    if (text !== null) emit({ framework: 'probe', text });
  };
  doc.addEventListener(FRAMEWORK_EVENT, probe);
  listeners.push([FRAMEWORK_EVENT, probe]);

  const unsubscribe = bridge.on((message) => {
    if (isPost(message)) toPage(message.framework, stringify(message.envelope));
  });
  bridge.onStop(() => {
    unsubscribe();
    for (const [name, listener] of listeners) doc.removeEventListener(name, listener);
    for (const framework of FRAMEWORKS) toPage(framework, stringify({ t: 'disconnect' }));
  });
  return true;
}
