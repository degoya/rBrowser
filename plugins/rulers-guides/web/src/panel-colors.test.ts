// The panel's colour fields (RB-090-58): the value arithmetic and the fields in the panel.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import type { VueWrapper } from '@vue/test-utils';
import { mount } from '@vue/test-utils';
import ui from '@nuxt/ui/vue-plugin';
import UColorPicker from '@nuxt/ui/components/ColorPicker.vue';
import UPopover from '@nuxt/ui/components/Popover.vue';
import USlider from '@nuxt/ui/components/Slider.vue';
import { colorArg, COLOR_KEYS, hueOf, isColor, opacityOf, withOpacity, withRgb } from './panel/colors';
import ColorField from './panel/ColorField.vue';

describe('a colour value', () => {
  it('is six or eight hex digits', () => {
    expect(isColor('#00a2ff')).toBe(true);
    expect(isColor('#00A2FF59')).toBe(true);
    for (const bad of ['', '#0af', 'red', '#00a2ff5', '#00a2ff;}', 7, null]) expect(isColor(bad), String(bad)).toBe(false);
  });

  it('keeps the opacity when the hue changes and the hue when the opacity does', () => {
    expect(hueOf('#00A2FF59')).toBe('#00a2ff');
    expect(withRgb('#00a2ff59', '#FF0000')).toBe('#ff000059');
    expect(withRgb('#00a2ff', '#FF0000')).toBe('#ff0000');
    expect(withRgb('#00a2ff', 'nonsense')).toBe('#00a2ff');
    expect(opacityOf('#00a2ff59')).toBe(35);
    expect(withOpacity('#00a2ff', 35)).toBe('#00a2ff59');
    expect(withOpacity('#00a2ff59', 100)).toBe('#00a2ff');
    expect(withOpacity('#00a2ff', 0)).toBe('#00a2ff00');
  });

  it('is set as one colour of `set`', () => {
    expect(COLOR_KEYS).toEqual(['ruler', 'ticks', 'guide', 'columnFill', 'columnEdge', 'baseline']);
    expect(colorArg('columnEdge', '#ff4081')).toEqual({ colors: { columnEdge: '#ff4081' } });
  });
});

const settle = async (): Promise<void> => {
  for (let round = 0; round < 3; round += 1) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
  }
};

let wrapper: VueWrapper | null = null;

beforeEach(() => {
  document.body.innerHTML = '';
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  vi.restoreAllMocks();
});

async function field(value: string): Promise<VueWrapper> {
  wrapper = mount(ColorField, {
    props: { value, colorKey: 'guide', name: 'Guides', pickLabel: 'Pick the colour: Guides', hexLabel: 'Guides, hex', opacityLabel: 'Guides, opacity', invalidText: 'Not a colour.' },
    attachTo: document.body,
    global: { plugins: [ui] },
  });
  await settle();
  return wrapper;
}

describe('a colour field of the panel', () => {
  it('applies a typed hex colour and refuses what is none', async () => {
    const view = await field('#00a2ff');
    const hex = view.get('#color-guide');
    expect(hex.attributes('aria-label')).toBe('Guides, hex');
    (hex.element as HTMLInputElement).value = 'blue';
    await hex.trigger('change');
    expect(view.emitted('update')).toBeUndefined();
    expect(hex.attributes('aria-invalid')).toBe('true');
    expect(view.get('#color-guide-invalid').text()).toBe('Not a colour.');
    (hex.element as HTMLInputElement).value = '#FF00FF80';
    await hex.trigger('change');
    expect(view.emitted('update')).toEqual([['#FF00FF80']]);
  });

  it('applies the picked hue and opacity once, when the popover closes', async () => {
    const view = await field('#00a2ff');
    const pick = view.get('#color-guide-pick');
    expect(pick.attributes('aria-label')).toBe('Pick the colour: Guides');
    await pick.trigger('click');
    await settle();
    view.findComponent(UColorPicker).vm.$emit('update:modelValue', '#FF0000');
    view.findComponent(USlider).vm.$emit('update:modelValue', 50);
    await settle();
    expect(view.emitted('update')).toBeUndefined();
    view.findComponent(UPopover).vm.$emit('update:open', false);
    await settle();
    expect(view.emitted('update')).toEqual([['#ff000080']]);
  });
});
