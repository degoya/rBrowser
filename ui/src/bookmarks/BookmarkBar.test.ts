import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import BookmarkBar from './BookmarkBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function mounted(mock: Parameters<typeof mountWithShell>[1] = {}) {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(BookmarkBar, { ...mock, mount: { props: { subscribe } } });
  current = result.wrapper;
  return { ...result, push: (message: CoreToShell) => push(message) };
}

describe('the bookmark bar', () => {
  it('stays closed for other commands', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.assign' });
    await settle();
    expect(wrapper.find('[data-testid="bookmark-bar"]').exists()).toBe(false);
  });

  it('searches, moves the selection with the arrow keys and opens with Enter', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.open' });
    await settle();
    const search = wrapper.get('[data-testid="bookmark-search"]');
    expect(document.activeElement).toBe(search.element);
    await search.setValue('shop');
    expect(wrapper.find('[data-testid="bookmark-match-home"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="bookmark-match-pdp"]').attributes('aria-current')).toBe('true');
    await search.trigger('keydown', { key: 'ArrowDown' });
    expect(wrapper.get('[data-testid="bookmark-match-checkout"]').attributes('aria-current')).toBe('true');
    await search.trigger('keydown', { key: 'Enter' });
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'checkout' } });
  });

  it('says when nothing matches and closes with Escape', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.open' });
    await settle();
    await wrapper.get('[data-testid="bookmark-search"]').setValue('nothing at all');
    expect(wrapper.get('[data-testid="bookmark-none"]').text()).toBe('No bookmark matches');
    await wrapper.get('[data-testid="bookmark-bar"]').trigger('keydown', { key: 'Escape' });
    expect(wrapper.find('[data-testid="bookmark-bar"]').exists()).toBe(false);
  });

  it('proposes the page and saves what was typed, with the switch, to the chosen file', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    const label = wrapper.get<HTMLInputElement>('[data-testid="bookmark-add-label"]');
    expect(document.activeElement).toBe(label.element);
    expect(label.element.value).toBe('Shop');
    expect(wrapper.get<HTMLInputElement>('[data-testid="bookmark-add-target"]').element.value).toBe(
      'http://localhost:5173/',
    );
    await wrapper.get('[data-testid="bookmark-add-target"]').setValue('/checkout');
    await wrapper.get('[data-testid="bookmark-add-group"]').setValue('Shop');
    // "Share with the team" starts on (the project file); off saves for me only.
    const share = wrapper.get('[data-testid="bookmark-add-scope"]');
    expect(share.attributes('aria-checked')).toBe('true');
    await share.trigger('click');
    await wrapper.get('[data-testid="bookmark-add"]').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'bookmarks.add',
      args: { label: 'Shop', path: '/checkout', follow: true, group: 'Shop', scope: 'local' },
    });
  });

  it('shows the path of a relative bookmark and where it leads now', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.open' });
    await settle();
    const relative = wrapper.get('[data-testid="bookmark-match-checkout"]');
    expect(relative.text()).toContain('/checkout');
    expect(relative.attributes('title')).toBe('http://localhost:5173/checkout');
    const absolute = wrapper.get('[data-testid="bookmark-match-storybook"]');
    expect(absolute.text()).not.toContain('6006');
    expect(absolute.attributes('title')).toBe('http://localhost:6006/');
  });

  it('follows the environment by default and leaves the form to the core (RB-080-48)', async () => {
    const state = { ...sampleState(), url: 'https://staging.example.com/shop/cart?x=1#top' };
    const { wrapper, mock, push } = await mounted({ mock: { state } });
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    const target = wrapper.get<HTMLInputElement>('[data-testid="bookmark-add-target"]');
    const follow = wrapper.get('[data-testid="bookmark-add-follow"]');
    // The page as it is: which path it has under the environments is the core's to say.
    expect(target.element.value).toBe('https://staging.example.com/shop/cart?x=1#top');
    expect(follow.attributes('aria-checked')).toBe('true');
    expect(follow.attributes('disabled')).toBeUndefined();
    await wrapper.get('[data-testid="bookmark-add"]').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'bookmarks.add',
      args: { label: 'Shop', url: 'https://staging.example.com/shop/cart?x=1#top', follow: true, scope: 'project' },
    });
    // Switched off, the same target goes with follow: false.
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    await wrapper.get('[data-testid="bookmark-add-follow"]').trigger('click');
    await wrapper.get('[data-testid="bookmark-add"]').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toMatchObject({ args: { follow: false } });
  });

  it('starts with the switch off while bookmarks.followEnvironment is off, also without an environment', async () => {
    const state = { ...sampleState(), environment: null, url: 'http://localhost:5173/shop/cart' };
    const { wrapper, mock, push } = await mounted({
      mock: { state, settings: { 'bookmarks.followEnvironment': false } },
    });
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    const follow = wrapper.get('[data-testid="bookmark-add-follow"]');
    expect(follow.attributes('aria-checked')).toBe('false');
    expect(follow.attributes('disabled')).toBeUndefined();
    await follow.trigger('click');
    await wrapper.get('[data-testid="bookmark-add"]').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'bookmarks.add',
      args: { label: 'Shop', url: 'http://localhost:5173/shop/cart', follow: true, scope: 'project' },
    });
  });

  it('is drawn with Nuxt UI: fields, two named switches and buttons, no popover (RB-080-88)', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.open' });
    await settle();
    expect(wrapper.get('[data-testid="bookmark-search"]').attributes('data-slot')).toBe('base');
    expect(wrapper.get('[data-testid="bookmark-match-pdp"]').attributes('data-slot')).toBe('base');
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    for (const id of ['label', 'target', 'group', 'save']) {
      expect(wrapper.get(`[data-testid="bookmark-add-${id}"]`).attributes('data-slot')).toBe('base');
    }
    const named = (id: string) => {
      const control = wrapper.get(`[data-testid="${id}"]`);
      expect(control.attributes('role')).toBe('switch');
      return wrapper.get(`label[for="${control.attributes('id')}"]`).text();
    };
    expect(named('bookmark-add-follow')).toBe('Follows the environment');
    expect(named('bookmark-add-scope')).toBe('Share with the team (config.json)');
    // A strip view draws nothing beyond its bounds: no select, so no popover (design.md).
    expect(wrapper.find('select').exists()).toBe(false);
    expect(wrapper.find('[role="combobox"]').exists()).toBe(false);
  });

  it('refuses a typed path that names another host', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="bookmark-add-target"]').setValue('//evil.example/login');
    await wrapper.get('[data-testid="bookmark-add"]').trigger('submit');
    await settle();
    expect(mock.sent.length).toBe(before);
    expect(wrapper.get('[data-testid="bookmark-add-target"]').attributes('aria-invalid')).toBe('true');
  });

  it('refuses a target that is neither a path nor a web URL', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'bookmarks.add' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="bookmark-add-target"]').setValue('javascript:alert(1)');
    await wrapper.get('[data-testid="bookmark-add"]').trigger('submit');
    await settle();
    expect(mock.sent.length).toBe(before);
    const target = wrapper.get('[data-testid="bookmark-add-target"]');
    expect(target.attributes('aria-invalid')).toBe('true');
    expect(wrapper.text()).toContain('Enter a path starting with / or an http or https URL');
  });
});
