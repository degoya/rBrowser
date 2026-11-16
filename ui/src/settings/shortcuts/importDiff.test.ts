import { describe, expect, it } from 'vitest';
import type { CommandInfo } from '../../ipc/protocol';
import { diffImport, parseShortcutFile } from './importDiff';

function command(id: string, defaults: string[], bindings = defaults): CommandInfo {
  return {
    id,
    titleKey: id,
    group: 'sync',
    bindings,
    defaultBindings: defaults,
    changed: bindings !== defaults,
    mcp: { omitted: 'x' },
  };
}

describe('parseShortcutFile', () => {
  it('reads a shortcut file of version 1', () => {
    expect(parseShortcutFile('{"version":1,"bindings":{"sync.toggle":["Ctrl+Shift+F9"],"reload.all":[]}}')).toEqual({
      ok: true,
      bindings: { 'sync.toggle': ['Ctrl+Shift+F9'], 'reload.all': [] },
    });
  });

  it('refuses what is not JSON, another version and a wrong shape', () => {
    expect(parseShortcutFile('{nope')).toEqual({ ok: false, errorKey: 'settings.shortcuts.import_error.json' });
    expect(parseShortcutFile('{"version":2,"bindings":{}}')).toEqual({
      ok: false,
      errorKey: 'settings.shortcuts.import_error.version',
    });
    expect(parseShortcutFile('{"bindings":{}}')).toEqual({
      ok: false,
      errorKey: 'settings.shortcuts.import_error.version',
    });
    for (const bad of [
      '[]',
      '{"version":1}',
      '{"version":1,"bindings":[]}',
      '{"version":1,"bindings":{"a":"Ctrl+X"}}',
      '{"version":1,"bindings":{"a":[3]}}',
      '{"version":1,"bindings":{"a":[""]}}',
      '{"version":1,"bindings":{"":["F9"]}}',
    ]) {
      expect(parseShortcutFile(bad)).toEqual({ ok: false, errorKey: 'settings.shortcuts.import_error.shape' });
    }
  });
});

describe('diffImport', () => {
  const commands = [
    command('sync.toggle', ['Ctrl+Shift+Space S S']),
    command('reload.all', ['Ctrl+Shift+Space R R'], ['Ctrl+Shift+F5']),
    command('view.zoom_in', ['Ctrl+Shift+Space V I']),
  ];

  it('lists what changes; a command the file omits goes back to its defaults', () => {
    const diff = diffImport(commands, { 'sync.toggle': ['Ctrl+Shift+F9'] });
    expect(diff.changes).toEqual([
      { id: 'sync.toggle', before: ['Ctrl+Shift+Space S S'], after: ['Ctrl+Shift+F9'] },
      { id: 'reload.all', before: ['Ctrl+Shift+F5'], after: ['Ctrl+Shift+Space R R'] },
    ]);
    expect(diff.unknown).toEqual([]);
    expect(diff.conflicts).toEqual([]);
  });

  it('names the commands it does not know and the bindings two commands would hold', () => {
    const diff = diffImport(commands, {
      'reload.all': ['Ctrl+Shift+F5'],
      'view.zoom_in': ['Ctrl+Shift+Space S S'],
      'plugin.gone.x': ['F8'],
    });
    expect(diff.unknown).toEqual(['plugin.gone.x']);
    expect(diff.conflicts).toEqual([
      { binding: 'Ctrl+Shift+Space S S', first: 'sync.toggle', second: 'view.zoom_in' },
    ]);
  });

  it('changes nothing when the file holds the current bindings', () => {
    expect(diffImport(commands, { 'reload.all': ['Ctrl+Shift+F5'] }).changes).toEqual([]);
  });
});
