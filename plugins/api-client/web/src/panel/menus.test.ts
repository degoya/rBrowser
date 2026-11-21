import { describe, expect, it } from 'vitest';
import { type TreeAction, folderMenu, requestMenu } from './menus';

const t = (key: string): string => key;

function labels(groups: { label?: string }[][]): (string | undefined)[][] {
  return groups.map((group) => group.map((item) => item.label));
}

describe("the collections' context menus (RB-096-11)", () => {
  it('offers a request open, duplicate, rename, move and delete', () => {
    const done: TreeAction[] = [];
    const menu = requestMenu('users/me.http', t, (action) => done.push(action));
    expect(labels(menu)).toEqual([
      ['panel.menu.open', 'panel.menu.duplicate', 'panel.menu.rename', 'panel.menu.move'],
      ['panel.menu.delete'],
    ]);
    for (const item of menu.flat()) item.onSelect?.(new Event('select'));
    expect(done.map((action) => action.kind)).toEqual(['open', 'duplicate', 'rename', 'move', 'delete']);
    expect(done[0]).toEqual({ kind: 'open', id: 'users/me.http' });
    expect(menu[1]?.[0]?.color).toBe('error');
  });

  it('offers a collection a new request, a collection inside, rename, duplicate, export and delete', () => {
    const done: TreeAction[] = [];
    const menu = folderMenu('users', t, (action) => done.push(action));
    expect(labels(menu)).toEqual([
      ['panel.menu.newRequest', 'panel.menu.newFolder'],
      ['panel.menu.rename', 'panel.menu.duplicate', 'panel.menu.export'],
      ['panel.menu.delete'],
    ]);
    for (const item of menu.flat()) item.onSelect?.(new Event('select'));
    expect(done).toEqual([
      { kind: 'newRequest', folder: 'users' },
      { kind: 'newFolder', folder: 'users' },
      { kind: 'renameFolder', folder: 'users' },
      { kind: 'duplicateFolder', folder: 'users' },
      { kind: 'exportFolder', folder: 'users' },
      { kind: 'deleteFolder', folder: 'users' },
    ]);
  });

  it('offers the top a new request and a new collection only', () => {
    expect(labels(folderMenu('', t, () => undefined))).toEqual([['panel.menu.newRequest', 'panel.menu.newFolder']]);
  });
});
