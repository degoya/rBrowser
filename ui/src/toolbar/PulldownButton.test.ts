// A toolbar pulldown as a button (RB-090-45): the value in effect with a chevron, the rows as the
// core's native menu below it with the one in effect checked, every choice told - the one in
// effect too -, the keyboard of a select (arrows and F4 open, Esc closes, the focus comes back),
// disabled without choices, and the group's icon alone in a narrow window.
import { afterEach, describe, expect, it } from 'vitest';
import { computed, defineComponent, h, provide } from 'vue';
import type { VueWrapper } from '@vue/test-utils';
import UButton from '@nuxt/ui/components/Button.vue';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
import PulldownButton from './PulldownButton.vue';
import { GROUP_LOOK_KEY, PULLDOWN_CHEVRON, choiceRows, shownChoice } from './pulldown';

const CHOICES = [
  { value: 'local', label: 'local' },
  { value: 'staging', label: 'staging' },
];

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function setup(props: Record<string, unknown> = {}) {
  const mounted = await mountWithShell(PulldownButton, {
    mount: {
      props: {
        kind: 'environment',
        label: 'Environment',
        value: 'local',
        rows: () => choiceRows(CHOICES, 'local'),
        ...props,
      },
    },
  });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

const menus = (sent: readonly { type: string }[]) => sent.filter((message) => message.type === 'nativeMenu').length;

describe('a toolbar pulldown', () => {
  it('is a button with the value in effect and a chevron, named with both', async () => {
    const { wrapper } = await setup();
    const button = wrapper.get('button');
    expect(button.text()).toBe('local');
    expect(button.attributes('aria-haspopup')).toBe('menu');
    expect(button.attributes('aria-label')).toBe('Environment: local');
    expect(button.attributes('title')).toBe('Environment: local');
    expect(button.attributes('data-pulldown')).toBeDefined();
    expect(wrapper.getComponent(UButton).props('trailingIcon')).toBe(PULLDOWN_CHEVRON);
    expect(button.find('[data-slot="trailingIcon"]').exists()).toBe(true);
    expect(wrapper.find('select').exists()).toBe(false);
  });

  it('opens its rows below itself, the one in effect checked, and tells every choice', async () => {
    const { wrapper, mock } = await setup();
    const anchor = wrapper.get('[data-pulldown]').element.parentElement as HTMLElement;
    anchor.getBoundingClientRect = () =>
      ({ left: 700.2, top: 6, width: 96, height: 32, right: 796, bottom: 38, x: 700, y: 6 }) as DOMRect;
    await wrapper.get('button').trigger('click');
    await settle();
    const request = lastNativeMenu(mock.sent);
    expect(request.menu).toMatch(/^environment-\d+$/);
    expect(request.anchor).toEqual({ x: 700, y: 6, width: 96, height: 32 });
    expect(request.rows).toEqual([
      { kind: 'item', id: 'local', label: 'local', checked: true },
      { kind: 'item', id: 'staging', label: 'staging', checked: false },
    ]);
    await closeNativeMenu(mock.sent, 'staging');
    // The one in effect again is a choice too (RB-080-16).
    await wrapper.get('button').trigger('click');
    await closeNativeMenu(mock.sent, 'local');
    expect(wrapper.emitted('choose')).toEqual([['staging'], ['local']]);
    expect(document.activeElement).toBe(wrapper.get('button').element);
  });

  it('opens on an arrow key or F4, not on Tab, Escape or a letter; Esc chooses nothing', async () => {
    const { wrapper, mock } = await setup();
    const element = wrapper.get('button').element as HTMLButtonElement;
    for (const key of ['Tab', 'Escape', 's']) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
    await settle();
    expect(menus(mock.sent)).toBe(0);
    for (const [index, key] of ['ArrowDown', 'ArrowUp', 'F4'].entries()) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      await settle();
      expect(down.defaultPrevented, key).toBe(true);
      expect(menus(mock.sent), key).toBe(index + 1);
      element.blur();
      await closeNativeMenu(mock.sent);
      expect(document.activeElement, key).toBe(element);
    }
    expect(wrapper.emitted('choose')).toBeUndefined();
  });

  it('opens nothing while it is disabled', async () => {
    const { wrapper, mock } = await setup({ value: 'No environments', disabled: true, rows: () => [] });
    const button = wrapper.get('button');
    expect(button.attributes('disabled')).toBeDefined();
    expect(button.text()).toBe('No environments');
    button.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    await settle();
    expect(menus(mock.sent)).toBe(0);
  });

  it('shows its label without a value', async () => {
    const { wrapper } = await setup({ value: undefined, label: 'Bookmarks' });
    expect(wrapper.get('button').text()).toBe('Bookmarks');
    expect(wrapper.get('button').attributes('aria-label')).toBe('Bookmarks');
  });

  it("is the group's icon alone in a narrow window, its name the tooltip", async () => {
    const Compact = defineComponent({
      setup() {
        provide(GROUP_LOOK_KEY, { compact: computed(() => true), icon: computed(() => 'i-lucide-server') });
        return () => h(PulldownButton, { kind: 'environment', label: 'Environment', value: 'local', rows: () => [] });
      },
    });
    const { wrapper } = await mountWithShell(Compact);
    current = wrapper;
    await settle();
    const button = wrapper.get('button');
    expect(button.text()).toBe('');
    expect(button.classes()).toContain('size-8');
    const props = wrapper.getComponent(UButton).props();
    expect(props.icon).toBe('i-lucide-server');
    expect(props.trailingIcon).toBeUndefined();
    expect(button.attributes('title')).toBe('Environment: local');
  });
});

describe('the rows of a choice', () => {
  it('names the choice in effect, or why there is none', () => {
    expect(shownChoice(CHOICES, 'staging', 'None', 'Empty')).toBe('staging');
    expect(shownChoice(CHOICES, null, 'None', 'Empty')).toBe('None');
    expect(shownChoice(CHOICES, 'gone', 'None', 'Empty')).toBe('None');
    expect(shownChoice([], null, 'None', 'Empty')).toBe('Empty');
  });
});
