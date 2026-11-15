// The channel between a framework's DevTools backend in the main world and the panel
// (RB-080-24): DOM events on `document`, whose `detail` is always JSON text (a string crosses
// the worlds as it is). The plugin's page script in its isolated world relays them to the panel
// over the plugin bridge and back. No port, no socket: the page sees the events - it only fools
// its own DevTools.

import type { DevtoolsEnvelope, Framework } from '../protocol';
import { devtoolsInEvent, devtoolsOutEvent } from '../protocol';

/** One end of the channel, in the main world. */
export interface PageChannel {
  /** Sends an envelope towards the panel. */
  post(envelope: DevtoolsEnvelope): void;
  /** Hands every valid envelope from the panel to `listener`; returns the function that stops. */
  on(listener: (envelope: DevtoolsEnvelope) => void): () => void;
}

function isEnvelope(value: unknown): value is DevtoolsEnvelope {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const envelope = value as Record<string, unknown>;
  if (envelope.t === 'connect' || envelope.t === 'disconnect') return true;
  return envelope.t === 'm' && 'd' in envelope && envelope.d !== undefined;
}

/** The main-world end of `framework`'s channel in `win`'s document. */
export function openChannel(win: Window, framework: Framework): PageChannel {
  // Taken once, so a page that replaces JSON or CustomEvent later changes nothing here.
  const doc = win.document;
  const CustomEventCtor = (win as Window & typeof globalThis).CustomEvent;
  const stringify = JSON.stringify;
  const parse = JSON.parse;
  const outName = devtoolsOutEvent(framework);
  const inName = devtoolsInEvent(framework);
  return {
    post(envelope) {
      doc.dispatchEvent(new CustomEventCtor(outName, { detail: stringify(envelope) }));
    },
    on(listener) {
      const handler = (event: Event): void => {
        const detail: unknown = (event as CustomEvent<unknown>).detail;
        if (typeof detail !== 'string') return;
        let envelope: unknown;
        try {
          envelope = parse(detail);
        } catch {
          return;
        }
        if (isEnvelope(envelope)) listener(envelope);
      };
      doc.addEventListener(inName, handler);
      return () => doc.removeEventListener(inName, handler);
    },
  };
}
