import { afterEach, describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore, WebStoreExtension } from '../ipc/protocol';
import type { Transport } from '../ipc/transport';
import InstalledExtensions from './InstalledExtensions.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const VUE: WebStoreExtension = {
  id: 'nhdogjmejiglipccpnnnanhbledajbpd',
  name: 'Vue.js devtools',
  version: '7.6.4',
  loaded: true,
};
const REACT: WebStoreExtension = {
  id: 'fmkadmapgofadopljbjfkapdkoienihi',
  name: 'React Developer Tools',
  version: '6.1.1',
  loaded: false,
};

/**
 * A core that answers the list request and the remove command with `ack` and, like the real
 * one, sends `webStoreExtensions` afterwards from its worker - the removal after the command
 * answered, so a list read right after the answer would still name the row.
 */
function core(items: WebStoreExtension[]) {
  let installed = [...items];
  const sent: ShellToCore[] = [];
  const listeners = new Set<(message: CoreToShell) => void>();
  const push = (now: WebStoreExtension[] = installed) => {
    installed = [...now];
    const message: CoreToShell = { type: 'webStoreExtensions', items: [...installed] };
    listeners.forEach((listener) => listener(message));
  };
  const transport: Transport = {
    async send(message: ShellToCore): Promise<CoreToShell> {
      sent.push(message);
      if (message.type === 'listWebStoreExtensions') {
        setTimeout(() => push(), 0);
      }
      if (message.type === 'runCommand' && message.id === 'extensions.remove') {
        const args = message.args as { ref: string };
        setTimeout(() => push(installed.filter((row) => row.id !== args.ref)), 0);
      }
      return { type: 'ack' };
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  return { client: new IpcClient(transport), sent, push };
}

async function settle(): Promise<void> {
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
}

async function mounted(items: WebStoreExtension[]) {
  const fake = core(items);
  const wrapper = mount(InstalledExtensions, {
    props: { client: fake.client },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  await settle();
  return { wrapper, ...fake };
}

describe('the list of the installed Web Store extensions', () => {
  it('names each with its version and id and whether this start loaded it', async () => {
    const { wrapper } = await mounted([VUE, REACT]);
    const rows = wrapper.findAll('[data-testid="installed-extension"]');
    expect(rows).toHaveLength(2);
    expect(rows[0]?.text()).toContain('Vue.js devtools');
    expect(rows[0]?.text()).toContain('7.6.4');
    expect(rows[0]?.attributes('data-id')).toBe(VUE.id);
    expect(rows[0]?.text()).toContain('Loaded');
    expect(rows[1]?.text()).toContain('next start');
  });

  it('says when nothing is installed', async () => {
    const { wrapper } = await mounted([]);
    expect(wrapper.findAll('[data-testid="installed-extension"]')).toHaveLength(0);
    expect(wrapper.get('[data-testid="installed-extensions-empty"]').text()).toContain('Nothing is installed');
  });

  it('removes a row with the person\'s command and lists again', async () => {
    const { wrapper, sent } = await mounted([VUE, REACT]);
    await wrapper.get(`[data-testid="installed-extension"][data-id="${VUE.id}"] [data-testid="installed-extension-remove"]`).trigger('click');
    await settle();
    expect(sent.filter((m) => m.type === 'runCommand')).toEqual([
      { type: 'runCommand', id: 'extensions.remove', args: { ref: VUE.id } },
    ]);
    const rows = wrapper.findAll('[data-testid="installed-extension"]');
    expect(rows).toHaveLength(1);
    expect(rows[0]?.attributes('data-id')).toBe(REACT.id);
  });

  it('follows the core\'s list after a change it did not make (an MCP install)', async () => {
    const { wrapper, push, sent } = await mounted([VUE]);
    expect(wrapper.findAll('[data-testid="installed-extension"]')).toHaveLength(1);
    sent.length = 0;
    push([VUE, REACT]);
    await settle();
    expect(sent).toEqual([]);
    const rows = wrapper.findAll('[data-testid="installed-extension"]');
    expect(rows).toHaveLength(2);
    expect(rows[1]?.attributes('data-id')).toBe(REACT.id);
  });
});
