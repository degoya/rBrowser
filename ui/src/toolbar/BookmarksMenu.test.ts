import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import { mountWithShell, settle } from '../test/mountShell';
import BookmarksMenu from './BookmarksMenu.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the bookmarks menu', () => {
  it('lists the bookmarks grouped and opens the chosen one', async () => {
    const { wrapper, mock } = await mountWithShell(BookmarksMenu);
    current = wrapper;
    const select = wrapper.get<HTMLSelectElement>('[data-testid="bookmarks-select"]');
    expect(select.attributes('disabled')).toBeUndefined();
    expect(wrapper.findAll('optgroup').map((group) => group.attributes('label'))).toEqual(['Shop', 'Tools']);
    // A bookmark that follows the environment shows its path, the tooltip where it leads now.
    const shop = wrapper.findAll('optgroup')[0]?.findAll('option') ?? [];
    expect(shop.map((o) => o.text())).toEqual(['Product - /products/42', 'Checkout - /checkout']);
    expect(shop.map((o) => o.attributes('title'))).toEqual([
      'http://localhost:5173/products/42',
      'http://localhost:5173/checkout',
    ]);
    const tools = wrapper.findAll('optgroup')[1]?.findAll('option') ?? [];
    expect(tools.map((o) => [o.text(), o.attributes('title')])).toEqual([['Storybook', 'http://localhost:6006/']]);
    await select.setValue('checkout');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'checkout' } });
    expect(select.element.value).toBe('');
  });

  it('follows the state: a bookmark set a moment ago is in the list', async () => {
    const { wrapper, mock } = await mountWithShell(BookmarksMenu);
    current = wrapper;
    mock.state.bookmarks = [
      ...mock.state.bookmarks,
      { id: 'mine', label: 'Just set', url: 'http://localhost:5173/mine' },
    ];
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    const select = wrapper.get<HTMLSelectElement>('[data-testid="bookmarks-select"]');
    expect(select.findAll('option').map((o) => o.text())).toContain('Just set');
    await select.setValue('mine');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'mine' } });
    mock.state.bookmarks = [];
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(select.attributes('disabled')).toBeDefined();
  });

  it('opens nothing while the keyboard browses the closed select (RB-090-23)', async () => {
    const { wrapper, mock } = await mountWithShell(BookmarksMenu);
    current = wrapper;
    const element = wrapper.get('[data-testid="bookmarks-select"]').element as HTMLSelectElement;
    element.focus();
    const before = mock.sent.length;
    for (const [key, value] of [
      ['ArrowDown', 'pdp'],
      ['ArrowDown', 'checkout'],
      ['s', 'storybook'],
    ] as const) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      expect(down.defaultPrevented, key).toBe(true);
      element.value = value;
      element.dispatchEvent(new Event('change', { bubbles: true }));
      await settle();
      element.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
      expect(element.value).toBe('');
    }
    expect(mock.sent).toHaveLength(before);
    await wrapper.get('[data-testid="bookmarks-select"]').trigger('pointerdown');
    await wrapper.get('[data-testid="bookmarks-select"]').setValue('checkout');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'checkout' } });
  });

  it('asks the core for the dialog from "Bookmark this page"', async () => {
    const { wrapper, mock } = await mountWithShell(BookmarksMenu);
    current = wrapper;
    const button = wrapper.get('[data-testid="bookmarks-add"]');
    expect(button.attributes('aria-label')).toBe('Bookmark this page');
    await button.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.add' });
  });

  it('is disabled without bookmarks', async () => {
    const { wrapper } = await mountWithShell(BookmarksMenu, { mock: { state: { ...sampleState(), bookmarks: [] } } });
    current = wrapper;
    const select = wrapper.get('[data-testid="bookmarks-select"]');
    expect(select.attributes('disabled')).toBeDefined();
    expect(select.text()).toContain('No bookmarks');
  });
});
