// The page script of the simulators plugin (manifest [[page_scripts]], bundled into
// ../page/simulators.js). It runs in the plugin's isolated world of every view, top frame only,
// from `document-start`: asks the component for this view's simulation, follows what the
// component posts (`{"type": "apply", "spec"}`), and removes every trace when the host switches
// it off (docs/plugins.md "Page scripts"). A null spec means nothing to draw - off, or Chromium
// renders it itself.

import { readSpec } from './filter';
import { Simulator } from './simulator';
import type { RbPlugin } from './types';

declare const rbPlugin: RbPlugin | undefined;

/** Wires `simulator` to `bridge`; answers the function that unwires it. */
export function start(bridge: RbPlugin, simulator: Simulator): () => void {
  const warn = (error: unknown): void => console.warn('[simulators]', error instanceof Error ? error.message : error);
  // A post is newer than the answer to `init`: once one arrived, the answer is stale.
  let posted = false;
  const off = bridge.on((message) => {
    if (!message || typeof message !== 'object' || (message as { type?: unknown }).type !== 'apply') return;
    posted = true;
    simulator.apply(readSpec((message as { spec?: unknown }).spec));
  });
  const stop = (): void => {
    off();
    simulator.destroy();
  };
  bridge.onStop(stop);
  bridge
    .send({ op: 'init' })
    .then((answer) => {
      if (!posted) simulator.apply(readSpec((answer as { spec?: unknown } | null)?.spec));
    })
    .catch(warn);
  return stop;
}

if (typeof rbPlugin !== 'undefined' && window === window.top) {
  start(rbPlugin, new Simulator(document));
}
