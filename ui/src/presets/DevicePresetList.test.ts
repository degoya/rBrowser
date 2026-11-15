import { describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import builtin from '../../../crates/rb-config/data/devices.json';
import type { DevicePreset } from '../ipc/protocol';
import { createShellI18n } from '../i18n';
import DevicePresetList from './DevicePresetList.vue';

function render(items: DevicePreset[]) {
  return mount(DevicePresetList, {
    props: { items },
    global: { plugins: [ui, createShellI18n()] },
  });
}

describe('the device preset list', () => {
  it('lists every preset under its kind and reports the one picked', async () => {
    const items = builtin as DevicePreset[];
    const wrapper = render(items);
    expect(wrapper.findAll('h3').map((h) => h.text())).toEqual(['Phones', 'Tablets', 'Desktops']);
    expect(wrapper.findAll('[data-device]')).toHaveLength(items.length);
    await wrapper.get('[data-device="iphone-15"]').trigger('click');
    expect(wrapper.emitted('pick')).toEqual([['iphone-15']]);
    expect(wrapper.get('[data-device="iphone-15"]').text()).toContain('iPhone 15');
  });

  it('says so when there is none', () => {
    expect(render([]).text()).toBe('No device presets.');
  });
});
