// Settings > Plugins (RB-080-51): the page lists the installed plugins the core reports. Before
// the job the category rendered only the `plugins.*` keys - no component asked the core for the
// plugins, so "Installed plugins" named none of the two dozen bundled ones.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingsPage from '../SettingsPage.vue';
import { createFakeCore } from '../test/fakeCore';
import { mountPage, settle } from '../test/mountPage';
import { SAMPLE_INSTALLED } from './sample';

let current: VueWrapper | null = null;

async function open(core = createFakeCore({ installed: SAMPLE_INSTALLED })) {
  const mounted = await mountPage(SettingsPage, { routeKey: 'plugins' }, core);
  current = mounted.wrapper;
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

describe('Settings > Plugins', () => {
  it('asks the core for the installed plugins and lists every one of them', async () => {
    const { wrapper, core } = await open();
    expect(core.sent).toContainEqual({ type: 'pluginPanel', request: { op: 'installed' } });
    const ids = () => wrapper.findAll('[data-testid="plugin-row"]').map((row) => row.attributes('data-plugin'));
    // Installed (RB-080-80): Snippets is off at this start but switched on again for the next.
    expect(ids()).toEqual(['seo', 'overlays', 'snippets', 'acme']);
    const tab = (label: string) =>
      wrapper.findAll('[data-testid="settings-tabs"] button[role="tab"]').find((t) => t.text().startsWith(label));
    await tab('Disabled')?.trigger('mousedown', { button: 0 });
    await settle();
    expect(ids()).toEqual(['broken-2.0.0.rbplugin']);
    // The plugin system's own keys are on Settings.
    await tab('Settings')?.trigger('mousedown', { button: 0 });
    await settle();
    expect(wrapper.get('[data-section="plugins.installed"]').find('[data-key="plugins.crashLimit"]').exists()).toBe(true);
  });

  it('opens the settings of a plugin where they hang, and its commands in the shortcut editor', async () => {
    const core = createFakeCore({ installed: SAMPLE_INSTALLED });
    const { wrapper } = await open(core);
    // The fake schema has no plugin keys: a plugin without settings offers no link.
    expect(wrapper.find('[data-plugin="seo"] [data-testid="plugin-settings"]').exists()).toBe(false);
    await wrapper.get('[data-plugin="acme"] [data-testid="plugin-commands"]').trigger('click');
    await settle();
    const search = wrapper.get<HTMLInputElement>('[data-testid="shortcut-search"]');
    expect(search.element.value).toBe('plugin.acme.');
    expect(wrapper.findAll('[data-command]').map((row) => row.attributes('data-command'))).toEqual([
      'plugin.acme.close_all',
    ]);
  });
});
