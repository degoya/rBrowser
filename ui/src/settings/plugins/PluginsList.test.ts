import { afterEach, describe, expect, it, vi } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { createShellI18n } from '../../i18n';
import type { JsonValue } from '../../ipc/protocol';
import type { InstalledMessage } from '../../panels/protocol';
import { createSettingsStore } from '../store';
import { createFakeCore, type FakeCore } from '../test/fakeCore';
import { mountPage, settle } from '../test/mountPage';
import PluginsList from './PluginsList.vue';
import { SAMPLE_INSTALLED } from './sample';
import { takeShortcutQuery } from './shortcutQuery';

let current: VueWrapper | null = null;

async function open(installed: InstalledMessage = SAMPLE_INSTALLED, prepare: (core: FakeCore) => void = () => undefined) {
  const core = createFakeCore({ installed });
  prepare(core);
  const store = createSettingsStore(core.client);
  await store.start();
  const mounted = await mountPage(PluginsList, { store }, core);
  current = mounted.wrapper;
  return { ...mounted, store };
}

const LOADING: InstalledMessage = { loading: true, plugins: [] };

/** The list over a core whose `n`-th answer to `installed` is `answer(n)`. */
async function openPolling(answer: (n: number) => InstalledMessage) {
  let answers = 0;
  const core = createFakeCore();
  const send = core.client.request.bind(core.client);
  core.client.request = async (message) => {
    if (message.type !== 'pluginPanel') {
      return send(message);
    }
    answers += 1;
    return { type: 'pluginPanel', message: answer(answers) };
  };
  const store = createSettingsStore(core.client);
  await store.start();
  const wrapper = mount(PluginsList, { props: { client: core.client, store }, global: { plugins: [ui, createShellI18n()] } });
  current = wrapper;
  await vi.advanceTimersByTimeAsync(0);
  return { wrapper, answers: () => answers };
}

function setValue(core: FakeCore, key: string, value: JsonValue): void {
  core.values.set(key, { key, value, origin: 'user' });
}

afterEach(() => {
  current?.unmount();
  current = null;
  vi.useRealTimers();
});

describe('the plugin list', () => {
  it('shows name, version, source, signature and state of every plugin', async () => {
    const { wrapper } = await open();
    const seo = wrapper.get('[data-plugin="seo"]');
    expect(seo.get('[data-testid="plugin-name"]').text()).toBe('SEO');
    expect(seo.get('[data-testid="plugin-version"]').text()).toBe('Version 0.1.0');
    expect(seo.get('[data-testid="plugin-source"]').text()).toBe('Bundled');
    expect(seo.get('[data-testid="plugin-signature"]').text()).toBe('Signed by rbrowser-plugin-v1');
    expect(seo.get('[data-testid="plugin-state"]').text()).toBe('Active');
    expect(seo.get('[data-testid="plugin-description"]').text()).toContain('a page for the SEO essentials.');
    expect(wrapper.get('[data-plugin="snippets"] [data-testid="plugin-state"]').text()).toBe('Disabled');
    const acme = wrapper.get('[data-plugin="acme"]');
    expect(acme.get('[data-testid="plugin-source"]').text()).toBe('Project');
    expect(acme.get('[data-testid="plugin-signature"]').text()).toBe('Unsigned');
    expect(acme.find('[data-testid="plugin-description"]').exists()).toBe(false);
  });

  it('names why a package did not load, by its file, and gives it no switch', async () => {
    const { wrapper } = await open();
    const broken = wrapper.get('[data-plugin="broken-2.0.0.rbplugin"]');
    expect(broken.attributes('data-state')).toBe('error');
    expect(broken.get('[data-testid="plugin-name"]').text()).toBe('broken-2.0.0.rbplugin');
    expect(broken.get('[data-testid="plugin-state"]').text()).toBe('Not loaded');
    expect(broken.get('[data-testid="plugin-signature"]').text()).toBe('Signature invalid');
    const reason = broken.get('[data-testid="plugin-error"]');
    expect(reason.attributes('role')).toBe('alert');
    expect(reason.text()).toContain('changed after it was signed');
    expect(reason.text()).toContain('plugins.error.signature_invalid');
    expect(broken.find('[data-testid="plugin-enabled"]').exists()).toBe(false);
    expect(broken.find('[data-testid="plugin-settings"]').exists()).toBe(false);
    expect(broken.find('[data-testid="plugin-commands"]').exists()).toBe(false);
  });

  it('marks what a refused package claims as unverified, and a claimed id as a claim', async () => {
    const { wrapper } = await open();
    const broken = wrapper.get('[data-plugin="broken-2.0.0.rbplugin"]');
    expect(broken.get('[data-testid="plugin-declared"]').text()).toContain('(not verified)');
    expect(broken.get('[data-testid="plugin-claims"]').text()).toContain('Claims the id of SEO');
    // The bundled plugin is its own row and keeps its switch.
    expect(wrapper.findAll('[data-plugin="seo"]')).toHaveLength(1);
    expect(wrapper.find('[data-plugin="seo"] [data-testid="plugin-enabled"]').exists()).toBe(true);
  });

  it('never writes an id a refused or waiting package claims', async () => {
    const hostile: InstalledMessage = {
      loading: false,
      plugins: [
        // What an old or lying core might send: an id on rows that did not verify.
        { id: 'seo', file: 'evil.rbplugin', verified: false, source: 'project', state: 'blocked', signature: {} },
        { id: 'seo', name: 'SEO', verified: true, source: 'project', state: 'blocked', signature: {} },
        { id: 'seo', name: 'SEO', verified: true, source: 'user', state: 'error', signature: {} },
      ],
    };
    const { wrapper, core } = await open(hostile);
    expect(wrapper.findAll('[data-testid="plugin-row"]')).toHaveLength(3);
    expect(wrapper.findAll('[data-testid="plugin-enabled"]')).toHaveLength(0);
    expect(wrapper.findAll('[data-plugin="seo"]')).toHaveLength(2);
    expect(core.sent.some((message) => message.type === 'setSetting')).toBe(false);
  });

  it('renders the texts of a manifest as text, never as HTML', async () => {
    const { wrapper } = await open();
    const broken = wrapper.get('[data-plugin="broken-2.0.0.rbplugin"]');
    expect(broken.get('[data-testid="plugin-declared"]').text()).toContain('<img src=x onerror=alert(1)>');
    expect(broken.find('img').exists()).toBe(false);
    const seo = wrapper.get('[data-plugin="seo"]');
    expect(seo.get('[data-testid="plugin-description"]').text()).toBe('<b>Checks</b> a page for the SEO essentials.');
    expect(seo.find('b').exists()).toBe(false);
  });

  it('says where it looked when no plugin was found, and which folder is missing', async () => {
    const { wrapper } = await open({
      loading: false,
      plugins: [],
      folders: { bundled: 'C:\\Tools\\rbrowser\\plugins' },
      missing: { user: 'C:\\Users\\dev\\plugins' },
    });
    const empty = wrapper.get('[data-testid="plugins-empty"]');
    expect(empty.text()).toContain('No plugins were found.');
    expect(empty.text()).toContain('rBrowser looked in C:\\Tools\\rbrowser\\plugins.');
    expect(empty.text()).toContain('C:\\Users\\dev\\plugins does not exist.');
    expect(wrapper.findAll('[data-testid="plugin-row"]')).toHaveLength(0);
  });

  it('says so when there is no folder at all', async () => {
    const { wrapper } = await open({ loading: false, plugins: [], folders: {} });
    expect(wrapper.get('[data-testid="plugins-empty"]').text()).toContain('no plugin folder');
  });

  it('hides the bundled plugins when plugins.showBundled is off and says how many', async () => {
    const { wrapper } = await open(SAMPLE_INSTALLED, (core) => setValue(core, 'plugins.showBundled', false));
    const ids = wrapper.findAll('[data-testid="plugin-row"]').map((row) => row.attributes('data-plugin'));
    expect(ids).toEqual(['broken-2.0.0.rbplugin', 'acme']);
    expect(wrapper.get('[data-testid="plugins-hidden"]').text()).toContain('3');
  });

  it('asks again while the core still loads its plugins', async () => {
    vi.useFakeTimers();
    const polled = await openPolling((n) => (n === 1 ? LOADING : SAMPLE_INSTALLED));
    expect(polled.wrapper.get('[data-testid="plugins-loading"]').text()).toContain('still loading');
    await vi.advanceTimersByTimeAsync(1000);
    expect(polled.answers()).toBe(2);
    expect(polled.wrapper.findAll('[data-testid="plugin-row"]')).toHaveLength(5);
    await vi.advanceTimersByTimeAsync(5000);
    expect(polled.answers()).toBe(2);
  });

  it('keeps one chain of requests when Refresh is pressed while one is on its way', async () => {
    vi.useFakeTimers();
    const polled = await openPolling(() => LOADING);
    expect(polled.answers()).toBe(1);
    // Two refreshes while the first retry waits: still one request per second afterwards.
    await polled.wrapper.get('[data-testid="plugins-refresh"]').trigger('click');
    await polled.wrapper.get('[data-testid="plugins-refresh"]').trigger('click');
    await vi.advanceTimersByTimeAsync(0);
    const after = polled.answers();
    expect(after).toBe(3);
    await vi.advanceTimersByTimeAsync(3000);
    expect(polled.answers()).toBe(after + 3);
  });

  it('stops asking when the page is left', async () => {
    vi.useFakeTimers();
    const polled = await openPolling(() => LOADING);
    polled.wrapper.unmount();
    current = null;
    await vi.advanceTimersByTimeAsync(5000);
    expect(polled.answers()).toBe(1);
  });

  it('gives up after its retries with an error and Retry, which starts over', async () => {
    vi.useFakeTimers();
    let ready = false;
    const polled = await openPolling(() => (ready ? SAMPLE_INSTALLED : LOADING));
    await vi.advanceTimersByTimeAsync(61_000);
    expect(polled.answers()).toBe(61);
    expect(polled.wrapper.find('[data-testid="plugins-loading"]').exists()).toBe(false);
    const error = polled.wrapper.get('[data-testid="plugins-error"]');
    expect(error.attributes('role')).toBe('alert');
    expect(error.text()).toContain('did not finish loading');
    ready = true;
    const retry = polled.wrapper.get('[data-testid="plugins-refresh"]');
    expect(retry.text()).toBe('Retry');
    await retry.trigger('click');
    await vi.advanceTimersByTimeAsync(0);
    expect(polled.wrapper.findAll('[data-testid="plugin-row"]')).toHaveLength(5);
  });

  it('writes plugins.disabled when a plugin is switched off or on, and says the next start decides', async () => {
    const { wrapper, core } = await open(SAMPLE_INSTALLED, (c) => setValue(c, 'plugins.disabled', ['snippets']));
    expect(wrapper.find('[data-plugin="seo"] [data-testid="plugin-restart"]').exists()).toBe(false);
    expect(wrapper.find('[data-plugin="snippets"] [data-testid="plugin-restart"]').exists()).toBe(false);

    await wrapper.get('[data-plugin="seo"] [data-testid="plugin-enabled"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'setSetting', key: 'plugins.disabled', value: ['seo', 'snippets'], scope: 'user' });
    expect(wrapper.get('[data-plugin="seo"] [data-testid="plugin-restart"]').text()).toContain('Stops loading');

    await wrapper.get('[data-plugin="snippets"] [data-testid="plugin-enabled"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'setSetting', key: 'plugins.disabled', value: ['seo'], scope: 'user' });
    expect(wrapper.get('[data-plugin="snippets"] [data-testid="plugin-restart"]').text()).toContain('Loads at the next start');
    // Nothing but the list of ids is written: the page installs and trusts nothing.
    const written = core.sent.flatMap((message) => (message.type === 'setSetting' ? [message.key] : []));
    expect(new Set(written)).toEqual(new Set(['plugins.disabled']));
  });

  // RB-090-59: a plugin paused after repeated failures runs again from its row - that plugin alone.
  it('resumes a paused plugin from its row, that plugin alone, and asks for the list again', async () => {
    const paused: InstalledMessage = {
      ...SAMPLE_INSTALLED,
      plugins: SAMPLE_INSTALLED.plugins.map((plugin) =>
        (plugin as { id?: string }).id === 'overlays' ? { ...(plugin as object), state: 'suspended' } : plugin,
      ),
    };
    const { wrapper, core } = await open(paused);
    expect(wrapper.findAll('[data-testid="plugin-resume"]')).toHaveLength(1);
    expect(wrapper.find('[data-plugin="seo"] [data-testid="plugin-resume"]').exists()).toBe(false);
    const resume = wrapper.get('[data-plugin="overlays"] [data-testid="plugin-resume"]');
    expect(resume.text()).toBe('Resume');
    expect(resume.attributes('aria-label')).toBe('Resume Overlays');
    const asked = core.sent.filter((message) => message.type === 'pluginPanel').length;

    await resume.trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'plugins.resume', args: { id: 'overlays' } });
    const resumes = core.sent.filter((message) => message.type === 'runCommand');
    expect(resumes).toHaveLength(1);
    expect(core.sent.filter((message) => message.type === 'pluginPanel').length).toBeGreaterThan(asked);
  });

  it('links to the first key of a plugin with settings, and to its commands', async () => {
    const { wrapper, store } = await open();
    const schema = store.schema.value;
    if (schema === null) throw new Error('no schema');
    const base = schema.defs[0];
    if (base === undefined) throw new Error('no defs');
    store.schema.value = {
      ...schema,
      defs: [
        ...schema.defs,
        { ...base, key: 'seo.titleLength', category: 'checks', section: 'seo', order: 2, plugin: 'seo' },
        { ...base, key: 'seo.crawlDepth', category: 'checks', section: 'seo', order: 1, plugin: 'seo' },
      ],
    };
    await settle();
    await wrapper.get('[data-plugin="seo"] [data-testid="plugin-settings"]').trigger('click');
    expect(wrapper.emitted('open')).toEqual([['seo.crawlDepth']]);
    expect(wrapper.find('[data-plugin="overlays"] [data-testid="plugin-settings"]').exists()).toBe(false);
    expect(wrapper.find('[data-plugin="overlays"] [data-testid="plugin-commands"]').exists()).toBe(false);

    const commands = wrapper.get('[data-plugin="seo"] [data-testid="plugin-commands"]');
    expect(commands.text()).toBe('Commands: 3');
    await commands.trigger('click');
    expect(wrapper.emitted('open')?.[1]).toEqual(['shortcuts.bindings']);
    expect(takeShortcutQuery()).toBe('plugin.seo.');
    expect(takeShortcutQuery()).toBe('');
  });

  it('says so when the core does not answer', async () => {
    const core = createFakeCore();
    const send = core.client.request.bind(core.client);
    core.client.request = async (message) => {
      if (message.type === 'pluginPanel') throw new Error('plugins.error.not_available');
      return send(message);
    };
    const store = createSettingsStore(core.client);
    await store.start();
    const mounted = await mountPage(PluginsList, { store }, core);
    current = mounted.wrapper;
    expect(current.get('[data-testid="plugins-error"]').text()).toContain('plugins.error.not_available');
  });
});
