// Mounts a component the way the pages do (Nuxt UI, vue-i18n, the shell and trust stores) over the
// mock core.

import ui from '@nuxt/ui/vue-plugin';
import { mount, type ComponentMountingOptions } from '@vue/test-utils';
import type { Component } from 'vue';
import { IpcClient } from '../ipc/client';
import { createMockTransport, type MockOptions } from '../ipc/mockTransport';
import { createShellI18n } from '../i18n';
import { createShell, SHELL_KEY } from '../state/shell';
import { createTrust, TRUST_KEY } from '../trust/trust';

export async function mountWithShell<C extends Component>(
  component: C,
  options: { mock?: MockOptions; mount?: ComponentMountingOptions<C> } = {},
) {
  const mock = createMockTransport(options.mock);
  const client = new IpcClient(mock);
  const shell = createShell(client, {
    root: document.createElement('html'),
    matchMedia: () => ({
      matches: false,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }),
  });
  await shell.start();
  const wrapper = mount(component, {
    ...options.mount,
    attachTo: document.body,
    global: {
      plugins: [ui, createShellI18n()],
      provide: { [SHELL_KEY]: shell, [TRUST_KEY]: createTrust(client) },
    },
  } as ComponentMountingOptions<C>);
  return { wrapper, shell, mock };
}

/** Lets the mock's promises and Vue's re-render settle. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
}
