// The choice of rBrowser's theme in the toolbar (RB-080-95, RB-090-10): one sun-and-moon icon in
// every state with a mark for the theme in effect, a native menu System / Light / Dark that runs
// `ui.theme.set`, the state and the Windows theme in its name and tooltip with the stepping
// shortcut, told apart from the page emulation's sun and moon, gone with `ui.toolbar.themeToggle`
// off, and a headed block of the overflow menu in a narrow window.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CommandInfo } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import { THEME_SETTING_KEY } from '../theme/theme';
import { OVERFLOW_ORDER } from './groups';
import { overflowEntries } from './overflowEntries';
import ShellToolbar from './ShellToolbar.vue';
import { THEME_CYCLE_COMMAND, THEME_ICON, THEME_MARKS, THEME_SET_COMMAND, THEME_TOGGLE_SETTING } from './themeToggle';

let current: VueWrapper | null = null;

async function setup(settings: Record<string, unknown> = {}) {
  const mounted = await mountWithShell(ShellToolbar, { mock: { settings } });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

const SELECT = '[data-testid="theme-toggle"]';
const ICON = '[data-testid="theme-toggle-icon"]';
const MARK = '[data-testid="theme-toggle-mark"]';

/** The menu's choices as their texts, the placeholder that names the menu left out. */
function choices(wrapper: VueWrapper): string[] {
  return wrapper
    .get(SELECT)
    .findAll('option')
    .filter((option) => option.attributes('value') !== '')
    .map((option) => option.text());
}

describe("the choice of rBrowser's theme in the toolbar", () => {
  it('is one sun-and-moon icon in every state, with a mark for the theme in effect', async () => {
    const settings: Record<string, unknown> = {};
    const { wrapper, mock } = await setup(settings);
    for (const theme of ['system', 'light', 'dark'] as const) {
      settings[THEME_SETTING_KEY] = theme;
      mock.emit({ type: 'settingsChanged', keys: [THEME_SETTING_KEY] });
      await settle();
      expect(wrapper.get(ICON).attributes('data-icon')).toBe(THEME_ICON);
      expect(wrapper.get(MARK).attributes('data-icon')).toBe(THEME_MARKS[theme]);
      expect(wrapper.get(SELECT).attributes('data-theme-setting')).toBe(theme);
    }
  });

  it("never looks like the page emulation's sun and moon", () => {
    expect(THEME_ICON).toBe('i-lucide-sun-moon');
    const marks = Object.values(THEME_MARKS);
    expect(new Set(marks).size).toBe(3);
    for (const icon of [THEME_ICON, ...marks]) {
      expect(icon).not.toBe('i-lucide-sun');
      expect(icon).not.toBe('i-lucide-moon');
    }
  });

  it('names the theme in effect, and for System what Windows uses now', async () => {
    const settings: Record<string, unknown> = {};
    const { wrapper, mock } = await setup(settings);
    // The test's Windows is light.
    expect(wrapper.get(SELECT).attributes('aria-label')).toBe('rBrowser theme: System (Windows: light)');
    for (const [theme, name] of [
      ['light', 'rBrowser theme: Light'],
      ['dark', 'rBrowser theme: Dark'],
      ['system', 'rBrowser theme: System (Windows: light)'],
    ] as const) {
      settings[THEME_SETTING_KEY] = theme;
      mock.emit({ type: 'settingsChanged', keys: [THEME_SETTING_KEY] });
      await settle();
      expect(wrapper.get(SELECT).attributes('aria-label')).toBe(name);
      expect(wrapper.get(SELECT).attributes('title')).toContain(name);
    }
  });

  it('offers System, Light and Dark, the current one marked, and sets the one chosen', async () => {
    const { wrapper, mock } = await setup({ [THEME_SETTING_KEY]: 'dark' });
    const select = wrapper.get(SELECT);
    expect(select.element.tagName).toBe('SELECT');
    expect(choices(wrapper)).toEqual(['System (Windows: light)', 'Light', 'Dark (current)']);
    await select.setValue('light');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'light' } });
    // The select names the menu again, so the same choice can be made twice.
    expect((select.element as HTMLSelectElement).value).toBe('');
    await select.setValue('system');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'system' } });
  });

  it('sets nothing while the keyboard browses the closed menu, only what is chosen', async () => {
    const { wrapper, mock } = await setup({ [THEME_SETTING_KEY]: 'dark' });
    const element = wrapper.get(SELECT).element as HTMLSelectElement;
    element.focus();
    const sets = () => mock.sent.filter((m) => m.type === 'runCommand' && m.id === THEME_SET_COMMAND);
    // What a closed select does on Windows for an arrow key or a typed letter: the next option,
    // and `change` - unless the key was held back. No popup here (no `showPicker`).
    for (const [key, value] of [
      ['ArrowDown', 'system'],
      ['ArrowDown', 'system'],
      ['ArrowUp', 'system'],
      ['l', 'light'],
      ['d', 'dark'],
    ] as const) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      expect(down.defaultPrevented, key).toBe(true);
      // Even a `change` such a key causes all the same sets nothing.
      element.value = value;
      element.dispatchEvent(new Event('change', { bubbles: true }));
      await settle();
      element.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
      expect(element.value).toBe('');
    }
    expect(sets()).toHaveLength(0);
    // A choice from the list sets the theme.
    await wrapper.get(SELECT).trigger('pointerdown');
    await wrapper.get(SELECT).setValue('light');
    await settle();
    expect(sets()).toEqual([{ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'light' } }]);
  });

  it('opens its list on an arrow, Enter or Space, and not on Tab or Escape', async () => {
    const { wrapper, mock } = await setup();
    const element = wrapper.get(SELECT).element as HTMLSelectElement;
    const showPicker = vi.fn();
    element.showPicker = showPicker;
    const before = mock.sent.length;
    for (const key of ['ArrowDown', 'Enter', ' ', 'd']) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      expect(down.defaultPrevented, key).toBe(true);
    }
    expect(showPicker).toHaveBeenCalledTimes(4);
    for (const key of ['Shift', 'Tab', 'Escape']) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
    expect(showPicker).toHaveBeenCalledTimes(4);
    expect(mock.sent).toHaveLength(before);
  });

  it('stands apart from the page emulation and says the pages keep their scheme', async () => {
    const { wrapper } = await setup();
    const select = wrapper.get(SELECT);
    expect(select.element.closest('[data-toolbar-group="theme"]')).not.toBeNull();
    expect(select.element.closest('[data-toolbar-group="emulation"]')).toBeNull();
    expect(select.attributes('title')).toContain('the pages in the views keep their own colour scheme');
  });

  it('is gone with ui.toolbar.themeToggle off', async () => {
    const { wrapper } = await setup({ [THEME_TOGGLE_SETTING]: false });
    expect(wrapper.find(SELECT).exists()).toBe(false);
    expect(wrapper.get('[data-toolbar-group="theme"]').element.children).toHaveLength(0);
  });

  it("names the stepping command's shortcut in its tooltip and follows an edit", async () => {
    const { wrapper, mock } = await setup({ [THEME_SETTING_KEY]: 'light' });
    const command = (bindings: string[]): CommandInfo => ({
      id: THEME_CYCLE_COMMAND,
      titleKey: `commands.${THEME_CYCLE_COMMAND}`,
      group: 'view',
      bindings,
      defaultBindings: ['Ctrl+Shift+Space V 0'],
      changed: false,
      mcp: { omitted: 'app display setting' },
    });
    const select = wrapper.get(SELECT);
    expect(select.attributes('title')).not.toContain('Ctrl+Shift+Space');
    mock.emit({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: [command(['Ctrl+Shift+Space V 0'])] });
    await settle();
    expect(select.attributes('title')).toContain('rBrowser theme: Light');
    expect(select.attributes('title')).toContain('Ctrl+Shift+Space V 0');
    mock.emit({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: [command(['Ctrl+Shift+Space V 9'])] });
    await settle();
    expect(select.attributes('title')).toContain('Ctrl+Shift+Space V 9');
  });

  it('leaves the row for the overflow menu as a headed block that sets the theme', async () => {
    expect(OVERFLOW_ORDER).toContain('theme');
    const { wrapper, mock } = await setup({ [THEME_SETTING_KEY]: 'dark' });
    const group = wrapper.get('[data-toolbar-group="theme"]').element as HTMLElement;
    const texts = { on: (label: string) => `${label}: on`, off: (label: string) => `${label}: off` };
    const entries = overflowEntries([['theme', group]], texts);
    expect(entries.map((entry) => entry.heading)).toEqual([
      'rBrowser theme: Dark',
      'rBrowser theme: Dark',
      'rBrowser theme: Dark',
    ]);
    expect(entries.map((entry) => entry.label)).toEqual(['System (Windows: light)', 'Light', 'Dark (current)']);
    entries[0]?.run();
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'system' } });
  });
});
