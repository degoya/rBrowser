// RB-080-96: in a narrow settings window the fixed 256 px navigation left the content about 40 px
// for its tab row. Below `NARROW_SETTINGS` the categories become a choice above the content, the
// search and "Only changed" beside it, and the content takes the whole width.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import USelect from '@nuxt/ui/components/Select.vue';
import SettingsPage from './SettingsPage.vue';
import { NARROW_SETTINGS } from './narrow';
import { createFakeCore } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;
const wideWidth = window.innerWidth;

async function open(props: Record<string, unknown> = {}) {
  const mounted = await mountPage(SettingsPage, { routeKey: null, ...props }, createFakeCore());
  current = mounted.wrapper;
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
  window.innerWidth = wideWidth;
});

function categorySelect(wrapper: VueWrapper): VueWrapper {
  return wrapper.get('[data-testid="settings-nav-bar"]').findComponent(USelect) as VueWrapper;
}

describe('the settings window at 400 px (RB-080-96)', () => {
  it('drops the fixed side navigation and offers the categories as a choice above the content', async () => {
    const { wrapper } = await open({ routeKey: 'layout', routeTab: 'grid', pageWidth: 400 });
    // The fixed 256 px column is gone; the page stacks the bar over the content.
    expect(wrapper.find('aside.w-64').exists()).toBe(false);
    expect(wrapper.find('[data-testid="settings-nav"]').exists()).toBe(false);
    const bar = wrapper.get('[data-testid="settings-nav-bar"]');
    expect(bar.element.closest('[data-testid="settings-main"]')).toBeNull();
    const select = wrapper.findComponent(USelect);
    expect(select.exists()).toBe(true);
    const items = (select.props() as Record<string, unknown>).items as { label: string; value: string }[];
    expect(items).toHaveLength(18);
    expect(items.find((item) => item.value === 'sync')?.label).toBe('Sync');
    expect((select.props() as Record<string, unknown>).modelValue).toBe('layout');
    expect(bar.find('[data-testid="search"]').exists()).toBe(true);
    expect(bar.find('[data-testid="only-changed"]').exists()).toBe(true);
    // The tab row stays, in the full width of the content.
    expect(wrapper.find('[data-testid="settings-tabs"]').exists()).toBe(true);
  });

  it('opens a category chosen in the select', async () => {
    const { wrapper } = await open({ pageWidth: 400 });
    categorySelect(wrapper).vm.$emit('update:modelValue', 'sync');
    await settle();
    expect((wrapper.findComponent(USelect).props() as Record<string, unknown>).modelValue).toBe('sync');
    expect(wrapper.get('[data-testid="settings-main"] h1').text()).toBe('Sync');
  });

  it('keeps the side navigation in a wide window and follows a resize', async () => {
    const { wrapper } = await open();
    expect(wrapper.find('[data-testid="settings-nav"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="settings-nav-bar"]').exists()).toBe(false);
    window.innerWidth = NARROW_SETTINGS - 1;
    window.dispatchEvent(new Event('resize'));
    await settle();
    expect(wrapper.find('[data-testid="settings-nav"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="settings-nav-bar"]').exists()).toBe(true);
    window.innerWidth = NARROW_SETTINGS;
    window.dispatchEvent(new Event('resize'));
    await settle();
    expect(wrapper.find('[data-testid="settings-nav"]').exists()).toBe(true);
  });
});
