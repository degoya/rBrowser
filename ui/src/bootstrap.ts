// What both pages share: the transport (CEF's, or the mock core outside CEF), the IPC client,
// the shell store, vue-i18n and Nuxt UI.

import ui from '@nuxt/ui/vue-plugin';
import { _api as iconifyApi } from '@iconify/vue';
import { createApp, type Component } from 'vue';
import { IpcClient, measureRoundtrip } from './ipc/client';
import { createMockTransport } from './ipc/mockTransport';
import { createCefTransport, hasCefQuery, type Transport } from './ipc/transport';
import { EMULATION_QUERY_KEY } from './emulation/emulation';
import { AUTH_STATUS_QUERY_KEY } from './basic-auth/basicAuth';
import { createShellI18n } from './i18n';
import { PALETTE_CLIENT_KEY } from './palette/palette';
import { createShell, SHELL_KEY, type Shell } from './state/shell';
import { PORTAL_SOURCE_KEY, portalSource } from './toolbar/portalButton';
import { ACTIVITIES_KEY, activitySource, createActivities } from './activity/store';
import { createTrust, TRUST_KEY } from './trust/trust';
import { guardZoom } from './zoomGuard';

/** Without CEF's message router in a production build: every request fails, the status says so. */
const NO_CORE: Transport = {
  send: () => Promise.reject(new Error('window.cefQuery is missing')),
  subscribe: () => () => undefined,
};

/**
 * CEF's transport when the message router is there; the mock core in `npm run dev` (the branch
 * and the mock are dropped from the production build).
 */
export function createTransport(win: Window = window): Transport {
  if (hasCefQuery(win)) {
    return createCefTransport(win);
  }
  if (import.meta.env.DEV) {
    return createMockTransport({ latencyMs: 30 });
  }
  return NO_CORE;
}

declare global {
  interface Window {
    /** For the acceptance run in the shell's DevTools: `await rbShell.measureIpc(200)`. */
    rbShell?: { measureIpc: (n: number) => ReturnType<typeof measureRoundtrip> };
  }
}

export function mountShell(root: Component, selector = '#app'): Shell {
  // No page of the shell zooms (RB-080-50).
  guardZoom(window);
  // Icons are bundled at build time; the shell never fetches one from the Iconify API.
  iconifyApi.setFetch(() => Promise.reject(new Error('icons are bundled')));
  const client = new IpcClient(createTransport());
  const shell = createShell(client, {
    root: document.documentElement,
    matchMedia: (query) => window.matchMedia(query),
  });
  const trust = createTrust(client);
  window.rbShell = { measureIpc: (n) => measureRoundtrip(client, n) };
  createApp(root)
    .provide(SHELL_KEY, shell)
    .provide(TRUST_KEY, trust)
    .provide(PALETTE_CLIENT_KEY, client)
    .provide(PORTAL_SOURCE_KEY, portalSource(client))
    // The activity bar's store (RB-090-67): it reads the activities from its first use on.
    .provide(ACTIVITIES_KEY, createActivities(shell, activitySource(client)))
    .provide(EMULATION_QUERY_KEY, (view) =>
      client.expect({ type: 'getEmulation', view }, 'emulation').then((a) => a.emulation),
    )
    // The Basic auth bar's stored credentials, never a password (RB-094-29).
    .provide(AUTH_STATUS_QUERY_KEY, () =>
      client.expect({ type: 'getBasicAuthStatus' }, 'basicAuthStatus').then((a) => a.environments),
    )
    .use(createShellI18n())
    .use(ui)
    .mount(selector);
  void shell.start();
  trust.start();
  return shell;
}
