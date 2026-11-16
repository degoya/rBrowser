// What the shortcut editor lists (RB-060-11): the text search over title, id and bindings, the
// search by key press (the command on that combination; after a leader prefix every chord it
// starts), "only changed", and the view by leader group or as one flat list.

import type { CommandInfo, Group } from '../../ipc/protocol';

export interface CommandFilter {
  query?: string;
  /** A recorded binding: the commands on it, or on a chord it starts. */
  pressed?: string | null;
  onlyChanged?: boolean;
  t: (key: string) => string;
}

export interface CommandSection {
  /** `null` for the flat list. */
  group: Group | null;
  commands: CommandInfo[];
}

function matchesText(command: CommandInfo, words: string[], t: (key: string) => string): boolean {
  const haystack = [t(command.titleKey), command.id, ...command.bindings].join(' ').toLowerCase();
  return words.every((word) => haystack.includes(word));
}

export function filterCommands(commands: readonly CommandInfo[], filter: CommandFilter): CommandInfo[] {
  const words = (filter.query ?? '').toLowerCase().split(/\s+/).filter((word) => word !== '');
  const pressed = filter.pressed ?? null;
  return commands.filter(
    (command) =>
      (!filter.onlyChanged || command.changed) &&
      matchesText(command, words, filter.t) &&
      (pressed === null || command.bindings.some((binding) => binding === pressed || binding.startsWith(`${pressed} `))),
  );
}

/** By leader group in the registry's order, or (`flat`) one list sorted by title. */
export function groupCommands(commands: readonly CommandInfo[], flat: boolean, t: (key: string) => string): CommandSection[] {
  if (flat) {
    const sorted = [...commands].sort((a, b) => t(a.titleKey).localeCompare(t(b.titleKey)));
    return sorted.length === 0 ? [] : [{ group: null, commands: sorted }];
  }
  const groups = new Map<Group, CommandInfo[]>();
  for (const command of commands) {
    groups.set(command.group, [...(groups.get(command.group) ?? []), command]);
  }
  return [...groups.entries()].map(([group, list]) => ({ group, commands: list }));
}

/**
 * The defaults a plugin brought that stayed unbound (taken or reserved). The core names them in
 * `droppedDefaults` of the command (RB-060-11's addition to `CommandInfo`); without it, none.
 */
export function droppedDefaults(command: CommandInfo): string[] {
  const dropped = (command as CommandInfo & { droppedDefaults?: unknown }).droppedDefaults;
  return Array.isArray(dropped) ? dropped.filter((item): item is string => typeof item === 'string') : [];
}
