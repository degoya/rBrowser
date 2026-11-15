// The focus in a list of strings (RB-090-23): a moved entry keeps it on the button that moved it,
// or on the other arrow when that one disabled itself at the end; a removed last entry hands it to
// the entry before, or to "Add".
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { defineComponent, h, shallowRef } from 'vue';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import type { JsonValue } from '../ipc/protocol';
import { settle } from '../test/mountShell';
import ListControl from './ListControl.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function setup(start: string[]) {
  // shallowRef: unwrapping the recursive JsonValue runs the type checker too deep.
  const items = shallowRef<JsonValue[]>(start);
  const Host = defineComponent(() => () =>
    h(
      ListControl,
      { items: items.value, label: 'Paths', fresh: () => '', onUpdate: (next: JsonValue[]) => (items.value = next) },
      { item: ({ item }: { item: JsonValue }) => h('span', String(item)) },
    ),
  );
  current = mount(Host, { attachTo: document.body, global: { plugins: [ui, createShellI18n()] } });
  return { wrapper: current, items };
}

function button(wrapper: VueWrapper, label: string): HTMLElement {
  return wrapper.get(`[aria-label="${label}"]`).element as HTMLElement;
}

describe('the focus in a list of strings', () => {
  it('follows the moved entry, and goes to the other arrow at either end', async () => {
    const { wrapper, items } = setup(['a', 'b', 'c']);
    button(wrapper, 'Move entry 2 up').focus();
    await wrapper.get('[aria-label="Move entry 2 up"]').trigger('click');
    await settle();
    expect(items.value).toEqual(['b', 'a', 'c']);
    // At the top Up disabled itself: Down of the same entry has the focus.
    expect(document.activeElement).toBe(button(wrapper, 'Move entry 1 down'));

    await wrapper.get('[aria-label="Move entry 1 down"]').trigger('click');
    await settle();
    expect(items.value).toEqual(['a', 'b', 'c']);
    expect(document.activeElement).toBe(button(wrapper, 'Move entry 2 down'));
  });

  it('hands the focus on when the last entry is removed', async () => {
    const { wrapper, items } = setup(['a', 'b']);
    const removes = () => wrapper.findAll('[data-testid="list-remove"]');
    (removes()[1]?.element as HTMLElement).focus();
    await removes()[1]?.trigger('click');
    await settle();
    expect(items.value).toEqual(['a']);
    expect(document.activeElement).toBe(removes()[0]?.element);
    await removes()[0]?.trigger('click');
    await settle();
    expect(items.value).toEqual([]);
    expect(document.activeElement).toBe(wrapper.get('[data-testid="list-add"]').element);
  });
});
