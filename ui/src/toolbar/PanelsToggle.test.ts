// The plugin panels' switch in the toolbar (RB-080-79): `panels.dock.toggle` as a button at the
// right end that shows whether the panels are open, with the shortcut in its tooltip, a chip while
// a panel a command opened waits in the rail, and the last group to leave the row for the overflow
// menu. Since RB-090-10 it is off by default - the icon rail beside the views opens the panels -
// and drawn only with `ui.toolbar.panelsToggle` on.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CommandInfo, CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import { OVERFLOW_ORDER } from './groups';
import { PANELS_COMMAND, PANELS_DOCK_SETTING, PANELS_STATE_SETTING, PANELS_TOGGLE_SETTING } from './panelsToggle';
import PanelsToggle from './PanelsToggle.vue';
import ShellToolbar from './ShellToolbar.vue';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const SWITCH = '[data-testid="panels-toggle"]';

/** The switch is drawn only when the setting is on (RB-090-10). */
const ON = { [PANELS_TOGGLE_SETTING]: true };

async function toolbar(settings: Record<string, unknown> = ON) {
  const mounted = await mountWithShell(ShellToolbar, { mock: { settings } });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

async function alone(settings: Record<string, unknown> = ON) {
  const listeners: ((message: CoreToShell) => void)[] = [];
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.push(listener);
    return () => undefined;
  };
  const mounted = await mountWithShell(PanelsToggle, { mock: { settings }, mount: { props: { subscribe } } });
  current = mounted.wrapper;
  await settle();
  const event = async (message: CoreToShell): Promise<void> => {
    listeners.forEach((listener) => listener(message));
    await settle();
  };
  return { ...mounted, event };
}

describe("the plugin panels' switch in the toolbar", () => {
  it('stands at the right end, runs panels.dock.toggle and is on while the panels are open', async () => {
    const { wrapper, mock } = await toolbar({ ...ON, [PANELS_STATE_SETTING]: 'open' });
    const button = wrapper.get(SWITCH);
    expect(button.attributes('role')).toBe('switch');
    expect(button.attributes('aria-checked')).toBe('true');
    expect(button.attributes('aria-label')).toBe('Plugin panels');
    const group = button.element.closest('[data-toolbar-group]');
    expect(group?.getAttribute('data-toolbar-group')).toBe('panels');
    // The last group in the row, only the overflow menu's button after it.
    const groups = [...wrapper.element.querySelectorAll('[data-toolbar-group]')];
    expect(groups.at(-1)?.getAttribute('data-toolbar-group')).toBe('panels');
    await button.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: PANELS_COMMAND });
  });

  it('is off at the rail, hidden or without a value, and follows a changed setting', async () => {
    const settings: Record<string, unknown> = { ...ON, [PANELS_STATE_SETTING]: 'rail' };
    const { wrapper, mock } = await toolbar(settings);
    expect(wrapper.get(SWITCH).attributes('aria-checked')).toBe('false');
    settings[PANELS_STATE_SETTING] = 'open';
    mock.emit({ type: 'settingsChanged', keys: [PANELS_STATE_SETTING] });
    await settle();
    expect(wrapper.get(SWITCH).attributes('aria-checked')).toBe('true');
    settings[PANELS_STATE_SETTING] = 'hidden';
    mock.emit({ type: 'settingsChanged', keys: [PANELS_STATE_SETTING] });
    await settle();
    expect(wrapper.get(SWITCH).attributes('aria-checked')).toBe('false');
    current?.unmount();
    const bare = await toolbar();
    expect(bare.wrapper.get(SWITCH).attributes('aria-checked')).toBe('false');
  });

  it("shows the icon of the panels' place", async () => {
    for (const [place, icon] of [
      ['left', 'i-lucide-panel-left'],
      ['bottom', 'i-lucide-panel-bottom'],
      ['right', 'i-lucide-panel-right'],
      ['floating', 'i-lucide-app-window'],
      [undefined, 'i-lucide-panel-right'],
    ] as const) {
      const { wrapper } = await alone(place === undefined ? ON : { ...ON, [PANELS_DOCK_SETTING]: place });
      expect(wrapper.get(SWITCH).attributes('data-icon')).toBe(icon);
      wrapper.unmount();
      current = null;
    }
  });

  it('is gone with ui.toolbar.panelsToggle off', async () => {
    const { wrapper } = await toolbar({ [PANELS_TOGGLE_SETTING]: false });
    expect(wrapper.find(SWITCH).exists()).toBe(false);
    expect(wrapper.get('[data-toolbar-group="panels"]').element.children).toHaveLength(0);
  });

  it('is off by default: the icon rail beside the views opens the panels (RB-090-10)', async () => {
    const settings: Record<string, unknown> = {};
    const { wrapper, mock } = await toolbar(settings);
    expect(wrapper.find(SWITCH).exists()).toBe(false);
    expect(wrapper.get('[data-toolbar-group="panels"]').element.children).toHaveLength(0);
    // Switched on in the settings, it is there at once.
    settings[PANELS_TOGGLE_SETTING] = true;
    mock.emit({ type: 'settingsChanged', keys: [PANELS_TOGGLE_SETTING] });
    await settle();
    expect(wrapper.find(SWITCH).exists()).toBe(true);
  });

  it('carries a chip while a panel a command opened waits in the rail, until the panels open', async () => {
    const settings: Record<string, unknown> = { ...ON, [PANELS_STATE_SETTING]: 'rail' };
    const { wrapper, event, mock } = await alone(settings);
    expect(wrapper.get(SWITCH).attributes('data-attention')).toBeUndefined();
    await event({ type: 'pluginPanel', message: { select: { core: 'findings' } } });
    expect(wrapper.get(SWITCH).attributes('data-attention')).toBe('true');
    expect(wrapper.get(SWITCH).attributes('aria-label')).toBe('Plugin panels, a panel waits in the rail');
    settings[PANELS_STATE_SETTING] = 'open';
    mock.emit({ type: 'settingsChanged', keys: [PANELS_STATE_SETTING] });
    await settle();
    expect(wrapper.get(SWITCH).attributes('data-attention')).toBeUndefined();
    // Open, a select changes nothing.
    await event({ type: 'pluginPanel', message: { select: { plugin: 'fonts', panel: 'fonts' } } });
    expect(wrapper.get(SWITCH).attributes('data-attention')).toBeUndefined();
  });

  it("names the command's shortcut in its tooltip", async () => {
    const { wrapper, mock } = await alone();
    expect(wrapper.get(SWITCH).attributes('title')).toBe('Plugin panels');
    const command: CommandInfo = {
      id: PANELS_COMMAND,
      titleKey: `commands.${PANELS_COMMAND}`,
      group: 'panels',
      bindings: ['Ctrl+Shift+Space P Z'],
      defaultBindings: ['Ctrl+Shift+Space P Z'],
      changed: false,
      mcp: { covered: 'panels_dock' },
    };
    mock.emit({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: [command] });
    await settle();
    expect(wrapper.get(SWITCH).attributes('title')).toBe('Plugin panels (Ctrl+Shift+Space P Z)');
  });

  it('leaves the row last in a narrow window', () => {
    expect(OVERFLOW_ORDER.at(-1)).toBe('panels');
  });
});
