// RB-090-08: every page of the shell passes axe-core's WCAG 2.2 A and AA rules - the toolbar,
// the status bar and every route of the pages window - over the mock core. Contrast is measured
// by `assets/contrast.test.ts` in both themes; what needs a layout is in `test/axe.ts`.

import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { h, type Component } from 'vue';
import App from '../App.vue';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import { createMockTransport } from '../ipc/mockTransport';
import { PALETTE_CLIENT_KEY } from '../palette/palette';
import PagesApp from '../settings/PagesApp.vue';
import { PAGES } from '../settings/routes';
import { createShell, SHELL_KEY } from '../state/shell';
import StatusBarApp from '../statusbar/StatusBarApp.vue';
import { axeViolations } from '../test/axe';
import { unnamedFocusables, unreachableControls } from '../test/keyboard';
import { settle } from '../test/mountShell';
import { createTrust, TRUST_KEY } from '../trust/trust';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  window.location.hash = '';
});

/** Mounts `root` the way its page does: Nuxt UI, i18n, the shell and trust stores, a client. */
async function mountPage(root: Component): Promise<VueWrapper> {
  const mock = createMockTransport();
  const client = new IpcClient(mock);
  const shell = createShell(client, {
    root: document.createElement('html'),
    matchMedia: () => ({ matches: false, addEventListener: () => undefined, removeEventListener: () => undefined }),
  });
  await shell.start();
  const wrapper = mount({ render: () => h(root, { client }) }, {
    attachTo: document.body,
    global: {
      plugins: [ui, createShellI18n()],
      provide: { [SHELL_KEY]: shell, [TRUST_KEY]: createTrust(client), [PALETTE_CLIENT_KEY]: client },
    },
  });
  current = wrapper;
  for (let i = 0; i < 3; i += 1) await settle();
  return wrapper;
}

describe('axe-core over the shell pages', () => {
  it('finds no WCAG violation in the toolbar page', async () => {
    await mountPage(App);
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation in the status bar page', async () => {
    await mountPage(StatusBarApp);
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it.each(Object.keys(PAGES))('finds no WCAG violation on the route #/%s', async (route) => {
    window.location.hash = `#/${route}`;
    await mountPage(PagesApp);
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });
});
