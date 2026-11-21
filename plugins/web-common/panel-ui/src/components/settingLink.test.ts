import { mount } from '@vue/test-utils';
import ui from '@nuxt/ui/vue-plugin';
import { describe, expect, it, vi } from 'vitest';
import { PANEL_KEY } from '../mount';
import type { Panel } from '../panel';
import SettingLink from './SettingLink.vue';

// RB-094-21 (owner): a text that names a settings path opens Settings at its key.
describe('a settings mention as a panel link', () => {
  it('opens Settings at its key on a click, with the plugin text as its name', async () => {
    const openSettings = vi.fn();
    const panel = { openSettings } as unknown as Panel;
    const wrapper = mount(SettingLink, {
      props: { settingKey: 'fonts.previewMaxKb', label: 'Open the setting' },
      global: { plugins: [ui], provide: { [PANEL_KEY as symbol]: panel } },
    });
    const button = wrapper.get('[data-testid="setting-link-fonts.previewMaxKb"]');
    expect(button.text()).toBe('Open the setting');
    expect(button.attributes('title')).toBe('fonts.previewMaxKb');
    await button.trigger('click');
    expect(openSettings).toHaveBeenCalledWith('fonts.previewMaxKb');
  });
});
