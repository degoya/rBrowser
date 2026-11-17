import { describe, expect, it } from 'vitest';
import type { CommandInfo } from '../../ipc/protocol';
import { bindingsConflict, resolveConflict } from './conflict';

function command(id: string, bindings: string[]): CommandInfo {
  return { id, titleKey: id, group: 'sync', bindings, defaultBindings: bindings, changed: false, mcp: { omitted: 'x' } };
}

describe('bindingsConflict', () => {
  it('holds equal bindings and a chord that starts another', () => {
    expect(bindingsConflict('Ctrl+Shift+F9', 'Ctrl+Shift+F9')).toBe(true);
    expect(bindingsConflict('Ctrl+Shift+Space S', 'Ctrl+Shift+Space S S')).toBe(true);
    expect(bindingsConflict('Ctrl+Shift+Space S S', 'Ctrl+Shift+Space S')).toBe(true);
    expect(bindingsConflict('Ctrl+Shift+Space S S', 'Ctrl+Shift+Space S N')).toBe(false);
    expect(bindingsConflict('Ctrl+Shift+2', 'Ctrl+Shift+Space 2')).toBe(false);
  });
});

describe('resolveConflict', () => {
  const sync = command('sync.toggle', ['Ctrl+Shift+Space S S']);
  const reload = command('reload.all', ['Ctrl+Shift+Space R R', 'Ctrl+Shift+F5']);

  it('replaces: the other command loses the binding, this one gains it', () => {
    expect(resolveConflict('replace', reload, sync, 'Ctrl+Shift+Space S S')).toEqual([
      { id: 'sync.toggle', bindings: [] },
      { id: 'reload.all', bindings: ['Ctrl+Shift+Space R R', 'Ctrl+Shift+F5', 'Ctrl+Shift+Space S S'] },
    ]);
  });

  it('frees a chord that only starts the new binding too', () => {
    const group = command('sync.menu', ['Ctrl+Shift+Space S']);
    expect(resolveConflict('replace', reload, group, 'Ctrl+Shift+Space S S')[0]).toEqual({ id: 'sync.menu', bindings: [] });
  });

  it('swaps: the other command takes this one\'s first binding', () => {
    expect(resolveConflict('swap', reload, sync, 'Ctrl+Shift+Space S S')).toEqual([
      { id: 'sync.toggle', bindings: [] },
      { id: 'reload.all', bindings: ['Ctrl+Shift+Space S S', 'Ctrl+Shift+F5'] },
      { id: 'sync.toggle', bindings: ['Ctrl+Shift+Space R R'] },
    ]);
  });

  it('swaps like a replace when this command has no binding', () => {
    const bare = command('view.zoom_in', []);
    expect(resolveConflict('swap', bare, sync, 'Ctrl+Shift+Space S S')).toEqual(
      resolveConflict('replace', bare, sync, 'Ctrl+Shift+Space S S'),
    );
  });

  it('cancels with nothing to do', () => {
    expect(resolveConflict('cancel', reload, sync, 'Ctrl+Shift+Space S S')).toEqual([]);
  });
});
