// The menu coverage (RB-090-63), as the MCP coverage holds the tools: every command of the
// registry - core (`schema/commands.v1.json`) and every bundled plugin's `[[commands]]` - has a
// visible way or a reason why not. A new command without either fails here.

import { describe, expect, it } from 'vitest';
import type { CommandInfo, Group } from '../ipc/protocol';
import { BUTTONS } from './buttonMenus';
import { MENU_OMITTED, OTHER_WAYS } from './coverage';
import { MAIN_MENU } from './mainMenu';
import { buildMenu, isPluginCommand, namedCommands, pluginGroups, type MenuContext } from './menuModel';
import { coreCommands, pluginCommands } from './testRegistry';

const TREES = [MAIN_MENU, ...Object.values(BUTTONS).map((button) => button.menu)];

function named(): Set<string> {
  const out = new Set<string>();
  for (const tree of TREES) {
    namedCommands(tree).forEach((id) => out.add(id));
  }
  Object.values(BUTTONS).forEach((button) => out.add(button.command));
  return out;
}

describe('the menu coverage', () => {
  it('gives every core command a menu row, another visible way or a reason', () => {
    const commands = coreCommands();
    expect(commands.length).toBeGreaterThan(150);
    const rows = named();
    const without = commands
      .map((command) => command.id)
      .filter((id) => !rows.has(id) && OTHER_WAYS[id] === undefined && MENU_OMITTED[id] === undefined);
    expect(without).toEqual([]);
  });

  it('names only commands that exist, each with one kind of way', () => {
    const known = new Set([...coreCommands(), ...pluginCommands()].map((command) => command.id));
    const rows = named();
    expect([...rows].filter((id) => !known.has(id))).toEqual([]);
    for (const table of [OTHER_WAYS, MENU_OMITTED]) {
      for (const [id, why] of Object.entries(table)) {
        expect(known.has(id), id).toBe(true);
        expect(rows.has(id), `${id} has a row and an entry`).toBe(false);
        expect(why.trim().length, id).toBeGreaterThan(8);
      }
    }
    for (const id of Object.keys(OTHER_WAYS)) {
      expect(MENU_OMITTED[id], id).toBeUndefined();
    }
  });

  // The owner (2026-11-21): no feature hides behind a shortcut or the palette. Only the main
  // menu's own opener and the numbered families another row lists may go without a way.
  it('lets only the main menu opener and the numbered families go without a menu row', () => {
    const allowed = (id: string) =>
      id === 'ui.toolbar.overflow.open' || /^layout\.preset\.[1-9]$/.test(id) || /^environments\.switch\.[1-9]$/.test(id);
    const others = Object.keys(MENU_OMITTED).filter((id) => !allowed(id));
    expect(others, 'a feature needs a menu row or a visible control (OTHER_WAYS), not an omission').toEqual([]);
  });

  // RB-094-20: the devices, the adb download and the AI agent's setup have rows of their own.
  it('lists the devices, the adb download, iOS, the MCP setup and the presentation in the main menu', () => {
    const rows = namedCommands(MAIN_MENU);
    for (const id of ['view.add_android', 'tools.adb.install', 'ios.devices', 'mcp.setup', 'portal.present.start']) {
      expect(rows.has(id), id).toBe(true);
    }
  });

  it('gives every plugin command a row: by name or in the submenu of its leader group', () => {
    const groups = pluginGroups(MAIN_MENU);
    const commands = pluginCommands();
    expect(commands.length).toBeGreaterThan(40);
    const rows = named();
    for (const command of commands) {
      expect(rows.has(command.id) || groups.has(command.group), `${command.id} (${command.group})`).toBe(true);
    }
    // And the main menu shows each, built from the registry as the plugin host gives it.
    const infos: CommandInfo[] = commands.map((command) => ({
      id: command.id,
      titleKey: command.id,
      group: command.group,
      bindings: [],
      defaultBindings: [],
      changed: false,
      mcp: { omitted: 'test' },
    }));
    const ctx: MenuContext = { t: (key) => key, state: null, settings: {}, commands: infos };
    const built = buildMenu(MAIN_MENU, ctx);
    const shown = new Set(
      [...built.actions.values()].flatMap((message) => (message.type === 'runCommand' ? [message.id] : [])),
    );
    for (const command of commands) {
      expect(shown.has(command.id), command.id).toBe(true);
    }
  });

  it('takes in the plugin commands of every leader group', () => {
    const groups: Group[] = [
      'breakpoints',
      'layout',
      'sync',
      'environments',
      'devtools',
      'overlays',
      'capture',
      'checks',
      'reload',
      'panels',
      'flows',
      'view',
      'plugins',
      'settings',
    ];
    const taken = pluginGroups(MAIN_MENU);
    expect(groups.filter((group) => !taken.has(group))).toEqual([]);
    expect([...named()].filter(isPluginCommand).length).toBeGreaterThan(0);
  });
});
