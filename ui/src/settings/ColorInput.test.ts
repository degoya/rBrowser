import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import UColorPicker from '@nuxt/ui/components/ColorPicker.vue';
import UPopover from '@nuxt/ui/components/Popover.vue';
import USlider from '@nuxt/ui/components/Slider.vue';
import ColorInput from './ColorInput.vue';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function field(value: string, alpha: boolean) {
  const { wrapper } = await mountPage(ColorInput, { value, label: 'Guides', inputId: 'c', alpha });
  current = wrapper;
  return wrapper;
}

async function open(wrapper: VueWrapper): Promise<void> {
  await wrapper.get('[data-testid="color-pick"]').trigger('click');
  await settle();
}

async function close(wrapper: VueWrapper): Promise<void> {
  wrapper.findComponent(UPopover).vm.$emit('update:open', false);
  await settle();
}

// RB-090-58: `UColorPicker` in a popover behind a swatch button, the opacity beside it where the
// setting takes it, the hex field for the keyboard.
describe('a colour field', () => {
  it('names its swatch button and its hex field', async () => {
    const wrapper = await field('#00a2ff59', true);
    const pick = wrapper.get('[data-testid="color-pick"]');
    expect(pick.element.tagName).toBe('BUTTON');
    expect(pick.attributes('aria-label')).toBeTruthy();
    expect(wrapper.get('#c').attributes('aria-label')).toBe('Guides');
    expect((wrapper.get('#c').element as HTMLInputElement).value).toBe('#00a2ff59');
  });

  it('commits a typed value as it is', async () => {
    const wrapper = await field('#00a2ff', false);
    const hex = wrapper.get('#c');
    (hex.element as HTMLInputElement).value = '#123456';
    await hex.trigger('change');
    expect(wrapper.emitted('update')).toEqual([['#123456']]);
  });

  it('commits the picked hue with its opacity once, when the popover closes', async () => {
    const wrapper = await field('#00a2ff59', true);
    await open(wrapper);
    const picker = wrapper.findComponent(UColorPicker);
    expect(picker.exists()).toBe(true);
    expect((picker.props() as Record<string, unknown>).modelValue).toBe('#00a2ff');
    picker.vm.$emit('update:modelValue', '#FF0000');
    picker.vm.$emit('update:modelValue', '#00FF00');
    await settle();
    expect(wrapper.emitted('update')).toBeUndefined();
    await close(wrapper);
    expect(wrapper.emitted('update')).toEqual([['#00ff0059']]);
  });

  it('sets the opacity with the slider where the setting takes one', async () => {
    const wrapper = await field('#00a2ff', true);
    await open(wrapper);
    const slider = wrapper.findComponent(USlider);
    expect((slider.props() as Record<string, unknown>).modelValue).toBe(100);
    slider.vm.$emit('update:modelValue', 50);
    await close(wrapper);
    expect(wrapper.emitted('update')).toEqual([['#00a2ff80']]);
  });

  it('offers no opacity where the setting takes six digits only, and closing unchanged commits nothing', async () => {
    const wrapper = await field('#e53935', false);
    await open(wrapper);
    expect(wrapper.findComponent(UColorPicker).exists()).toBe(true);
    expect(wrapper.findComponent(USlider).exists()).toBe(false);
    await close(wrapper);
    expect(wrapper.emitted('update')).toBeUndefined();
  });
});
