import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { createShellI18n } from '../i18n';
import type { CoreToShell } from '../ipc/protocol';
import { createShell, SHELL_KEY } from '../state/shell';
import CommandPalette from './CommandPalette.vue';
import { PALETTE_COMMAND } from './constants';
import { PALETTE_CLIENT_KEY } from './palette';
import { RECENT_STORAGE_KEY } from './recent';
import { createPaletteCore, type PaletteCore } from './testCore';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
  window.localStorage.removeItem(RECENT_STORAGE_KEY);
});

async function settle(): Promise<void> {
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

async function setup(core: PaletteCore = createPaletteCore()) {
  const shell = createShell(core.client, {
    root: document.createElement('html'),
    matchMedia: () => ({ matches: false, addEventListener: () => undefined, removeEventListener: () => undefined }),
  });
  await shell.start();
  let push: (message: CoreToShell) => void = () => undefined;
  const wrapper = mount(CommandPalette, {
    props: {
      subscribe: (listener: (message: CoreToShell) => void) => {
        push = listener;
        return () => undefined;
      },
    },
    attachTo: document.body,
    global: {
      plugins: [ui, createShellI18n()],
      provide: { [SHELL_KEY]: shell, [PALETTE_CLIENT_KEY]: core.client },
    },
  });
  current = wrapper;
  const open = async (): Promise<void> => {
    push({ type: 'uiCommand', id: PALETTE_COMMAND });
    await settle();
  };
  const input = () => wrapper.get('[role="combobox"]');
  const type = async (text: string): Promise<void> => {
    await input().setValue(text);
    await nextTick();
  };
  const key = async (name: string, init: KeyboardEventInit = {}): Promise<void> => {
    await input().trigger('keydown', { key: name, ...init });
    await settle();
  };
  const emit = (message: CoreToShell): void => push(message);
  return { wrapper, core, open, input, type, key, shell, emit };
}

describe('the command palette', () => {
  it('opens in its popup at once as an ARIA combobox with the focus in its field', async () => {
    const { wrapper, input } = await setup();
    await settle();
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(true);
    const field = input();
    expect(document.activeElement).toBe(field.element);
    expect(field.attributes('aria-expanded')).toBe('true');
    const listbox = wrapper.get(`#${field.attributes('aria-controls')}`);
    expect(listbox.attributes('role')).toBe('listbox');
    const options = listbox.findAll('[role="option"]');
    expect(options.length).toBeGreaterThan(0);
    expect(field.attributes('aria-activedescendant')).toBe(options[0]?.attributes('id'));
    expect(options[0]?.attributes('aria-selected')).toBe('true');
    expect(wrapper.get('[role="status"]').text()).toMatch(/\d+ matches/);
  });

  it('is visible at once, before the core has answered', async () => {
    const { wrapper, emit } = await setup();
    emit({ type: 'uiCommand', id: PALETTE_COMMAND });
    await nextTick();
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(true);
  });

  it('runs the selected command with Enter, hides its popup and gives the focus back to the view', async () => {
    const { wrapper, core, open, type, key } = await setup();
    await open();
    await type('reload');
    await key('Enter');
    const after = core.sent.map((m) => m.type).filter((t) => ['dismissPalette', 'setActiveView', 'runCommand'].includes(t));
    expect(after).toEqual(['dismissPalette', 'setActiveView', 'runCommand']);
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'reload.all' });
    expect(core.sent).toContainEqual({ type: 'setActiveView', view: 1 });
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(false);
  });

  it('starts afresh on the next double leader', async () => {
    const { input, open, type } = await setup();
    await open();
    await type('reload');
    await open();
    expect((input().element as HTMLInputElement).value).toBe('');
    expect(document.activeElement).toBe(input().element);
  });

  it('leaves the popup to hide itself when it loses the focus', async () => {
    const { wrapper, core, open, input } = await setup();
    await open();
    await input().trigger('blur');
    await settle();
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(false);
    expect(core.sent.some((m) => m.type === 'dismissPalette')).toBe(false);
  });

  it('moves the selection with the arrow keys', async () => {
    const { wrapper, open, input, key } = await setup();
    await open();
    const first = input().attributes('aria-activedescendant');
    await key('ArrowDown');
    const second = input().attributes('aria-activedescendant');
    expect(second).not.toBe(first);
    expect(wrapper.get(`#${second}`).attributes('aria-selected')).toBe('true');
    await key('ArrowUp');
    expect(input().attributes('aria-activedescendant')).toBe(first);
  });

  it('sets the active view to "tablet" through "Set view to tablet"', async () => {
    const { core, open, type, key, wrapper } = await setup();
    await open();
    await type('set view to tablet');
    expect(wrapper.get('[role="option"][aria-selected="true"]').text()).toContain('Set view to Tablet');
    await key('Enter');
    expect(core.sent).toContainEqual({ type: 'setBreakpoint', view: 1, breakpoint: 'tablet' });
  });

  it('asks for the argument in a second step and goes back with Escape', async () => {
    const { wrapper, core, open, type, key } = await setup();
    await open();
    await type('set view to ...');
    await key('Enter');
    expect(wrapper.get('[data-testid="palette-step"]').text()).toContain('Set view to');
    // The step is a Nuxt UI badge (RB-080-88).
    expect(wrapper.get('[data-testid="palette-step"]').attributes('data-slot')).toBe('base');
    expect(wrapper.findAll('[role="option"]').map((o) => o.text()).join(' ')).toContain('Tablet');
    await key('Escape');
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(true);
    await key('Escape');
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(false);
    expect(core.sent).toContainEqual({ type: 'dismissPalette' });
  });

  it('shows each entry\'s shortcut and opens the shortcut editor with Ctrl+Enter', async () => {
    const { wrapper, core, open, type, key } = await setup();
    await open();
    await type('sync');
    const selected = wrapper.get('[role="option"][aria-selected="true"]');
    expect(selected.text()).toContain('Ctrl+Shift+Space');
    await key('Enter', { ctrlKey: true });
    expect(core.sent).toContainEqual({ type: 'openSettings', key: 'shortcuts.bindings' });
  });

  it('opens a setting at its field', async () => {
    const { core, open, type, key } = await setup();
    await open();
    await type('#scroll');
    await key('Enter');
    expect(core.sent).toContainEqual({ type: 'openSettings', key: 'sync.scroll' });
  });

  it('shows the description of the selected entry unless the setting says no', async () => {
    const { wrapper, open, type } = await setup();
    await open();
    await type('#scroll');
    expect(wrapper.get('[data-testid="palette-description"]').text()).toContain('Scrolling');
    current?.unmount();
    const core = createPaletteCore();
    core.values.push({ key: 'palette.showDescriptions', value: false, origin: 'user' });
    const second = await setup(core);
    await second.open();
    await second.type('#scroll');
    expect(second.wrapper.find('[data-testid="palette-description"]').exists()).toBe(false);
  });

  it('says when nothing matches', async () => {
    const { wrapper, open, type } = await setup();
    await open();
    await type('zzzzqqq');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(0);
    expect(wrapper.get('[role="status"]').text()).toBe('Nothing matches');
  });
  // RB-080-46: the new-size picker is the palette in its step "New view".
  it('starts in the new-view step on view.add_size and afresh on the palette command', async () => {
    const { wrapper, emit, open, key } = await setup();
    await settle();
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(false);
    emit({ type: 'uiCommand', id: 'view.add_size' });
    await settle();
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(true);
    await open();
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(false);
    emit({ type: 'uiCommand', id: 'view.add_size' });
    await settle();
    await key('Escape');
    expect(wrapper.find('[data-testid="palette"]').exists()).toBe(false);
  });
});
