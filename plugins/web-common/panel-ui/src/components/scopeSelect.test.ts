import { mount } from '@vue/test-utils';
import ui from '@nuxt/ui/vue-plugin';
import USelect from '@nuxt/ui/components/Select.vue';
import USelectMenu from '@nuxt/ui/components/SelectMenu.vue';
import { describe, expect, it } from 'vitest';
import ScopeSelect from './ScopeSelect.vue';

const TEXTS = {
  label: 'Run in',
  all: 'All views',
  visible: 'Visible views',
  chosen: 'Chosen breakpoints',
  breakpoints: 'Breakpoints',
  pick: 'Choose breakpoints',
  none: 'Choose at least one breakpoint.',
};

// RB-099-10: a panel's run over every view, the visible views or chosen breakpoints.
describe("a panel's scope pick list", () => {
  it('offers the three modes with the panel texts and the breakpoints when chosen', async () => {
    const wrapper = mount(ScopeSelect, {
      props: { modelValue: { mode: 'all', breakpoints: [] }, breakpoints: [{ id: 'mobile', label: 'Mobile' }], texts: TEXTS },
      global: { plugins: [ui] },
    });
    expect(wrapper.getComponent(USelect).props('items')).toEqual([
      { value: 'all', label: 'All views' },
      { value: 'visible', label: 'Visible views' },
      { value: 'chosen', label: 'Chosen breakpoints' },
    ]);
    expect(wrapper.findAllComponents(USelectMenu)).toHaveLength(0);
    wrapper.getComponent(USelect).vm.$emit('update:modelValue', 'chosen');
    expect(wrapper.emitted('update:modelValue')).toEqual([[{ mode: 'chosen', breakpoints: [] }]]);
    await wrapper.setProps({ modelValue: { mode: 'chosen', breakpoints: [] } });
    expect(wrapper.get('[data-testid="scope-empty"]').text()).toBe('Choose at least one breakpoint.');
    wrapper.getComponent(USelectMenu).vm.$emit('update:modelValue', ['mobile']);
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([{ mode: 'chosen', breakpoints: ['mobile'] }]);
  });
});
