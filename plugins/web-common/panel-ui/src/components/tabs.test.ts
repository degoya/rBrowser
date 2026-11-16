import ui from '@nuxt/ui/vue-plugin';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { defineComponent, h, nextTick, ref } from 'vue';
import { PANEL_KEY } from '../mount';
import type { Panel } from '../panel';
import PanelTabs from './PanelTabs.vue';

const ITEMS = Array.from({ length: 15 }, (_, i) => ({ label: `Group ${i + 1}`, value: `g${i + 1}` }));

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function settle(): Promise<void> {
  await flushPromises();
  await nextTick();
}

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

interface Options {
  initial?: string;
  variant?: 'pill' | 'link';
  strings?: Record<string, string>;
  attrs?: Record<string, unknown>;
  slots?: Record<string, () => unknown>;
  items?: { label: string; value: string; slot?: string }[];
}

async function mountTabs(options: Options = {}) {
  const model = ref(options.initial ?? 'g1');
  const Host = defineComponent({
    setup: () => () =>
      h(
        PanelTabs,
        {
          modelValue: model.value,
          'onUpdate:modelValue': (value: string | number | undefined) => (model.value = String(value)),
          items: options.items ?? ITEMS,
          variant: options.variant,
          content: false,
          ...options.attrs,
        },
        options.slots,
      ),
  });
  const provide = options.strings ? { [PANEL_KEY as symbol]: { strings: ref(options.strings) } as unknown as Panel } : {};
  const wrapper = mount(Host, { attachTo: document.body, global: { plugins: [ui], provide } });
  await settle();
  current = wrapper;
  return { wrapper, model };
}

/** The scroll handler measures the row; the scroll event is how a layout would tell it. */
async function scrolled(list: HTMLElement): Promise<void> {
  list.dispatchEvent(new Event('scroll'));
  await settle();
}

describe('PanelTabs, the one tab row of a plugin panel (RB-090-48)', () => {
  for (const variant of ['pill', 'link'] as const) {
    it(`keeps every tab in one row that never scrolls up and down (${variant})`, async () => {
      const { wrapper } = await mountTabs({ variant });
      const list = wrapper.get('[role="tablist"]');
      const classes = list.classes();
      expect(classes).toContain('flex-nowrap');
      expect(classes).toContain('overflow-x-auto');
      // The owner's screens: `overflow-x-auto` alone made `overflow-y` auto too, and Windows drew
      // a vertical scroll bar with arrows beside the tabs.
      expect(classes).toContain('overflow-y-hidden');
      for (const banned of ['overflow-auto', 'overflow-scroll', 'overflow-y-auto', 'overflow-y-scroll', 'overflow-x-scroll', 'flex-wrap']) {
        expect(classes).not.toContain(banned);
      }
      // No scroll bar shows in either direction; the buttons say there is more.
      expect(classes).toContain('[scrollbar-width:none]');
      for (const tab of wrapper.findAll('[role="tab"]')) {
        expect(tab.classes()).toContain('shrink-0');
      }
    });
  }

  it('draws the link line and the marker inside the row, so nothing sticks out below it', async () => {
    const { wrapper, model } = await mountTabs({ variant: 'link' });
    // The marker is placed once a tab is chosen.
    model.value = 'g2';
    await settle();
    const list = wrapper.get('[role="tablist"]').classes();
    expect(list).not.toContain('-mb-px');
    expect(list).not.toContain('border-b');
    expect(list).toContain('shadow-[inset_0_-1px_0_var(--ui-border)]');
    const marker = wrapper.get('[data-slot="indicator"]').classes();
    expect(marker).not.toContain('-bottom-px');
    expect(marker).toContain('bottom-0');
  });

  it('shows no scroll buttons when every tab fits (wide panel)', async () => {
    const { wrapper } = await mountTabs();
    await scrolled(layOut(wrapper, 1600, 1500));
    expect(wrapper.find('[data-testid="tabs-back"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="tabs-forward"]').exists()).toBe(false);
  });

  it('offers the side with more tabs at 400 and 800 px and pages there', async () => {
    for (const width of [400, 800]) {
      const { wrapper } = await mountTabs();
      const list = layOut(wrapper, width, 1500);
      list.scrollLeft = 0;
      await scrolled(list);
      const back = wrapper.get('[data-testid="tabs-back"]');
      const forward = wrapper.get('[data-testid="tabs-forward"]');
      // The back button keeps its place, hidden, so the row does not jump while it scrolls.
      expect(back.classes()).toContain('invisible');
      expect(forward.classes()).not.toContain('invisible');
      // The buttons stand over the row; screen readers and the keyboard move by the tabs.
      expect(forward.attributes('aria-hidden')).toBe('true');
      expect(forward.attributes('tabindex')).toBe('-1');
      expect(forward.attributes('title')).toBe('Show more tabs');
      expect(back.attributes('title')).toBe('Show earlier tabs');
      await forward.trigger('click');
      await settle();
      expect(list.scrollLeft).toBe(Math.round(width * 0.8));
      expect(wrapper.get('[data-testid="tabs-back"]').classes()).not.toContain('invisible');
      current?.unmount();
      current = null;
    }
  });

  it('names the buttons with the texts the host sends', async () => {
    const strings = { 'plugins.panel.tabs_back': 'Earlier', 'plugins.panel.tabs_forward': 'More' };
    const { wrapper } = await mountTabs({ strings });
    await scrolled(layOut(wrapper, 400, 1500));
    expect(wrapper.get('[data-testid="tabs-back"]').attributes('title')).toBe('Earlier');
    expect(wrapper.get('[data-testid="tabs-forward"]').attributes('title')).toBe('More');
  });

  it('scrolls the chosen tab into sight, also when the keyboard moves to it', async () => {
    const { wrapper, model } = await mountTabs();
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

  it('passes the attributes and the content slots through to the tabs', async () => {
    const items = [
      { label: 'One', value: 'g1', slot: 'one' },
      { label: 'Two', value: 'g2', slot: 'two' },
    ];
    const { wrapper } = await mountTabs({
      items,
      attrs: { id: 'tabs', 'aria-label': 'Sections', content: true, unmountOnHide: false, 'data-testid': 'sections' },
      slots: { one: () => h('p', { id: 'first' }, 'first'), two: () => h('p', { id: 'second' }, 'second') },
    });
    const root = wrapper.get('#tabs');
    expect(root.attributes('data-testid')).toBe('sections');
    expect(wrapper.find('#first').text()).toBe('first');
    // `unmount-on-hide` off: the hidden tab's content stays in the page.
    expect(wrapper.find('#second').exists()).toBe(true);
    expect(wrapper.findAll('[role="tab"]').map((tab) => tab.text())).toEqual(['One', 'Two']);
  });

  it('marks exactly the chosen tab', async () => {
    const { wrapper } = await mountTabs({ initial: 'g9' });
    const selected = wrapper.findAll('[role="tab"]').filter((tab) => tab.attributes('aria-selected') === 'true');
    expect(selected.map((tab) => tab.text())).toEqual(['Group 9']);
  });
});
