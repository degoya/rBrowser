// The choice of rBrowser's theme in the toolbar (RB-080-95, RB-090-10): one sun-and-moon icon in
// every state with a mark for the theme in effect, the core's native menu System / Light / Dark
// below it with the one in effect checked that runs `ui.theme.set` (RB-090-41), the state and the
// Windows theme in its name and tooltip with the stepping shortcut, told apart from the page
// emulation's sun and moon, gone with `ui.toolbar.themeToggle` off, and a headed block of the
// overflow menu in a narrow window.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CommandInfo, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
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

const BUTTON = '[data-testid="theme-toggle"]';
const ICON = '[data-testid="theme-toggle-icon"]';
const MARK = '[data-testid="theme-toggle-mark"]';

/** The rows of the native menu asked for last: text and check. */
function rows(sent: Parameters<typeof lastNativeMenu>[0]): [string, boolean | undefined][] {
  return lastNativeMenu(sent).rows.map((row) => (row.kind === 'item' ? [row.label, row.checked] : [row.kind, undefined]));
}

const sets = (sent: readonly ShellToCore[]) =>
  sent.filter((message) => message.type === 'runCommand' && message.id === THEME_SET_COMMAND);

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
      expect(wrapper.get(BUTTON).attributes('data-theme-setting')).toBe(theme);
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
    expect(wrapper.get(BUTTON).attributes('aria-label')).toBe('rBrowser theme: System (Windows: light)');
    for (const [theme, name] of [
      ['light', 'rBrowser theme: Light'],
      ['dark', 'rBrowser theme: Dark'],
      ['system', 'rBrowser theme: System (Windows: light)'],
    ] as const) {
      settings[THEME_SETTING_KEY] = theme;
      mock.emit({ type: 'settingsChanged', keys: [THEME_SETTING_KEY] });
      await settle();
      expect(wrapper.get(BUTTON).attributes('aria-label')).toBe(name);
      expect(wrapper.get(BUTTON).attributes('title')).toContain(name);
    }
  });

  it('offers System, Light and Dark below the button, the current one checked, and sets the one chosen', async () => {
    const { wrapper, mock } = await setup({ [THEME_SETTING_KEY]: 'dark' });
    const button = wrapper.get(BUTTON);
    expect(button.element.tagName).toBe('BUTTON');
    expect(button.attributes('aria-haspopup')).toBe('menu');
    const anchor = button.element.parentElement as HTMLElement;
    anchor.getBoundingClientRect = () =>
      ({ left: 600.4, top: 6, width: 32, height: 32, right: 632, bottom: 38, x: 600, y: 6 }) as DOMRect;
    await button.trigger('click');
    await settle();
    expect(lastNativeMenu(mock.sent).anchor).toEqual({ x: 600, y: 6, width: 32, height: 32 });
    expect(rows(mock.sent)).toEqual([
      ['System (Windows: light)', false],
      ['Light', false],
      ['Dark', true],
    ]);
    // No list of the page's own: the menu is the core's, in front of the views.
    expect(wrapper.get('[data-toolbar-group="theme"]').find('select').exists()).toBe(false);
    expect(document.body.querySelector('[role="menu"]')).toBeNull();
    await closeNativeMenu(mock.sent, 'light');
    expect(sets(mock.sent)).toEqual([{ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'light' } }]);
    expect(document.activeElement).toBe(button.element);
    // The same choice again sets it again.
    await button.trigger('click');
    await closeNativeMenu(mock.sent, 'light');
    expect(sets(mock.sent)).toHaveLength(2);
    await button.trigger('click');
    await closeNativeMenu(mock.sent, 'system');
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'system' } });
  });

  it('follows the theme and the Windows theme in its rows', async () => {
    const settings: Record<string, unknown> = { [THEME_SETTING_KEY]: 'light' };
    const { wrapper, mock } = await setup(settings);
    await wrapper.get(BUTTON).trigger('click');
    await settle();
    expect(rows(mock.sent)).toEqual([
      ['System (Windows: light)', false],
      ['Light', true],
      ['Dark', false],
    ]);
    await closeNativeMenu(mock.sent);
    settings[THEME_SETTING_KEY] = 'system';
    mock.emit({ type: 'settingsChanged', keys: [THEME_SETTING_KEY] });
    await settle();
    await wrapper.get(BUTTON).trigger('click');
    await settle();
    expect(rows(mock.sent)[0]).toEqual(['System (Windows: light)', true]);
  });

  it('opens on an arrow key or F4, not on Tab, Escape or a letter; Esc closes it with the focus back', async () => {
    const { wrapper, mock } = await setup({ [THEME_SETTING_KEY]: 'dark' });
    const element = wrapper.get(BUTTON).element as HTMLButtonElement;
    element.focus();
    const menus = () => mock.sent.filter((message) => message.type === 'nativeMenu').length;
    for (const key of ['Shift', 'Tab', 'Escape', 'd']) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
    await settle();
    expect(menus()).toBe(0);
    for (const [index, key] of ['ArrowDown', 'ArrowUp', 'F4'].entries()) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      await settle();
      expect(down.defaultPrevented, key).toBe(true);
      expect(menus(), key).toBe(index + 1);
      // Esc in the native menu: it closes without a choice.
      element.blur();
      await closeNativeMenu(mock.sent);
      expect(document.activeElement, key).toBe(element);
    }
    expect(sets(mock.sent)).toHaveLength(0);
  });

  it('stands apart from the page emulation and says the pages keep their scheme', async () => {
    const { wrapper } = await setup();
    const select = wrapper.get(BUTTON);
    expect(select.element.closest('[data-toolbar-group="theme"]')).not.toBeNull();
    expect(select.element.closest('[data-toolbar-group="emulation"]')).toBeNull();
    expect(select.attributes('title')).toContain('the pages in the views keep their own colour scheme');
  });

  it('is gone with ui.toolbar.themeToggle off', async () => {
    const { wrapper } = await setup({ [THEME_TOGGLE_SETTING]: false });
    expect(wrapper.find(BUTTON).exists()).toBe(false);
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
    const select = wrapper.get(BUTTON);
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
    const texts = {
      on: (label: string) => `${label}: on`,
      off: (label: string) => `${label}: off`,
    };
    const entries = overflowEntries([['theme', group]], texts);
    expect(entries.map((entry) => entry.heading)).toEqual([
      'rBrowser theme: Dark',
      'rBrowser theme: Dark',
      'rBrowser theme: Dark',
    ]);
    // The theme in effect checked, as in the button's own menu (RB-090-45).
    expect(entries.map((entry) => [entry.label, entry.checked])).toEqual([
      ['System (Windows: light)', false],
      ['Light', false],
      ['Dark', true],
    ]);
    entries[0]?.run();
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme: 'system' } });
  });
});
