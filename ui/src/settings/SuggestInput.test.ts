// RB-080-88: a free text with proposals is a Nuxt UI `UInputMenu` with free entry that commits
// like the native field it replaced - on Enter, on leaving it, on a chosen proposal - and never
// per keystroke.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import UInputMenu from '@nuxt/ui/components/InputMenu.vue';
import SuggestInput from './SuggestInput.vue';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const ITEMS = [
  { value: 'Win32', label: 'Win32' },
  { value: 'MacIntel', label: 'MacIntel' },
];

async function field(value = '') {
  const { wrapper } = await mountPage(SuggestInput, {
    value,
    items: ITEMS,
    inputId: 's',
    label: 'Platform',
    describedBy: 's-description',
  });
  current = wrapper;
  return wrapper;
}

function menu(wrapper: VueWrapper) {
  return wrapper.findComponent(UInputMenu);
}

/** Types `text` as the menu reports it: its model follows every keystroke in autocomplete. */
async function type(wrapper: VueWrapper, text: string): Promise<void> {
  menu(wrapper).vm.$emit('update:modelValue', text);
  await settle();
}

describe('a free text with proposals', () => {
  it('is an autocomplete menu that offers the proposals and names the field', async () => {
    const wrapper = await field('Win32');
    const props = menu(wrapper).props() as Record<string, unknown>;
    expect(props.mode).toBe('autocomplete');
    expect(props.items).toEqual(ITEMS);
    const input = wrapper.get('#s');
    expect(input.attributes('aria-label')).toBe('Platform');
    expect(input.attributes('aria-describedby')).toBe('s-description');
    expect((input.element as HTMLInputElement).value).toBe('Win32');
  });

  it('commits a typed text on leaving the field, not per keystroke', async () => {
    const wrapper = await field();
    await type(wrapper, 'Lin');
    await type(wrapper, 'Linux armv81');
    expect(wrapper.emitted('commit')).toBeUndefined();
    menu(wrapper).vm.$emit('blur', new FocusEvent('blur'));
    await settle();
    expect(wrapper.emitted('commit')).toEqual([['Linux armv81']]);
  });

  it('commits on Enter once, however often the field is left after', async () => {
    const wrapper = await field();
    await type(wrapper, 'MacIntel');
    await wrapper.get('#s').trigger('keydown', { key: 'Enter' });
    await settle();
    menu(wrapper).vm.$emit('blur', new FocusEvent('blur'));
    await settle();
    expect(wrapper.emitted('commit')).toEqual([['MacIntel']]);
  });

  it('commits nothing when the text did not change', async () => {
    const wrapper = await field('Win32');
    menu(wrapper).vm.$emit('blur', new FocusEvent('blur'));
    await settle();
    expect(wrapper.emitted('commit')).toBeUndefined();
  });

  it('shows a value set from outside and commits from there', async () => {
    const wrapper = await field('Win32');
    await wrapper.setProps({ value: 'MacIntel' });
    await settle();
    expect((wrapper.get('#s').element as HTMLInputElement).value).toBe('MacIntel');
    menu(wrapper).vm.$emit('blur', new FocusEvent('blur'));
    await settle();
    expect(wrapper.emitted('commit')).toBeUndefined();
  });
});
