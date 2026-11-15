// Settings > Plugins in tabs (RB-080-80): Installed, Disabled and Settings, a count beside each
// plugin tab, the tab remembered and in the address, search and "Only changed" without tabs, and
// `settings.sectionTabs` off one page as before.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { takePluginTexts } from '../../i18n/pluginTexts';
import type { JsonValue } from '../../ipc/protocol';
import SettingsPage from '../SettingsPage.vue';
import { createFakeCore, type FakeCore } from '../test/fakeCore';
import { mountPage, settle } from '../test/mountPage';
import { SAMPLE_INSTALLED } from './sample';

let current: VueWrapper | null = null;

function setValue(core: FakeCore, key: string, value: JsonValue): void {
  core.values.set(key, { key, value, origin: 'user' });
}

/** The sample with Snippets switched off, as the start that disabled it read it. */
function sampleCore(): FakeCore {
  const core = createFakeCore({ installed: SAMPLE_INSTALLED });
  setValue(core, 'plugins.disabled', ['snippets']);
  return core;
}

async function open(props: Record<string, unknown> = { routeKey: 'plugins' }, core = sampleCore()) {
  const mounted = await mountPage(SettingsPage, props, core);
  current = mounted.wrapper;
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
  window.localStorage.clear();
});

function tabs(wrapper: VueWrapper) {
  return wrapper.findAll('[data-testid="settings-tabs"] button[role="tab"]');
}

function selectedTab(wrapper: VueWrapper): string | undefined {
  return tabs(wrapper)
    .find((tab) => tab.attributes('aria-selected') === 'true')
    ?.text();
}

function rows(wrapper: VueWrapper): (string | undefined)[] {
  return wrapper.findAll('[data-testid="plugin-row"]').map((row) => row.attributes('data-plugin'));
}

async function choose(wrapper: VueWrapper, label: string): Promise<void> {
  const tab = tabs(wrapper).find((candidate) => candidate.text().startsWith(label));
  if (tab === undefined) throw new Error(`no tab ${label}`);
  await tab.trigger('mousedown', { button: 0 });
  await settle();
}

describe('Settings > Plugins in tabs (RB-080-80)', () => {
  it('splits the plugins into Installed and Disabled, with the count of each, and their settings', async () => {
    const { wrapper } = await open();
    // The label and its count (the badge).
    expect(tabs(wrapper).map((tab) => tab.text())).toEqual(['Installed3', 'Disabled2', 'Settings']);
    expect(selectedTab(wrapper)).toBe('Installed3');
    expect(rows(wrapper)).toEqual(['seo', 'overlays', 'acme']);
    // No settings card on a list tab.
    expect(wrapper.find('[data-section="plugins.installed"]').exists()).toBe(false);

    await choose(wrapper, 'Disabled');
    expect(rows(wrapper)).toEqual(['snippets', 'broken-2.0.0.rbplugin']);
    expect(window.location.hash).toBe('#/settings/plugins?tab=disabled');
    // A failed package with its reason, and the list's Retry.
    expect(wrapper.get('[data-plugin="broken-2.0.0.rbplugin"] [data-testid="plugin-error"]').text()).toContain(
      'changed after it was signed',
    );
    expect(wrapper.find('[data-testid="plugins-refresh"]').exists()).toBe(true);

    await choose(wrapper, 'Settings');
    expect(wrapper.find('[data-testid="plugins-list"]').exists()).toBe(false);
    expect(wrapper.get('[data-section="plugins.installed"]').find('[data-key="plugins.crashLimit"]').exists()).toBe(true);
  });

  it('moves a plugin switched off to Disabled, the focus to the next switch, and counts again', async () => {
    const { wrapper, core } = await open();
    const seo = wrapper.get('[data-plugin="seo"] [data-testid="plugin-enabled"]');
    (seo.element as HTMLElement).focus();
    await seo.trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'setSetting', key: 'plugins.disabled', value: ['seo', 'snippets'], scope: 'user' });
    expect(rows(wrapper)).toEqual(['overlays', 'acme']);
    expect(tabs(wrapper).map((tab) => tab.text())).toEqual(['Installed2', 'Disabled3', 'Settings']);
    expect(document.activeElement).toBe(wrapper.get('[data-plugin="overlays"] [data-testid="plugin-enabled"]').element);

    await choose(wrapper, 'Disabled');
    expect(rows(wrapper)).toEqual(['seo', 'snippets', 'broken-2.0.0.rbplugin']);
    expect(wrapper.get('[data-plugin="seo"] [data-testid="plugin-restart"]').text()).toContain('Stops loading');
  });

  it('keeps the focus on the list when the last plugin of a tab leaves it', async () => {
    const { wrapper } = await open({ routeKey: 'plugins', routeTab: 'disabled' });
    const snippets = wrapper.get('[data-plugin="snippets"] [data-testid="plugin-enabled"]');
    (snippets.element as HTMLElement).focus();
    await snippets.trigger('click');
    await settle();
    expect(rows(wrapper)).toEqual(['broken-2.0.0.rbplugin']);
    // The refused package has no switch: the list itself takes the focus.
    expect(document.activeElement).toBe(wrapper.get('[data-testid="plugins-list"]').element);
  });

  it('opens the tab a link names, a plugin key in Settings, and the tab it was left on', async () => {
    const { wrapper } = await open({ routeKey: 'plugins', routeTab: 'disabled' });
    expect(selectedTab(wrapper)).toBe('Disabled2');
    await wrapper.setProps({ routeKey: 'plugins.crashLimit', routeTab: null });
    await settle();
    expect(selectedTab(wrapper)).toBe('Settings');
    expect(wrapper.get('[data-key="plugins.crashLimit"]').attributes('data-highlighted')).toBe('true');
    // Left on Disabled, back from General: Disabled again.
    await choose(wrapper, 'Disabled');
    await wrapper.get('[data-category="general"]').trigger('click');
    await settle();
    await wrapper.get('[data-category="plugins"]').trigger('click');
    await settle();
    expect(selectedTab(wrapper)).toBe('Disabled2');
  });

  it('searches without tabs, and shows one page with settings.sectionTabs off', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="search"]').setValue('crash');
    await settle();
    expect(wrapper.find('[data-testid="settings-tabs"]').exists()).toBe(false);
    expect(wrapper.find('[data-key="plugins.crashLimit"]').exists()).toBe(true);
    current?.unmount();

    const core = sampleCore();
    setValue(core, 'settings.sectionTabs', false);
    const page = await open({ routeKey: 'plugins' }, core);
    expect(page.wrapper.find('[data-testid="settings-tabs"]').exists()).toBe(false);
    expect(rows(page.wrapper)).toEqual(['seo', 'overlays', 'snippets', 'broken-2.0.0.rbplugin', 'acme']);
    expect(page.wrapper.get('[data-section="plugins.installed"]').find('[data-testid="plugins-list"]').exists()).toBe(true);
  });
});

describe('the texts of a plugin in the settings window (RB-080-82)', () => {
  it('names a plugin key by the label and the plugin page by the name the schema carries', async () => {
    const core = sampleCore();
    const send = core.client.request.bind(core.client);
    core.client.request = async (message) => {
      const answer = await send(message);
      if (answer.type !== 'settings' || answer.schema === undefined) return answer;
      const schema = answer.schema as { defs: Record<string, unknown>[] } & Record<string, unknown>;
      const base = schema.defs.find((def) => def.key === 'plugins.crashLimit');
      if (base === undefined) throw new Error('no base def');
      const def = {
        ...base,
        key: 'seo.depth',
        category: 'checks',
        section: 'seo',
        order: 1,
        plugin: 'seo',
        labelKey: 'plugin.seo.settings.depth.label',
        descriptionKey: 'plugin.seo.settings.depth.description',
        helpKey: 'plugin.seo.settings.depth.help',
      };
      const texts = {
        'plugin.seo.name': 'SEO',
        'plugin.seo.settings.depth.label': 'Crawl depth',
        'plugin.seo.settings.depth.description': 'How many links deep.',
        'plugin.seo.settings.depth.help': 'Levels of links.',
      };
      // The override bypasses the client's own request, so the texts take its door by hand.
      const next = { ...answer, schema: { ...schema, defs: [...schema.defs, def], texts } } as typeof answer;
      takePluginTexts(next);
      return next;
    };
    const { wrapper } = await open({ routeKey: 'plugins', routeTab: 'settings' }, core);
    const page = wrapper.get('[data-section="plugins.plugin:seo"]');
    expect(page.text()).toContain('SEO');
    expect(page.get('[data-key="seo.depth"]').text()).toContain('Crawl depth');
    expect(page.text()).not.toContain('plugin.seo.settings.depth.label');
  });
});
