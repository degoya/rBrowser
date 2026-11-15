import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CommandInfo, CoreToShell, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import { DEFAULT_STATE, type DockState } from './dock';
import PanelDockBar from './PanelDockBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  document.body.replaceChildren();
});

const command = (id: string, binding: string): CommandInfo => ({
  id,
  titleKey: `commands.${id}`,
  group: 'panels',
  bindings: [binding],
  defaultBindings: [binding],
  changed: false,
  mcp: { covered: 'panels_dock' },
});

async function mounted(state: Partial<DockState>, selected: string | null = 'network-panel.main') {
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message): Promise<CoreToShell> => {
      asked.push(message);
      return { type: 'ack' };
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(PanelDockBar, {
    mount: { props: { client, current: selected, state: { ...DEFAULT_STATE, collapsed: false, ...state } } },
  });
  current = result.wrapper;
  await settle();
  const commands = () => asked.filter((message) => message.type === 'runCommand');
  /** Opens the Dock menu from the keyboard (happy-dom has no pointer for Reka's trigger). */
  const open = async (): Promise<HTMLElement[]> => {
    const button = result.wrapper.get('[data-testid="panel-dock-menu"]');
    await button.trigger('keydown', { key: 'Enter' });
    await settle();
    return [...document.body.querySelectorAll<HTMLElement>('[role="menuitemcheckbox"], [role="menuitem"]')];
  };
  const choose = async (label: string): Promise<void> => {
    const entry = (await open()).find((item) => item.textContent?.includes(label));
    if (entry === undefined) {
      throw new Error(`no entry ${label}`);
    }
    entry.click();
    await settle();
  };
  return { ...result, commands, open, choose };
}

describe('the panels dock bar', () => {
  it('in its own window: only the Dock menu, with its icon, no fold and no close', async () => {
    const { wrapper } = await mounted({ position: 'floating' });
    const menu = wrapper.get('[data-testid="panel-dock-menu"]');
    expect(menu.attributes('data-icon')).toBe('i-lucide-app-window');
    // Label in name (RB-090-23, WCAG 2.5.3): "Dock" is the name, the place its description.
    expect(menu.text()).toBe('Dock');
    expect(menu.attributes('aria-label')).toBeUndefined();
    expect(menu.attributes('title')).toBe('Panel position: Own window');
    expect(wrapper.find('[data-testid="panel-dock-collapse"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="panel-dock-close"]').exists()).toBe(false);
    // Smaller and Larger are gone (RB-080-61); the bar between views and panels resizes.
    expect(wrapper.find('[data-testid="panel-dock-larger"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="panel-dock-smaller"]').exists()).toBe(false);
  });

  it('the Dock menu is a Nuxt UI menu: an icon per place, the current one ticked, then Hide', async () => {
    const { open } = await mounted({ position: 'right' });
    const entries = await open();
    expect(entries.map((entry) => entry.textContent?.trim())).toEqual([
      'Dock left',
      'Dock below',
      'Dock right',
      'Own window',
      'Hide the panels',
    ]);
    const checked = entries.filter((entry) => entry.getAttribute('aria-checked') === 'true');
    expect(checked.map((entry) => entry.textContent?.trim())).toEqual(['Dock right']);
    // Every entry carries its icon (screen 18: the native select had none).
    for (const entry of entries) {
      expect(entry.querySelector('[data-slot="itemLeadingIcon"]')).not.toBeNull();
    }
  });

  it("names each place's shortcut in its entry", async () => {
    const { open, mock } = await mounted({ position: 'right' });
    mock.emit({
      type: 'commands',
      leader: 'Ctrl+Shift+Space',
      commands: [
        command('panels.dock.left', 'Ctrl+Shift+Space P Left'),
        command('panels.undock', 'Ctrl+Shift+Space P Up'),
      ],
    });
    await settle();
    const entries = await open();
    const kbds = (index: number) =>
      [...(entries[index]?.querySelectorAll('[data-slot="itemTrailingKbds"] kbd') ?? [])].map((kbd) =>
        kbd.textContent?.trim(),
      );
    expect(kbds(0)).toEqual(['Ctrl+Shift+Space', 'P', '←']);
    expect(kbds(3)).toEqual(['Ctrl+Shift+Space', 'P', '↑']);
    expect(kbds(1)).toEqual([]);
  });

  it('the menu runs the dock command with the selected panel', async () => {
    const { commands, choose } = await mounted({ position: 'floating' });
    await choose('Dock below');
    await choose('Dock left');
    expect(commands()).toEqual([
      { type: 'runCommand', id: 'panels.dock.bottom', args: { plugin: 'network-panel', panel: 'main' } },
      { type: 'runCommand', id: 'panels.dock.left', args: { plugin: 'network-panel', panel: 'main' } },
    ]);
  });

  it('Hide the panels hides them, whatever Close does', async () => {
    const { commands, choose } = await mounted({ position: 'left' });
    await choose('Hide the panels');
    expect(commands()).toEqual([{ type: 'runCommand', id: 'panels.close', args: { to: 'hidden' } }]);
  });

  it('docked: folds and closes by icon buttons with their names', async () => {
    const { wrapper, commands } = await mounted({ position: 'right' }, null);
    expect(wrapper.get('[data-testid="panel-dock-menu"]').attributes('data-icon')).toBe('i-lucide-panel-right');
    const collapse = wrapper.get('[data-testid="panel-dock-collapse"]');
    expect(collapse.attributes('aria-label')).toBe('Collapse the panels');
    expect(collapse.attributes('title')).toBe('Collapse the panels');
    expect(collapse.attributes('aria-expanded')).toBe('true');
    const close = wrapper.get('[data-testid="panel-dock-close"]');
    expect(close.attributes('aria-label')).toBe('Close the panels');
    expect(close.attributes('title')).toBe('Close the panels');
    expect(close.text()).toBe('');
    await collapse.trigger('click');
    await close.trigger('click');
    expect(commands()).toEqual([
      { type: 'runCommand', id: 'panels.dock.toggle', args: {} },
      { type: 'runCommand', id: 'panels.close', args: {} },
    ]);
  });
});

describe('the Dock menu keeps what it does not change (review point 6)', () => {
  it('choosing the current place runs nothing; another side sends no fold', async () => {
    const { commands, choose } = await mounted({ position: 'right' });
    await choose('Dock right');
    expect(commands()).toEqual([]);
    await choose('Dock left');
    expect(commands()).toEqual([
      { type: 'runCommand', id: 'panels.dock.left', args: { plugin: 'network-panel', panel: 'main' } },
    ]);
  });
});
