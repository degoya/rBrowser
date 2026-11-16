import { describe, expect, it } from 'vitest';
import type { CommandInfo } from '../../ipc/protocol';
import { droppedDefaults, filterCommands, groupCommands } from './filter';

function command(id: string, group: CommandInfo['group'], bindings: string[], changed = false): CommandInfo {
  return { id, titleKey: `t.${id}`, group, bindings, defaultBindings: bindings, changed, mcp: { omitted: 'x' } };
}

const t = (key: string): string => ({ 't.sync.toggle': 'Toggle sync', 't.view.set_breakpoint.2': 'Breakpoint 2' })[key] ?? key;

const COMMANDS = [
  command('sync.toggle', 'sync', ['Ctrl+Shift+Space S S']),
  command('view.set_breakpoint.2', 'breakpoints', ['Ctrl+Shift+2', 'Ctrl+Shift+Space B 2'], true),
  command('reload.all', 'reload', ['Ctrl+Shift+Space R R']),
];

describe('filterCommands', () => {
  it('searches the title, the id and the bindings', () => {
    expect(filterCommands(COMMANDS, { query: 'toggle sync', t }).map((c) => c.id)).toEqual(['sync.toggle']);
    expect(filterCommands(COMMANDS, { query: 'reload.all', t }).map((c) => c.id)).toEqual(['reload.all']);
    expect(filterCommands(COMMANDS, { query: 'space b', t }).map((c) => c.id)).toEqual(['view.set_breakpoint.2']);
  });

  it('finds the command on a pressed combination, and the chords a leader prefix starts', () => {
    expect(filterCommands(COMMANDS, { pressed: 'Ctrl+Shift+2', t }).map((c) => c.id)).toEqual(['view.set_breakpoint.2']);
    expect(filterCommands(COMMANDS, { pressed: 'Ctrl+Shift+Space S', t }).map((c) => c.id)).toEqual(['sync.toggle']);
    expect(filterCommands(COMMANDS, { pressed: 'Ctrl+Shift+F12', t })).toEqual([]);
  });

  it('keeps only the changed commands on request', () => {
    expect(filterCommands(COMMANDS, { onlyChanged: true, t }).map((c) => c.id)).toEqual(['view.set_breakpoint.2']);
  });
});

describe('groupCommands', () => {
  it('groups by leader group in the registry order, or one flat list by title', () => {
    expect(groupCommands(COMMANDS, false, t).map((g) => [g.group, g.commands.length])).toEqual([
      ['sync', 1],
      ['breakpoints', 1],
      ['reload', 1],
    ]);
    const [flat] = groupCommands(COMMANDS, true, t);
    expect(flat?.group).toBeNull();
    expect(flat?.commands.map((c) => c.id)).toEqual(['view.set_breakpoint.2', 'reload.all', 'sync.toggle']);
  });
});

describe('droppedDefaults', () => {
  it('reads the defaults the registry could not bind, when the core names them', () => {
    expect(droppedDefaults(COMMANDS[0] as CommandInfo)).toEqual([]);
    expect(droppedDefaults({ ...(COMMANDS[0] as CommandInfo), droppedDefaults: ['Ctrl+W'] } as CommandInfo)).toEqual(['Ctrl+W']);
  });
});
