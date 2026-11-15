// A panel's channel to one view's DevTools backend (RB-080-24): the page script's relay
// messages of that view and framework, put together again and parsed, in; a panel's envelopes
// out through the component's `send` tool, which posts them to the view's page script. The
// probe's reports come the same way (`watchProbe`).

import type { Envelope, Framework, FrameworkReport } from './protocol';
import { Reassembler, isEnvelope, isRelay } from './protocol';
import type { PanelBridge } from './panel-bridge';
// The one validator of the probe's report, the agent's (RB-080-31): the same bound on the
// version string on both sides of the relay.
import { isFrameworkReport } from '../../../../agent/src/framework/report';

/** The panel's end of a view's channel. */
export interface ViewChannel {
  readonly view: number;
  readonly framework: Framework;
  /** Sends `envelope` to the backend; resolves once the component posted it. */
  post(envelope: Envelope): Promise<void>;
  /** Hands every envelope of the backend to `listener`; returns the function that stops. */
  on(listener: (envelope: Envelope) => void): () => void;
  /** Stops listening. */
  close(): void;
}

/** Opens the channel of `framework` in `view` over `bridge`. */
export function openViewChannel(bridge: PanelBridge, view: number, framework: Framework): ViewChannel {
  const listeners = new Set<(envelope: Envelope) => void>();
  const reassembler = new Reassembler();
  const stop = bridge.onEvent((from, message) => {
    if (from !== view || !isRelay(message) || message.framework !== framework) return;
    const text = reassembler.take(message);
    if (text === null) return;
    let envelope: unknown;
    try {
      envelope = JSON.parse(text);
    } catch {
      return;
    }
    if (!isEnvelope(envelope)) return;
    for (const listener of [...listeners]) listener(envelope);
  });
  return {
    view,
    framework,
    async post(envelope) {
      await bridge.call('send', { view, framework, envelope });
    },
    on(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    close() {
      stop();
      listeners.clear();
    },
  };
}

/** Calls `found` with every probe report a page script relays, with its view. Returns the
 *  function that stops watching. */
export function watchProbe(bridge: PanelBridge, found: (view: number, report: FrameworkReport) => void): () => void {
  return bridge.onEvent((view, message) => {
    if (view === null || !isRelay(message) || message.framework !== 'probe') return;
    let report: unknown;
    try {
      report = JSON.parse(message.text);
    } catch {
      return;
    }
    if (isFrameworkReport(report)) found(view, report);
  });
}
