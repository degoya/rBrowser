// The React DevTools backend in a view's main world (RB-080-24): `react-devtools-inline`'s
// hook (`__REACT_DEVTOOLS_GLOBAL_HOOK__`), installed before React loads so React registers its
// renderer, and its bridge over rBrowser's channel (`channel.ts`) instead of a WebSocket or
// `postMessage`. The backend starts when the panel says `connect` and stops at `disconnect`;
// a second `connect` (the panel opened again) shuts the old bridge down first. The hook carries
// `HOOK_MARKER`, so the probe can tell it from the extension's.

import { activate, createBridge, initialize, type BackendBridge } from 'react-devtools-inline/backend';
import type { JsonValue } from '../protocol';
import { HOOK_MARKER } from '../protocol';
import { type PageChannel, openChannel } from './channel';

type Scope = Window & { __REACT_DEVTOOLS_GLOBAL_HOOK__?: Record<string, unknown> };

/** The hook a bridge is run through: `activate` with a bridge over `channel`. */
function connect(win: Window, channel: PageChannel): BackendBridge {
  const bridge = createBridge(win, {
    listen(fn) {
      return channel.on((envelope) => {
        if (envelope.t !== 'm') return;
        const message = envelope.d;
        if (typeof message === 'object' && message !== null && !Array.isArray(message)) {
          fn({ event: String(message.event), payload: message.payload });
        }
      });
    },
    send(event, payload) {
      channel.post({ t: 'm', d: { event, payload: (payload ?? null) as JsonValue } });
    },
  });
  activate(win, { bridge });
  return bridge;
}

/** Installs the hook in `win`'s top document and waits for the panel. False in a child frame
 *  or when another hook (the browser extension's) is there already. */
export function installReactDevtools(win: Window): boolean {
  const scope = win as Scope;
  if (win !== win.top || scope.__REACT_DEVTOOLS_GLOBAL_HOOK__ !== undefined) return false;
  initialize(win);
  // Read afresh: the early return above narrowed `scope`'s property to undefined.
  const hook = (win as Scope).__REACT_DEVTOOLS_GLOBAL_HOOK__;
  if (hook === undefined) return false;
  hook[HOOK_MARKER] = true;
  const channel = openChannel(win, 'react');
  let bridge: BackendBridge | null = null;
  channel.on((envelope) => {
    if (envelope.t === 'connect') {
      bridge?.shutdown();
      bridge = connect(win, channel);
    } else if (envelope.t === 'disconnect') {
      bridge?.shutdown();
      bridge = null;
    }
  });
  return true;
}
