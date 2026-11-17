// The Vue DevTools backend in a view's main world (RB-080-24): `@vue/devtools-kit`'s hook
// (`__VUE_DEVTOOLS_GLOBAL_HOOK__`, Vue 3 announces its apps to it; apps mounted up to three
// seconds before it arrives are replayed) and the RPC function table of `vue-functions.ts`,
// served over rBrowser's channel (`channel.ts`) as a birpc channel - the functions the official
// client calls over its iframe, extension or Vite presets. The panel is the client; `connect`
// and `disconnect` tell the kit whether one listens and whether a client is there at all: the
// kit tracks components, inspectors and timeline events only while a client is detected
// (`highPerfModeEnabled` off), as it does for the extension and the Vite plugin. The hook
// carries `HOOK_MARKER`.

import {
  createRpcServer,
  devtools,
  toggleClientConnected,
  updateDevToolsClientDetected,
} from '@vue/devtools-kit';
import type { JsonValue } from '../protocol';
import { HOOK_MARKER } from '../protocol';
import { openChannel } from './channel';
import { broadcastUpdates, functions } from './vue-functions';

type Scope = Window & { __VUE_DEVTOOLS_GLOBAL_HOOK__?: Record<string, unknown> };

/** Installs the hook and the RPC server in `win`'s top document. False in a child frame or
 *  when the hook of Vue DevTools 7+ (the extension's) is there already. */
export function installVueDevtools(win: Window): boolean {
  const scope = win as Scope;
  if (win !== win.top) return false;
  const existing = scope.__VUE_DEVTOOLS_GLOBAL_HOOK__;
  if (existing !== undefined && existing.id === 'vue-devtools-next') return false;
  devtools.init();
  const hook = scope.__VUE_DEVTOOLS_GLOBAL_HOOK__;
  if (hook === undefined) return false;
  hook[HOOK_MARKER] = true;
  const channel = openChannel(win, 'vue');
  createRpcServer(functions, {
    channel: {
      post: (data: unknown) => channel.post({ t: 'm', d: (data ?? null) as JsonValue }),
      on: (handler: (data: unknown) => void) =>
        channel.on((envelope) => {
          if (envelope.t === 'm') handler(envelope.d);
        }),
    },
  });
  broadcastUpdates();
  channel.on((envelope) => {
    if (envelope.t === 'connect') {
      updateDevToolsClientDetected({ rbrowser: true });
      toggleClientConnected(true);
    } else if (envelope.t === 'disconnect') {
      toggleClientConnected(false);
      updateDevToolsClientDetected({ rbrowser: false });
    }
  });
  return true;
}
