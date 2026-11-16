import { afterEach, describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { h } from 'vue';
import { createShellI18n } from '../i18n';
import type { JsonValue } from '../ipc/protocol';
import CollapsibleList from './CollapsibleList.vue';
import { copyItem } from './listOps';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const ITEMS: JsonValue[] = [
  { id: 'mobile', width: 375 },
  { id: 'tablet', width: 768 },
  { id: 'desktop', width: 1280 },
];

function open(items: JsonValue[] = ITEMS) {
  const wrapper = mount(CollapsibleList, {
    props: {
      items,
      label: 'Breakpoints',
      fresh: () => ({ id: '' }),
      copy: (item: JsonValue, all: readonly JsonValue[]) => copyItem(item, all),
      summary: (item: JsonValue) => String((item as { id: string }).id),
    },
    slots: {
      item: ({ item }: { item: JsonValue }) => h('p', { 'data-testid': 'editor' }, JSON.stringify(item)),
    },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  return wrapper;
}

function updates(wrapper: VueWrapper): JsonValue[][] {
  return (wrapper.emitted('update') ?? []).map((args) => args[0] as JsonValue[]);
}

function ids(items: JsonValue[] | undefined): string[] {
  return (items ?? []).map((item) => (item as { id: string }).id);
}

describe('a list of objects as collapsible one-line entries (RB-080-13)', () => {
  it('shows every entry closed as its summary and opens one with Enter or a click', async () => {
    const wrapper = open();
    const toggles = wrapper.findAll('[data-testid="list-toggle"]');
    expect(toggles.map((toggle) => toggle.text())).toEqual(['mobile', 'tablet', 'desktop']);
    expect(toggles.every((toggle) => toggle.attributes('aria-expanded') === 'false')).toBe(true);
    expect(wrapper.find('[data-testid="editor"]').exists()).toBe(false);
    await toggles[1]!.trigger('click');
    expect(toggles[1]!.attributes('aria-expanded')).toBe('true');
    const region = wrapper.get(`#${toggles[1]!.attributes('aria-controls')}`);
    expect(region.attributes('role')).toBe('region');
    expect(region.get('[data-testid="editor"]').text()).toContain('tablet');
    expect(toggles[1]!.element.tagName).toBe('BUTTON');
  });

  it('adds an entry and opens it', async () => {
    const wrapper = open();
    await wrapper.get('[data-testid="list-add"]').trigger('click');
    const added = updates(wrapper)[0];
    expect(ids(added)).toEqual(['mobile', 'tablet', 'desktop', '']);
    await wrapper.setProps({ items: added });
    const toggles = wrapper.findAll('[data-testid="list-toggle"]');
    expect(toggles[3]!.attributes('aria-expanded')).toBe('true');
  });

  it('duplicates an entry after itself with an id of its own', async () => {
    const wrapper = open();
    await wrapper.findAll('[data-testid="list-toggle"]')[0]!.trigger('click');
    await wrapper.get('[data-testid="list-duplicate"]').trigger('click');
    expect(ids(updates(wrapper)[0])).toEqual(['mobile', 'mobile-copy', 'tablet', 'desktop']);
  });

  it('deletes only after the confirmation in the entry, and keeps it on cancel', async () => {
    const wrapper = open();
    await wrapper.findAll('[data-testid="list-toggle"]')[1]!.trigger('click');
    await wrapper.get('[data-testid="list-remove"]').trigger('click');
    expect(updates(wrapper)).toEqual([]);
    expect(wrapper.get('[data-testid="list-confirm"]').text()).toContain('Delete this entry?');
    await wrapper.get('[data-testid="list-remove-cancel"]').trigger('click');
    expect(wrapper.find('[data-testid="list-confirm"]').exists()).toBe(false);
    await wrapper.get('[data-testid="list-remove"]').trigger('click');
    await wrapper.get('[data-testid="list-remove-confirm"]').trigger('click');
    expect(ids(updates(wrapper)[0])).toEqual(['mobile', 'desktop']);
  });

  it('moves an entry with its buttons, with Alt+Arrow on its line, and by dragging', async () => {
    const wrapper = open();
    await wrapper.findAll('[data-testid="list-down"]')[0]!.trigger('click');
    expect(ids(updates(wrapper)[0])).toEqual(['tablet', 'mobile', 'desktop']);
    await wrapper.findAll('[data-testid="list-toggle"]')[2]!.trigger('keydown', { key: 'ArrowUp', altKey: true });
    expect(ids(updates(wrapper)[1])).toEqual(['mobile', 'desktop', 'tablet']);
    await wrapper.findAll('[data-testid="list-drag"]')[0]!.trigger('dragstart');
    await wrapper.findAll('[data-testid="list-item"]')[2]!.trigger('drop');
    expect(ids(updates(wrapper)[2])).toEqual(['tablet', 'desktop', 'mobile']);
    expect(wrapper.findAll('[data-testid="list-up"]')[0]!.attributes('disabled')).toBeDefined();
  });

  it('keeps an open entry open when it moves', async () => {
    const wrapper = open();
    await wrapper.findAll('[data-testid="list-toggle"]')[0]!.trigger('click');
    await wrapper.findAll('[data-testid="list-down"]')[0]!.trigger('click');
    await wrapper.setProps({ items: updates(wrapper)[0] });
    const toggles = wrapper.findAll('[data-testid="list-toggle"]');
    expect(toggles.map((toggle) => toggle.attributes('aria-expanded'))).toEqual(['false', 'true', 'false']);
  });

  it('shows an empty state with the add button', async () => {
    const wrapper = open([]);
    expect(wrapper.get('[data-testid="list-empty"]').text()).toBe('No entries.');
    expect(wrapper.find('[data-testid="list-add"]').exists()).toBe(true);
  });
});
