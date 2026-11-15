// The bars over the toolbar strip hand the focus back to the control that had it when they
// opened, once Escape, Cancel or done closes them (RB-090-23); closing because the focus left
// them leaves it where it went.
import { afterEach, describe, expect, it } from 'vitest';
import type { Component } from 'vue';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import BookmarkBar from '../bookmarks/BookmarkBar.vue';
import SizeInputBar from '../breakpoint_keys/SizeInputBar.vue';
import SavePresetBar from '../layout/SavePresetBar.vue';
import SessionBar from '../sessions/SessionBar.vue';
import SitemapBar from '../sitemap/SitemapBar.vue';
import NewSizeBar from '../toolbar/NewSizeBar.vue';

let current: VueWrapper | null = null;
let opener: HTMLButtonElement | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  opener?.remove();
  opener = null;
});

async function openBar(component: Component, command: string, testid: string) {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const { wrapper } = await mountWithShell(component, { mount: { props: { subscribe } } });
  current = wrapper;
  opener = document.createElement('button');
  opener.textContent = 'opener';
  document.body.append(opener);
  opener.focus();
  push({ type: 'uiCommand', id: command });
  await settle();
  const bar = wrapper.get(`[data-testid="${testid}"]`);
  expect(bar.element.contains(document.activeElement)).toBe(true);
  return { wrapper, bar };
}

const BARS: [string, Component, string, string][] = [
  ['save preset', SavePresetBar, 'layout.save_preset', 'save-preset'],
  ['bookmarks', BookmarkBar, 'bookmarks.open', 'bookmark-bar'],
  ['session', SessionBar, 'session.assign', 'session-bar'],
  ['sitemap', SitemapBar, 'panels.sitemap', 'sitemap-bar'],
  ['new size', NewSizeBar, 'view.add_size.custom', 'new-size'],
  ['size input', SizeInputBar, 'view.set_size', 'size-input'],
];

describe('the focus after a bar closes', () => {
  it.each(BARS)('the %s bar hands it back to its opener on Escape', async (_name, component, command, testid) => {
    const { wrapper } = await openBar(component, command, testid);
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    await settle();
    expect(wrapper.find(`[data-testid="${testid}"]`).exists()).toBe(false);
    expect(document.activeElement).toBe(opener);
  });

  it.each([
    ['save preset', SavePresetBar, 'layout.save_preset', 'save-preset', 'save-preset-cancel'],
    ['session', SessionBar, 'session.assign', 'session-bar', 'session-cancel'],
    ['sitemap', SitemapBar, 'panels.sitemap', 'sitemap-bar', 'sitemap-close'],
  ] as const)('the %s bar hands it back on its close button', async (_name, component, command, testid, close) => {
    const { wrapper } = await openBar(component, command, testid);
    await wrapper.get(`[data-testid="${close}"]`).trigger('click');
    await settle();
    expect(document.activeElement).toBe(opener);
  });

  it('leaves the focus where it went when leaving the size field closes the bar', async () => {
    const { wrapper } = await openBar(SizeInputBar, 'view.set_size', 'size-input');
    const elsewhere = document.createElement('button');
    document.body.append(elsewhere);
    elsewhere.focus();
    await settle();
    expect(wrapper.find('[data-testid="size-input"]').exists()).toBe(false);
    expect(document.activeElement).toBe(elsewhere);
    elsewhere.remove();
  });
});
