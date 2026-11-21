import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import type { ShellToCore } from '../ipc/protocol';
import SettingLink from './SettingLink.vue';

afterEach(() => {
  document.body.replaceChildren();
});

// RB-094-21 (owner): a text that names a settings path opens Settings at its key.
describe('a settings mention as a link', () => {
  it('opens Settings at its key on a click', async () => {
    const wrapper = mount(SettingLink, {
      props: { settingKey: 'mcp.enabled' },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    const button = wrapper.get('[data-testid="setting-link-mcp.enabled"]');
    expect(button.text()).toBe('Open the setting');
    expect(button.attributes('aria-label')).toBe('Open the setting mcp.enabled');
    await button.trigger('click');
    expect(wrapper.emitted('open')?.map(([message]) => message as ShellToCore)).toEqual([
      { type: 'openSettings', key: 'mcp.enabled' },
    ]);
  });
});
