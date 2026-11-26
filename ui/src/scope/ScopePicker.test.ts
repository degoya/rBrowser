import { describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { nextTick, type Component } from 'vue';
import USelect from '@nuxt/ui/components/Select.vue';
import USelectMenu from '@nuxt/ui/components/SelectMenu.vue';
import { memoryStorage } from '../../../plugins/web-common/panel-ui/src/storage';
import { createShellI18n } from '../i18n';
import ScopePicker from './ScopePicker.vue';
import { useScope } from './useScope';

const BREAKPOINTS = [{ id: 'mobile', label: 'Mobile' }, { id: 'desktop' }];

describe('the scope pick list (RB-099-10)', () => {
  it('chooses every view, the visible views or chosen breakpoints from a pick list', async () => {
    const wrapper = mount(ScopePicker, {
      props: { modelValue: { mode: 'all', breakpoints: [] }, breakpoints: BREAKPOINTS },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    expect(wrapper.findAllComponents(USelectMenu)).toHaveLength(0);
    const mode = wrapper.findAllComponents(USelect)[0]!;
    mode.vm.$emit('update:modelValue', 'visible');
    mode.vm.$emit('update:modelValue', 'chosen');
    expect(wrapper.emitted('update:modelValue')).toEqual([
      [{ mode: 'visible', breakpoints: [] }],
      [{ mode: 'chosen', breakpoints: [] }],
    ]);
    await wrapper.setProps({ modelValue: { mode: 'chosen', breakpoints: [] } });
    expect(wrapper.find('[data-testid="scope-empty"]').exists()).toBe(true);
    // The generic SelectMenu types its wrapper as a DOM node: found as a plain component.
    const menu = wrapper.findAllComponents(USelectMenu as unknown as Component)[0]!;
    expect((menu.props() as { items: unknown }).items).toEqual([
      { value: 'mobile', label: 'Mobile' },
      { value: 'desktop', label: 'desktop' },
    ]);
    menu.vm.$emit('update:modelValue', ['desktop']);
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([{ mode: 'chosen', breakpoints: ['desktop'] }]);
    wrapper.unmount();
  });

  it('offers single views beside the scopes and names the one chosen', async () => {
    const wrapper = mount(ScopePicker, {
      props: { modelValue: { mode: 'all', breakpoints: [] }, breakpoints: BREAKPOINTS, views: [{ id: 7, label: '500x800' }] },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    const mode = wrapper.findAllComponents(USelect)[0]!;
    expect((mode.props() as { items: { value: string }[] }).items.map((item) => item.value)).toEqual(['all', 'visible', 'chosen', 'view:7']);
    mode.vm.$emit('update:modelValue', 'view:7');
    expect(wrapper.emitted('update:view')).toEqual([[7]]);
    await wrapper.setProps({ view: 7 });
    expect((mode.props() as { modelValue: unknown }).modelValue).toBe('view:7');
    mode.vm.$emit('update:modelValue', 'visible');
    expect(wrapper.emitted('update:view')?.at(-1)).toEqual([null]);
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([{ mode: 'visible', breakpoints: [] }]);
    wrapper.unmount();
  });

  it('remembers the last choice per function and drops breakpoints the project lost', async () => {
    const storage = memoryStorage();
    const checks = useScope('checks', () => ['mobile', 'desktop'], storage);
    checks.value = { mode: 'chosen', breakpoints: ['mobile'] };
    await nextTick();
    expect(useScope('checks', () => ['mobile'], storage).value).toEqual({ mode: 'chosen', breakpoints: ['mobile'] });
    expect(useScope('flows', () => ['mobile'], storage).value).toEqual({ mode: 'all', breakpoints: [] });
    expect(useScope('checks', () => ['desktop'], storage).value).toEqual({ mode: 'all', breakpoints: [] });
  });
});
