import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import { IpcErrorCode, type AboutInfo, type CoreToShell, type ShellToCore } from '../ipc/protocol';
import { IpcError, type Transport } from '../ipc/transport';
import { settle } from '../settings/test/mountPage';
import UTable from '@nuxt/ui/components/Table.vue';
import AboutPage from './AboutPage.vue';
import { allComponents, type Component } from './licences';

const INFO: AboutInfo = {
  version: '0.1.0',
  commit: 'f98c7fa1',
  cef: '156.0.26+g8ea0c73',
  chromium: '156.0.8078.88',
  built: '2026-11-10T12:00:00Z',
  platform: 'windows x86_64',
  apis: { plugin: 'rbrowser:plugin@0.1.0', configSchema: '1' },
  dirs: { logs: 'C:/rb/logs' },
};

function core(answer: (message: ShellToCore) => CoreToShell) {
  const sent: ShellToCore[] = [];
  const transport: Transport = {
    send: (message) => {
      sent.push(message);
      try {
        return Promise.resolve(answer(message));
      } catch (error) {
        return Promise.reject(error);
      }
    },
    subscribe: () => () => undefined,
  };
  return { client: new IpcClient(transport), sent };
}

async function mountAbout(answer: (message: ShellToCore) => CoreToShell = (message) =>
  message.type === 'getAbout' ? { type: 'about', ...INFO } : { type: 'ack' }) {
  const fake = core(answer);
  const wrapper = mount(AboutPage, {
    props: { client: fake.client },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return { wrapper, sent: fake.sent };
}

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('Settings > About', () => {
  it('shows version, commit, build time and platform of the running build', async () => {
    const { wrapper, sent } = await mountAbout();
    expect(sent[0]).toEqual({ type: 'getAbout' });
    const text = wrapper.find('[data-section="about.version"]').text();
    for (const value of ['0.1.0', 'f98c7fa1', '2026-11-10T12:00:00Z', 'windows x86_64']) {
      expect(text).toContain(value);
    }
  });

  it('shows the engines; an absent Firefox reads "not found" and links to the engine settings', async () => {
    const { wrapper, sent } = await mountAbout();
    expect(wrapper.find('[data-engine="chromium"]').text()).toContain('156.0.8078.88');
    expect(wrapper.find('[data-engine="firefox"]').text()).toContain('not found');
    await wrapper.find('[data-testid="open-engines"]').trigger('click');
    await settle();
    expect(sent).toContainEqual({ type: 'openSettings', key: 'engines' });
  });

  // RB-080-73: where testers look for it.
  it('offers Save diagnostics', async () => {
    const { wrapper, sent } = await mountAbout();
    await wrapper.get('[data-testid="diagnostics-save"]').trigger('click');
    await settle();
    expect(sent).toContainEqual({ type: 'runCommand', id: 'diagnostics.create' });
  });

  it('copies the version block to the clipboard', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    const { wrapper } = await mountAbout();
    await wrapper.find('[data-testid="copy-version"]').trigger('click');
    await settle();
    expect(writeText).toHaveBeenCalledOnce();
    const block = String((writeText.mock.calls[0] as unknown[])[0]);
    expect(block.split('\n')[0]).toBe('rBrowser 0.1.0 (commit f98c7fa1, built 2026-11-10T12:00:00Z, windows x86_64)');
    expect(block).toContain('Chromium: 156.0.8078.88');
    expect(wrapper.text()).toContain('Version info copied to the clipboard.');
  });

  it('has a light and a dark logo, switched by the theme class', async () => {
    const { wrapper } = await mountAbout();
    expect(wrapper.find('[data-testid="logo-light"]').classes()).toContain('dark:hidden');
    expect(wrapper.find('[data-testid="logo-dark"]').classes()).toEqual(expect.arrayContaining(['hidden', 'dark:block']));
  });

  it('opens a link in the views and marks it unpublished', async () => {
    const { wrapper, sent } = await mountAbout();
    const source = wrapper.find('[data-link="source"]');
    expect(source.text()).toContain('not yet published');
    await source.find('[data-testid="open-link"]').trigger('click');
    await settle();
    expect(sent).toContainEqual({ type: 'navigate', url: 'https://github.com/degoya/rBrowser' });
  });

  it('says so when the facts cannot be read', async () => {
    const { wrapper } = await mountAbout(() => {
      throw new IpcError(IpcErrorCode.NotImplemented, 'getAbout is not implemented');
    });
    expect(wrapper.find('[role="alert"]').text()).toContain('getAbout is not implemented');
  });

  it('keeps its tabs in the one row that scrolls, like the settings (RB-080-81)', async () => {
    const { wrapper } = await mountAbout();
    const list = wrapper.get('[data-testid="about-tabs"] [role="tablist"]');
    expect(list.classes()).toContain('overflow-x-auto');
    expect(list.classes()).not.toContain('flex-wrap');
  });

  it('lists every component on the Licences tab and filters by licence', async () => {
    const { wrapper } = await mountAbout();
    const licences = wrapper.findAll('button[role="tab"]').find((tab) => tab.text() === 'Licences');
    await licences?.trigger('mousedown', { button: 0 });
    await licences?.trigger('click');
    await settle();
    // RB-080-88: a Nuxt UI `UTable` that renders only the rows in sight, so its data is the list.
    const table = wrapper.findComponent(UTable);
    const props = () => table.props() as Record<string, unknown>;
    const data = () => props().data as Component[];
    expect(props().virtualize).toBeTruthy();
    expect(data()).toHaveLength(allComponents().length);
    expect(wrapper.get('[data-testid="licence-table"] thead').text()).toContain('Licence');
    await wrapper.find('[data-testid="licence-filter"]').setValue('MPL');
    expect(data().some((row) => row.name === 'axe-core')).toBe(true);
    expect(data().every((row) => `${row.name} ${row.license}`.toLowerCase().includes('mpl'))).toBe(true);
    expect(wrapper.get('[data-testid="licence-count"]').text()).toContain(`of ${allComponents().length}`);
  });

  it('names how this installation updates, not a fixed "disabled" (RB-091-05)', async () => {
    const texts = {
      notBuilt: 'Updates are disabled in this build.',
      off: 'Updates are switched off',
      manual: 'only when you ask',
      onStart: 'at every start',
    } as const;
    for (const [state, text] of Object.entries(texts)) {
      const { wrapper } = await mountAbout((message) =>
        message.type === 'getAbout'
          ? { type: 'about', ...INFO, updates: state as keyof typeof texts }
          : { type: 'ack' },
      );
      expect(wrapper.get('[data-testid="about-updates"]').text()).toContain(text);
      wrapper.unmount();
    }
    const { wrapper } = await mountAbout();
    expect(wrapper.find('[data-testid="about-updates"]').exists()).toBe(false);
  });
});
