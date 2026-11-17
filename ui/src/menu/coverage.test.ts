// The menu coverage (RB-090-63), as the MCP coverage holds the tools: every command of the
// registry - core (`schema/commands.v1.json`) and every bundled plugin's `[[commands]]` - has a
// visible way or a reason why not. A new command without either fails here.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { CommandInfo, Group } from '../ipc/protocol';
import { BUTTONS } from './buttonMenus';
import { MENU_OMITTED, OTHER_WAYS } from './coverage';
import { MAIN_MENU } from './mainMenu';
import { buildMenu, isPluginCommand, namedCommands, pluginGroups, type MenuContext } from './menuModel';

// The repository root: the tests run in ui/ (`npm test`).
const ROOT = join(process.cwd(), '..');

/** The core's commands as the registry writes them. */
function coreCommands(): { id: string; group: Group }[] {
  const file = JSON.parse(readFileSync(join(ROOT, 'schema/commands.v1.json'), 'utf-8')) as {
    commands: { id: string; group: Group }[];
  };
  return file.commands;
}

/** Every bundled plugin's commands, as the plugin host names them (`plugin.<id_>.<command>`). */
function pluginCommands(): { id: string; group: Group }[] {
  const out: { id: string; group: Group }[] = [];
  const plugins = join(ROOT, 'plugins');
  for (const dir of readdirSync(plugins)) {
    const path = join(plugins, dir, 'manifest.toml');
    if (!existsSync(path)) {
      continue;
    }
    const text = readFileSync(path, 'utf-8');
    const plugin = /^id = "([^"]+)"/m.exec(text)?.[1] ?? dir;
    for (const block of text.split(/^\[\[commands\]\]$/m).slice(1)) {
      const table = block.split(/^\[/m)[0] ?? '';
      const id = /^id = "([^"]+)"/m.exec(table)?.[1] ?? '?';
      const group = (/^group = "([^"]+)"/m.exec(table)?.[1] ?? 'plugins') as Group;
      out.push({ id: `plugin.${plugin.replaceAll('-', '_')}.${id}`, group });
    }
  }
  return out;
}

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
