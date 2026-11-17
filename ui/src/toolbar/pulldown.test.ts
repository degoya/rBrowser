// RB-090-57: the environment and layout preset pulldowns name each row by its place in the menu,
// never by the environment's or preset's name - the core takes row ids of at most `MAX_TEXT`
// characters, and one long name must not break the whole menu (as the bookmarks, RB-090-54).
import { describe, expect, it } from 'vitest';
import { MAX_TEXT } from './menuBounds';
import { choiceMenu } from './pulldown';

describe('a pulldown of choices', () => {
  it('names each row by its place and maps the row back to its choice', () => {
    const menu = choiceMenu();
    const long = 'e'.repeat(MAX_TEXT + 44);
    const rows = menu.rows(
      [
        { value: 'local', label: 'local' },
        { value: long, label: long },
      ],
      long,
    );
    expect(rows).toEqual([
      { kind: 'item', id: 'choice:0', label: 'local', checked: false },
      { kind: 'item', id: 'choice:1', label: long, checked: true },
    ]);
    for (const row of rows) {
      expect(row.kind === 'item' && [...row.id].length <= MAX_TEXT).toBe(true);
    }
    expect(menu.valueOf('choice:0')).toBe('local');
    expect(menu.valueOf('choice:1')).toBe(long);
  });

  it('chooses nothing for a row id it did not build', () => {
    const menu = choiceMenu();
    expect(menu.valueOf('choice:0')).toBeUndefined();
    menu.rows([{ value: 'local', label: 'local' }], null);
    for (const id of ['choice:1', 'choice:', 'choice:-1', 'choice:0x0', 'choice:01', 'local', '']) {
      expect(menu.valueOf(id), id).toBeUndefined();
    }
  });

  it('maps a row to the choices it was built from, not to later ones', () => {
    const menu = choiceMenu();
    menu.rows(
      [
        { value: 'a', label: 'a' },
        { value: 'b', label: 'b' },
      ],
      null,
    );
    expect(menu.valueOf('choice:1')).toBe('b');
    menu.rows([{ value: 'c', label: 'c' }], 'c');
    expect(menu.valueOf('choice:0')).toBe('c');
    expect(menu.valueOf('choice:1')).toBeUndefined();
  });
});
