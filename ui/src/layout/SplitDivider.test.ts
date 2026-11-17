// The split's divider is a Nuxt UI slider (RB-080-88): named and read out as before, and it sends
// the share only when it is let go - by the arrow keys at once, by the mouse on release.
import USlider from '@nuxt/ui/components/Slider.vue';
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import SplitDivider from './SplitDivider.vue';

function mountDivider() {
  return mount(SplitDivider, {
    props: { split: { ratioPermille: 600, vertical: true, stepPermille: 50 } },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
}

describe('the split divider', () => {
  it('is a slider with the share, its step and its name', async () => {
    const wrapper = mountDivider();
    await settle();
    const slider = wrapper.getComponent(USlider).props() as Record<string, unknown>;
    expect(slider.min).toBe(10);
    expect(slider.max).toBe(90);
    expect(slider.step).toBe(5);
    const thumb = wrapper.get('[role="slider"]');
    expect(thumb.attributes('aria-valuenow')).toBe('60');
    expect(thumb.attributes('aria-label')).toBe('Split divider (primary side left)');
    expect(thumb.attributes('aria-valuetext')).toBe('60 % for the primary side');
    expect(wrapper.find('input[type="range"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('sends the share when an arrow key moves it', async () => {
    const wrapper = mountDivider();
    await settle();
    await wrapper.get('[role="slider"]').trigger('keydown', { key: 'ArrowLeft' });
    await settle();
    expect(wrapper.emitted('change')).toEqual([[0.55]]);
    wrapper.unmount();
  });

  it('sends nothing while it is dragged and the share where it is let go', async () => {
    const wrapper = mountDivider();
    const slider = wrapper.getComponent(USlider);
    slider.vm.$emit('update:modelValue', 42);
    await settle();
    expect(wrapper.emitted('change')).toBeUndefined();
    expect(wrapper.get('[role="slider"]').attributes('aria-valuetext')).toBe('42 % for the primary side');
    slider.vm.$emit('change', new Event('change'));
    await settle();
    expect(wrapper.emitted('change')).toEqual([[0.42]]);
    wrapper.unmount();
  });
});
