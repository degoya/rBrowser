// Mounts a pages-window component over the fake core, with Nuxt UI, vue-i18n and the trust store
// (the workspace trust page sits under Security & Privacy and Project).

import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import type { Component } from 'vue';
import { createPagesI18n } from '../../i18n/pages';
import { createTrust, TRUST_KEY } from '../../trust/trust';
import { createFakeCore, type FakeCore } from './fakeCore';

export async function mountPage(
  component: Component,
  props: Record<string, unknown> = {},
  core: FakeCore = createFakeCore(),
) {
  const wrapper = mount(component, {
    props: { client: core.client, ...props },
    attachTo: document.body,
    global: {
      plugins: [ui, createPagesI18n()],
      provide: { [TRUST_KEY]: createTrust(core.client) },
    },
  });
  await settle();
  return { wrapper, core };
}

/** Lets the fake core's promises and Vue's re-render settle. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
}
