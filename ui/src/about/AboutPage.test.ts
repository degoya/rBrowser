import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import { IpcErrorCode, type AboutInfo, type CoreToShell, type ShellToCore } from '../ipc/protocol';
import { IpcError, type Transport } from '../ipc/transport';
import { settle } from '../settings/test/mountPage';
import AboutPage from './AboutPage.vue';
import { allComponents } from './licences';

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
    expect(wrapper.findAll('[data-testid="licence-row"]')).toHaveLength(allComponents().length);
    await wrapper.find('[data-testid="licence-filter"]').setValue('MPL');
    const rows = wrapper.findAll('[data-testid="licence-row"]').map((row) => row.text());
    expect(rows.some((row) => row.includes('axe-core'))).toBe(true);
    expect(rows.every((row) => row.includes('MPL'))).toBe(true);
  });
});
