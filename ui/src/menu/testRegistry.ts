// The command registry as the tests of the menus read it (RB-090-63, RB-090-82): the core's
// commands from `schema/commands.v1.json`, every bundled plugin's `[[commands]]` as the plugin
// host names them, and the settings schema. Tests only; it reads the repository's files.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { CommandInfo, Group } from '../ipc/protocol';

// The repository root: the tests run in ui/ (`npm test`).
export const ROOT = join(process.cwd(), '..');

/** The core's commands as the registry writes them. */
export function coreCommands(): { id: string; group: Group }[] {
  const file = JSON.parse(readFileSync(join(ROOT, 'schema/commands.v1.json'), 'utf-8')) as {
    commands: { id: string; group: Group }[];
  };
  return file.commands;
}

/** Every bundled plugin's commands, as the plugin host names them (`plugin.<id_>.<command>`). */
export function pluginCommands(): { id: string; group: Group }[] {
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

/** Every command, core and plugins, as the core lists them to the shell. */
export function registryInfos(): CommandInfo[] {
  return [...coreCommands(), ...pluginCommands()].map((command) => ({
    id: command.id,
    titleKey: command.id,
    group: command.group,
    bindings: [],
    defaultBindings: [],
    changed: false,
    mcp: { omitted: 'test' },
  }));
}

/** Whether the user settings schema (`schema/settings.v1.json`) has the dotted `key`. */
export function hasSetting(key: string): boolean {
  type Node = { properties?: Record<string, Node> };
  let node: Node | undefined = JSON.parse(readFileSync(join(ROOT, 'schema/settings.v1.json'), 'utf-8')) as Node;
  for (const part of key.split('.')) {
    node = node?.properties?.[part];
  }
  return node !== undefined;
}
