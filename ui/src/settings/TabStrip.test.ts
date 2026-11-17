import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { defineComponent, h, ref } from 'vue';
import { createShellI18n } from '../i18n';
import { settle } from './test/mountPage';
import TabStrip from './TabStrip.vue';

const ITEMS = Array.from({ length: 15 }, (_, i) => ({ label: `Group ${i + 1}`, value: `g${i + 1}` }));

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

/** Gives the row the box a layout would: `clientWidth` px visible of `scrollWidth`, each tab 100 px. */
function layOut(wrapper: VueWrapper, clientWidth: number, scrollWidth: number): HTMLElement {
  const list = wrapper.get('[role="tablist"]').element as HTMLElement;
  Object.defineProperty(list, 'clientWidth', { configurable: true, value: clientWidth });
  Object.defineProperty(list, 'scrollWidth', { configurable: true, value: scrollWidth });
  list.querySelectorAll<HTMLElement>('[role="tab"]').forEach((tab, index) => {
    Object.defineProperty(tab, 'offsetLeft', { configurable: true, value: index * 100 });
    Object.defineProperty(tab, 'offsetWidth', { configurable: true, value: 100 });
  });
  return list;
}

async function mountStrip(initial = 'g1', variant: 'pill' | 'link' = 'pill') {
  const model = ref(initial);
  const Host = defineComponent({
    setup: () => () =>
      h(TabStrip, {
        modelValue: model.value,
        'onUpdate:modelValue': (value: string | number | undefined) => (model.value = String(value)),
        items: ITEMS,
        variant,
        'data-testid': 'strip',
      }),
  });
  const wrapper = mount(Host, { attachTo: document.body, global: { plugins: [ui, createShellI18n()] } });
  await settle();
  current = wrapper;
  return { wrapper, model };
}

describe('TabStrip, the one tab row of the pages window (RB-080-81)', () => {
  it('keeps every tab in one row that scrolls, never a second row', async () => {
    const { wrapper } = await mountStrip();
    const list = wrapper.get('[data-testid="strip"] [role="tablist"]');
    expect(list.classes()).toContain('flex-nowrap');
    expect(list.classes()).toContain('overflow-x-auto');
    expect(list.classes()).not.toContain('flex-wrap');
    for (const tab of wrapper.findAll('[role="tab"]')) {
      expect(tab.classes()).toContain('shrink-0');
    }
  });

  // Owner, 2026-11-16 (RB-080-88): a vertical scroll bar with arrows beside every tab row on
  // Windows. The list hides the vertical overflow, and the link variant's line stays inside it.
  it.each(['pill', 'link'] as const)('never scrolls vertically (%s)', async (variant) => {
    const { wrapper } = await mountStrip('g1', variant);
    const list = wrapper.get('[data-testid="strip"] [role="tablist"]');
    expect(list.classes()).toContain('overflow-y-hidden');
    expect(list.classes()).not.toContain('overflow-y-auto');
    if (variant === 'link') {
      expect(list.classes()).not.toContain('-mb-px');
      expect(list.classes()).not.toContain('border-b');
      const tab = wrapper.get('[role="tab"]');
      expect(tab.classes()).toContain('after:-bottom-1');
      expect(tab.classes()).not.toContain('after:-bottom-[calc(var(--spacing)+1px)]');
    }
  });

  it('shows no scroll buttons when every tab fits (wide window)', async () => {
    const { wrapper } = await mountStrip();
    layOut(wrapper, 1600, 1500);
    wrapper.get('[role="tablist"]').element.dispatchEvent(new Event('scroll'));
    await settle();
    expect(wrapper.find('[data-testid="tabs-back"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="tabs-forward"]').exists()).toBe(false);
  });

  it('offers the side with more tabs at 400 and 800 px and pages there', async () => {
    for (const width of [400, 800]) {
      const { wrapper } = await mountStrip();
      const list = layOut(wrapper, width, 1500);
      list.scrollLeft = 0;
      list.dispatchEvent(new Event('scroll'));
      await settle();
      const back = wrapper.get('[data-testid="tabs-back"]');
      const forward = wrapper.get('[data-testid="tabs-forward"]');
      // The back button keeps its place, hidden, so the row does not jump while it scrolls.
      expect(back.classes()).toContain('invisible');
      expect(forward.classes()).not.toContain('invisible');
      expect(forward.attributes('aria-label')).toBe('Show more tabs');
      await forward.trigger('click');
      await settle();
      expect(list.scrollLeft).toBe(Math.round(width * 0.8));
      expect(wrapper.get('[data-testid="tabs-back"]').classes()).not.toContain('invisible');
      current?.unmount();
      current = null;
    }
  });

  it('scrolls the chosen tab into sight, also when the keyboard moves to it', async () => {
    const { wrapper, model } = await mountStrip();
    const list = layOut(wrapper, 400, 1500);
    list.scrollLeft = 0;
    model.value = 'g12';
    await settle();
    // Tab 12 spans 1100-1200: its end plus the margin in sight.
    expect(list.scrollLeft).toBe(1200 - 400 + 24);
    const tabs = wrapper.findAll('[role="tab"]');
    (tabs[11]!.element as HTMLElement).focus();
    await tabs[11]!.trigger('keydown', { key: 'ArrowRight' });
    await settle();
    expect(model.value).toBe('g13');
    expect(list.scrollLeft).toBe(1300 - 400 + 24);
  });

  it('marks exactly the chosen tab', async () => {
    const { wrapper } = await mountStrip('g9');
    const selected = wrapper.findAll('[role="tab"]').filter((tab) => tab.attributes('aria-selected') === 'true');
    expect(selected.map((tab) => tab.text())).toEqual(['Group 9']);
  });
});
