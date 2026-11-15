import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import NativeSelect from './NativeSelect.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function setup() {
  const wrapper = mount(NativeSelect, {
    props: {
      label: 'Environment',
      emptyLabel: 'No environments',
      noneLabel: 'None',
      options: [
        { value: 'local', label: 'Local' },
        { value: 'stage', label: 'Stage' },
      ],
      modelValue: 'local',
    },
    attachTo: document.body,
  });
  current = wrapper;
  return wrapper;
}

describe('the native select of the toolbar (EnvSwitch, LayoutSwitch)', () => {
  it('chooses nothing while the keyboard browses the closed select (RB-090-23)', async () => {
    const wrapper = setup();
    const element = wrapper.get('select').element as HTMLSelectElement;
    element.focus();
    // What a closed select does on Windows for an arrow key or a typed letter: the next option
    // and `change`, unless the key was held back. No popup here (no `showPicker`).
    for (const [key, value] of [
      ['ArrowDown', 'stage'],
      ['ArrowUp', 'local'],
      ['s', 'stage'],
    ] as const) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      expect(down.defaultPrevented, key).toBe(true);
      element.value = value;
      element.dispatchEvent(new Event('change', { bubbles: true }));
      element.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
      expect(element.value).toBe('');
    }
    expect(wrapper.emitted('update:modelValue')).toBeUndefined();
    // A choice from the list is a choice.
    await wrapper.get('select').trigger('pointerdown');
    await wrapper.get('select').setValue('stage');
    expect(wrapper.emitted('update:modelValue')).toEqual([['stage']]);
  });

  it('opens its list on an arrow, Enter or Space, and not on Tab or Escape', () => {
    const wrapper = setup();
    const element = wrapper.get('select').element as HTMLSelectElement;
    const showPicker = vi.fn();
    element.showPicker = showPicker;
    for (const key of ['ArrowDown', 'Enter', ' ']) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      expect(down.defaultPrevented, key).toBe(true);
    }
    for (const key of ['Tab', 'Escape']) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
    expect(showPicker).toHaveBeenCalledTimes(3);
    expect(wrapper.emitted('update:modelValue')).toBeUndefined();
  });
});
