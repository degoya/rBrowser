// The toolbar's menus as data (RB-090-63): the always-visible main menu `≡` (`mainMenu.ts`) and
// the split buttons' menus (DevTools, Screenshot, Video, Checks; `buttonMenus.ts`) are trees of
// registry commands. `buildMenu` turns a tree into the rows of the core's native menu
// (`nativeMenu.ts`) - a command's title from the registry, its shortcut beside it, a check or a
// radio mark from the window's state or a setting - and keeps what each row runs. Every row runs
// a command of the one command registry (`runCommand`) or the message a list row names (a
// bookmark, a layout preset); nothing here runs anything itself.
//
// A plugin's command is shown only while the registry has it (an enabled plugin); a plugin's
// commands not placed by name land in the submenu of their leader group (`plugins` nodes), so
// every command a plugin brings has a menu row without this file knowing it.

import type { CommandInfo, Group, JsonValue, NativeMenuRow, ShellToCore, StateSnapshot } from '../ipc/protocol';

/** `t` of vue-i18n, as far as the menus need it. */
export type Translate = (key: string, args?: Record<string, unknown>) => string;

/** What a tree is built from: the window's state, the settings in effect, the registry. */
export interface MenuContext {
  t: Translate;
  state: StateSnapshot | null;
  settings: Readonly<Record<string, JsonValue>>;
  /** The registry's commands (titles, bindings, plugin commands); empty until known. */
  commands: readonly CommandInfo[];
  /** Windows' theme, for the System choice's text. */
  systemTheme?: 'light' | 'dark';
}

/** A predicate over the context: a check row's state, whether a row can be chosen. */
export type When = (ctx: MenuContext) => boolean;

/** One node of a menu tree. */
export type MenuNode =
  /** A registry command; `checked` makes it a check row, with `radio` a choice. */
  | {
      kind: 'command';
      id: string;
      args?: JsonValue;
      /** A label of its own (an i18n key) instead of the command's title. */
      labelKey?: string;
      checked?: When;
      radio?: boolean;
      enabled?: When;
    }
  /** A row of its own: a list entry (a bookmark, a preset) with the message it sends. */
  | { kind: 'row'; label: string; message: ShellToCore; checked?: boolean; radio?: boolean; disabled?: boolean }
  | { kind: 'submenu'; labelKey: string; children: readonly MenuNode[] }
  | { kind: 'heading'; label: string }
  | { kind: 'separator' }
  /** Rows read from the state when the menu opens; `covers` names the commands they run. */
  | { kind: 'list'; covers: readonly string[]; build: (ctx: MenuContext) => MenuNode[] }
  /** The plugin commands of a leader group that no node names. */
  | { kind: 'plugins'; group: Group };

/** The rows for the core and what each choosable row sends. */
export interface BuiltMenu {
  rows: NativeMenuRow[];
  actions: Map<string, ShellToCore>;
}

/** The prefix of every plugin command id (`plugin.<plugin>.<command>`). */
const PLUGIN = 'plugin.';

/** Whether `id` is a plugin's command. */
export function isPluginCommand(id: string): boolean {
  return id.startsWith(PLUGIN);
}

/** Every command id a tree names, list nodes' `covers` included. */
export function namedCommands(nodes: readonly MenuNode[]): Set<string> {
  const out = new Set<string>();
  const walk = (list: readonly MenuNode[]) => {
    for (const node of list) {
      if (node.kind === 'command') {
        out.add(node.id);
      } else if (node.kind === 'submenu') {
        walk(node.children);
      } else if (node.kind === 'list') {
        node.covers.forEach((id) => out.add(id));
      }
    }
  };
  walk(nodes);
  return out;
}

/** The leader groups whose plugin commands a tree takes in. */
export function pluginGroups(nodes: readonly MenuNode[]): Set<Group> {
  const out = new Set<Group>();
  const walk = (list: readonly MenuNode[]) => {
    for (const node of list) {
      if (node.kind === 'plugins') {
        out.add(node.group);
      } else if (node.kind === 'submenu') {
        walk(node.children);
      }
    }
  };
  walk(nodes);
  return out;
}

/** A command's message: `runCommand`, with its arguments when it has some. */
export function commandMessage(id: string, args?: JsonValue): ShellToCore {
  return args === undefined ? { type: 'runCommand', id } : { type: 'runCommand', id, args };
}

/** Drops leading, trailing and doubled lines, and a heading with nothing under it. */
function tidy(rows: NativeMenuRow[]): NativeMenuRow[] {
  const out: NativeMenuRow[] = [];
  for (const row of rows) {
    if (row.kind === 'separator' && (out.length === 0 || out.at(-1)?.kind === 'separator')) {
      continue;
    }
    if (row.kind === 'separator' && out.at(-1)?.kind === 'heading') {
      out.pop();
    }
    out.push(row);
  }
  while (out.length > 0 && (out.at(-1)?.kind === 'separator' || out.at(-1)?.kind === 'heading')) {
    out.pop();
  }
  return out;
}

/** Builds `nodes` for `ctx`: the rows, and the message of every row that can be chosen. */
export function buildMenu(nodes: readonly MenuNode[], ctx: MenuContext, prefix = 'm'): BuiltMenu {
  const actions = new Map<string, ShellToCore>();
  const byId = new Map(ctx.commands.map((command) => [command.id, command]));
  const named = namedCommands(nodes);

  const add = (row: Omit<Extract<NativeMenuRow, { kind: 'item' }>, 'kind' | 'id'>, message: ShellToCore) => {
    const id = `${prefix}${actions.size}`;
    actions.set(id, message);
    return { kind: 'item', id, ...row } as NativeMenuRow;
  };

  const command = (node: Extract<MenuNode, { kind: 'command' }>): NativeMenuRow | null => {
    const info = byId.get(node.id);
    // A plugin's command only while its plugin is enabled.
    if (info === undefined && isPluginCommand(node.id)) {
      return null;
    }
    const label = ctx.t(node.labelKey ?? info?.titleKey ?? `commands.${node.id}`);
    const checked = node.checked?.(ctx);
    const accel = info?.bindings[0];
    return add(
      {
        label,
        ...(checked === undefined ? {} : { checked }),
        ...(node.radio === true && checked !== undefined ? { radio: true } : {}),
        ...(accel === undefined ? {} : { accel }),
        ...(node.enabled === undefined || node.enabled(ctx) ? {} : { disabled: true }),
      },
      commandMessage(node.id, node.args),
    );
  };

  const build = (list: readonly MenuNode[]): NativeMenuRow[] => {
    const rows: NativeMenuRow[] = [];
    for (const node of list) {
      switch (node.kind) {
        case 'command': {
          const row = command(node);
          if (row !== null) rows.push(row);
          break;
        }
        case 'row':
          rows.push(
            add(
              {
                label: node.label,
                ...(node.checked === undefined ? {} : { checked: node.checked }),
                ...(node.radio === true && node.checked !== undefined ? { radio: true } : {}),
                ...(node.disabled === true ? { disabled: true } : {}),
              },
              node.message,
            ),
          );
          break;
        case 'submenu': {
          const inner = build(node.children);
          if (inner.some((row) => row.kind === 'item' || row.kind === 'submenu')) {
            rows.push({ kind: 'submenu', label: ctx.t(node.labelKey), rows: inner });
          }
          break;
        }
        case 'heading':
          rows.push({ kind: 'heading', label: node.label });
          break;
        case 'separator':
          rows.push({ kind: 'separator' });
          break;
        case 'list':
          rows.push(...build(node.build(ctx)));
          break;
        case 'plugins':
          for (const info of ctx.commands) {
            if (isPluginCommand(info.id) && info.group === node.group && !named.has(info.id)) {
              const row = command({ kind: 'command', id: info.id });
              if (row !== null) rows.push(row);
            }
          }
          break;
      }
    }
    return tidy(rows);
  };

  return { rows: build(nodes), actions };
}

/** A boolean setting's value, `fallback` while it is not known. */
export function settingOn(ctx: MenuContext, key: string, fallback = false): boolean {
  const value = ctx.settings[key];
  return typeof value === 'boolean' ? value : fallback;
}
