// The toolbar's bookmarks (RB-070-06) as a pulldown whose menu the core draws (RB-090-45): grouped
// under headings, a bookmark without a URL shown but not chosen, a long list cut with a last row
// that opens the search, "Bookmark this page" beside it.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { BookmarkSummary, ShellToCore } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
import { MAX_ROWS, MAX_TEXT } from './nativeMenu';
import BookmarksMenu from './BookmarksMenu.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const BUTTON = '[data-testid="bookmarks-menu"] button';

async function setup(bookmarks?: BookmarkSummary[]) {
  const state = sampleState();
  if (bookmarks !== undefined) {
    state.bookmarks = bookmarks;
  }
  const mounted = await mountWithShell(BookmarksMenu, { mock: { state } });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

async function open(wrapper: VueWrapper, sent: readonly ShellToCore[]) {
  await wrapper.get(BUTTON).trigger('click');
  await settle();
  return lastNativeMenu(sent).rows;
}

describe('the bookmarks menu', () => {
  it('lists the bookmarks grouped under headings and opens the chosen one', async () => {
    const { wrapper, mock } = await setup();
    const button = wrapper.get(BUTTON);
    expect(button.attributes('disabled')).toBeUndefined();
    expect(button.attributes('aria-haspopup')).toBe('menu');
    expect(button.text()).toBe('Bookmarks');
    expect(wrapper.find('select').exists()).toBe(false);
    const rows = await open(wrapper, mock.sent);
    expect(lastNativeMenu(mock.sent).menu).toMatch(/^bookmarks-\d+$/);
    // Those without a group first; a bookmark that follows the environment shows its path; a
    // line between two groups.
    expect(rows).toEqual([
      { kind: 'item', id: 'open:0', label: 'Home - /', disabled: false },
      { kind: 'separator' },
      { kind: 'heading', label: 'Shop' },
      { kind: 'item', id: 'open:1', label: 'Product - /products/42', disabled: false },
      { kind: 'item', id: 'open:2', label: 'Checkout - /checkout', disabled: false },
      { kind: 'separator' },
      { kind: 'heading', label: 'Tools' },
      { kind: 'item', id: 'open:3', label: 'Storybook', disabled: false },
    ]);
    await closeNativeMenu(mock.sent, 'open:2');
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'checkout' } });
    expect(document.activeElement).toBe(button.element);
  });

  it('follows the state: a bookmark set a moment ago is in the list, one without a URL cannot be chosen', async () => {
    const { wrapper, mock } = await setup();
    mock.state.bookmarks = [
      ...mock.state.bookmarks,
      { id: 'mine', label: 'Just set', url: 'http://localhost:5173/mine' },
      { id: 'lost', label: 'No URL here' },
    ];
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    const rows = await open(wrapper, mock.sent);
    expect(rows).toContainEqual({ kind: 'item', id: 'open:1', label: 'Just set', disabled: false });
    expect(rows).toContainEqual({ kind: 'item', id: 'open:2', label: 'No URL here', disabled: true });
    await closeNativeMenu(mock.sent, 'open:1');
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'mine' } });
    mock.state.bookmarks = [];
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(wrapper.get(BUTTON).attributes('disabled')).toBeDefined();
  });

  // RB-090-45: the core takes at most 200 rows, and hundreds of rows are no way to find one -
  // the list is cut, and its last row opens the quick pick that searches them all.
  it('cuts a long list with a last row that opens the search', async () => {
    const many: BookmarkSummary[] = Array.from({ length: 250 }, (_, index) => ({
      id: `b${index}`,
      label: `Page ${index}`,
      url: `http://localhost:5173/${index}`,
    }));
    const { wrapper, mock } = await setup(many);
    const rows = await open(wrapper, mock.sent);
    expect(rows.length).toBeLessThan(MAX_ROWS);
    const items = rows.filter((row) => row.kind === 'item');
    const last = rows.at(-1);
    expect(last).toEqual({ kind: 'item', id: 'search', label: `${250 - (items.length - 1)} more bookmarks - search them all` });
    await closeNativeMenu(mock.sent, 'search');
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open' });
  });

  // RB-090-54: the core refuses a menu with a row id over MAX_TEXT characters, so a row carries
  // the bookmark's place in the menu, never its id - a long id cannot break the whole menu.
  it('opens a bookmark whose id is longer than the core takes for a row id', async () => {
    const long = `b-${'x'.repeat(MAX_TEXT + 50)}`;
    const { wrapper, mock } = await setup([
      { id: 'short', label: 'Short', url: 'http://localhost:5173/short' },
      { id: long, label: 'Long', url: 'http://localhost:5173/long' },
    ]);
    const rows = await open(wrapper, mock.sent);
    const ids = rows.flatMap((row) => (row.kind === 'item' ? [row.id] : []));
    expect(ids).toEqual(['open:0', 'open:1']);
    expect(ids.every((id) => id.length <= MAX_TEXT)).toBe(true);
    await closeNativeMenu(mock.sent, 'open:1');
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: long } });
  });

  it('opens nothing for a row id it did not show', async () => {
    const { wrapper, mock } = await setup();
    await open(wrapper, mock.sent);
    const before = mock.sent.length;
    await closeNativeMenu(mock.sent, 'open:99');
    expect(mock.sent).toHaveLength(before);
  });

  it('opens nothing on Esc', async () => {
    const { wrapper, mock } = await setup();
    await open(wrapper, mock.sent);
    const before = mock.sent.length;
    await closeNativeMenu(mock.sent);
    expect(mock.sent).toHaveLength(before);
  });

  it('asks the core for the dialog from "Bookmark this page"', async () => {
    const { wrapper, mock } = await setup();
    const button = wrapper.get('[data-testid="bookmarks-add"]');
    expect(button.attributes('aria-label')).toBe('Bookmark this page');
    await button.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.add' });
  });

  it('is disabled without bookmarks', async () => {
    const { wrapper } = await setup([]);
    const button = wrapper.get(BUTTON);
    expect(button.attributes('disabled')).toBeDefined();
    expect(button.text()).toBe('No bookmarks');
  });
});
