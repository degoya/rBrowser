// A toolbar pulldown as the core's native menu (RB-090-41): the request names the button's
// rectangle and the rows, the row chosen comes back once the menu closed, and the keyboard focus
// goes back to the button - with a choice and without one (Esc).
import { afterEach, describe, expect, it } from 'vitest';
import { effectScope } from 'vue';
import type { CoreToShell, NativeMenuRow, ShellToCore } from '../ipc/protocol';
import { MAX_ROWS, MAX_TEXT, capRows, countRows, isOpeningKey, useNativeMenu } from './nativeMenu';

const ROWS: NativeMenuRow[] = [
  { kind: 'item', id: 'light', label: 'Light', checked: false },
  { kind: 'item', id: 'dark', label: 'Dark', checked: true },
];

function harness(accepts = true) {
  const sent: ShellToCore[] = [];
  let push: (message: CoreToShell) => void = () => undefined;
  const scope = effectScope();
  const menu = scope.run(() =>
    useNativeMenu('theme', {
      send: (message) => {
        sent.push(message);
        return Promise.resolve(accepts);
      },
      subscribe: (listener) => {
        push = listener;
        return () => {
          push = () => undefined;
        };
      },
    }),
  );
  if (menu === undefined) throw new Error('no menu');
  const span = document.createElement('span');
  const button = document.createElement('button');
  span.append(button);
  document.body.append(span);
  span.getBoundingClientRect = () =>
    ({ left: 600.4, top: 6, width: 32.2, height: 31.6, right: 632, bottom: 38 }) as DOMRect;
  return { sent, push: (message: CoreToShell) => push(message), menu, span, button, scope };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('a toolbar pulldown as a native menu', () => {
  it('asks the core for the rows under its button', async () => {
    const { sent, menu, span } = harness();
    await menu.open(span, ROWS, () => undefined);
    expect(sent).toHaveLength(1);
    const request = sent[0];
    expect(request).toMatchObject({
      type: 'nativeMenu',
      anchor: { x: 600, y: 6, width: 32, height: 32 },
      rows: ROWS,
    });
    expect(request?.type === 'nativeMenu' && request.menu).toMatch(/^theme-\d+$/);
  });

  it('runs the row chosen once, and gives the focus back to the button', async () => {
    const { sent, menu, span, button, push } = harness();
    const chosen: string[] = [];
    await menu.open(span, ROWS, (id) => chosen.push(id));
    const name = sent[0]?.type === 'nativeMenu' ? sent[0].menu : '';
    expect(menu.isOpen()).toBe(true);
    push({ type: 'nativeMenuClosed', menu: 'another', item: 'light' });
    expect(chosen).toEqual([]);
    push({ type: 'nativeMenuClosed', menu: name, item: 'dark' });
    push({ type: 'nativeMenuClosed', menu: name, item: 'dark' });
    expect(chosen).toEqual(['dark']);
    expect(document.activeElement).toBe(button);
    expect(menu.isOpen()).toBe(false);
  });

  it('chooses nothing when the menu closes without a choice (Esc), the focus back all the same', async () => {
    const { sent, menu, span, button, push } = harness();
    const chosen: string[] = [];
    await menu.open(span, ROWS, (id) => chosen.push(id));
    const name = sent[0]?.type === 'nativeMenu' ? sent[0].menu : '';
    push({ type: 'nativeMenuClosed', menu: name });
    expect(chosen).toEqual([]);
    expect(document.activeElement).toBe(button);
  });

  it('is not open when the core refused the menu', async () => {
    const { menu, span } = harness(false);
    await menu.open(span, ROWS, () => undefined);
    expect(menu.isOpen()).toBe(false);
  });

  it('gives two pulldowns of one kind names of their own', async () => {
    const one = harness();
    const two = harness();
    await one.menu.open(one.span, ROWS, () => undefined);
    await two.menu.open(two.span, ROWS, () => undefined);
    const names = [one.sent[0], two.sent[0]].map((m) => (m?.type === 'nativeMenu' ? m.menu : ''));
    expect(new Set(names).size).toBe(2);
  });

  // RB-090-54: a click on the button while its menu is open closes that menu (the core says so
  // later) and opens it again. The late close of the first menu is not the second's: each opening
  // has a name of its own, and the choice made in the second menu still runs.
  it('keeps a second opening of the same pulldown apart from the first one\'s late close', async () => {
    const { sent, menu, span, push } = harness();
    const first: string[] = [];
    const second: string[] = [];
    await menu.open(span, ROWS, (id) => first.push(id));
    await menu.open(span, ROWS, (id) => second.push(id));
    const [one, two] = sent.map((m) => (m.type === 'nativeMenu' ? m.menu : ''));
    expect(one).toMatch(/^theme-\d+$/);
    expect(two).toMatch(/^theme-\d+$/);
    expect(two).not.toBe(one);
    push({ type: 'nativeMenuClosed', menu: one ?? '', item: 'light' });
    expect(menu.isOpen()).toBe(true);
    push({ type: 'nativeMenuClosed', menu: two ?? '', item: 'dark' });
    expect(first).toEqual([]);
    expect(second).toEqual(['dark']);
    expect(menu.isOpen()).toBe(false);
  });

  it('keeps the second opening open when the first one was refused late', async () => {
    let accept = true;
    const sent: ShellToCore[] = [];
    let resolveFirst: (shown: boolean) => void = () => undefined;
    const scope = effectScope();
    const menu = scope.run(() =>
      useNativeMenu('theme', {
        send: (message) => {
          sent.push(message);
          if (accept) {
            accept = false;
            return new Promise<boolean>((resolve) => {
              resolveFirst = resolve;
            });
          }
          return Promise.resolve(true);
        },
        subscribe: () => () => undefined,
      }),
    );
    if (menu === undefined) throw new Error('no menu');
    const span = document.createElement('span');
    document.body.append(span);
    const first = menu.open(span, ROWS, () => undefined);
    await menu.open(span, ROWS, () => undefined);
    resolveFirst(false);
    await first;
    expect(menu.isOpen()).toBe(true);
    scope.stop();
  });

  it('opens on the arrow keys and F4 too (Enter and Space are the button\'s click)', () => {
    const key = (k: string, extra: KeyboardEventInit = {}) => new KeyboardEvent('keydown', { key: k, ...extra });
    expect(isOpeningKey(key('ArrowDown'))).toBe(true);
    expect(isOpeningKey(key('ArrowDown', { altKey: true }))).toBe(true);
    expect(isOpeningKey(key('ArrowUp'))).toBe(true);
    expect(isOpeningKey(key('F4'))).toBe(true);
    expect(isOpeningKey(key('Escape'))).toBe(false);
    expect(isOpeningKey(key('Tab'))).toBe(false);
    expect(isOpeningKey(key('ArrowDown', { ctrlKey: true }))).toBe(false);
    expect(isOpeningKey(key('d'))).toBe(false);
  });

  it('stops listening with its scope', async () => {
    const { sent, menu, span, push, scope } = harness();
    const chosen: string[] = [];
    await menu.open(span, ROWS, (id) => chosen.push(id));
    const name = sent[0]?.type === 'nativeMenu' ? sent[0].menu : '';
    scope.stop();
    push({ type: 'nativeMenuClosed', menu: name, item: 'dark' });
    expect(chosen).toEqual([]);
  });

  // RB-090-45: the core refuses a menu with a label over its bound; a long bookmark title is
  // shortened instead, so the menu still opens.
  it('shortens a label longer than the core takes', async () => {
    const { sent, menu, span } = harness();
    const long = 'x'.repeat(MAX_TEXT + 40);
    await menu.open(span, [{ kind: 'item', id: 'a', label: long }], () => undefined);
    const row = sent[0]?.type === 'nativeMenu' ? sent[0].rows[0] : undefined;
    const label = row?.kind === 'item' ? row.label : '';
    expect([...label]).toHaveLength(MAX_TEXT);
    expect(label.endsWith('\u2026')).toBe(true);
  });

  it('never asks for more rows than the core takes', async () => {
    const { sent, menu, span } = harness();
    const many: NativeMenuRow[] = Array.from({ length: MAX_ROWS + 5 }, (_, i) => ({
      kind: 'item',
      id: `r${i}`,
      label: `Row ${i}`,
    }));
    await menu.open(span, many, () => undefined);
    expect(sent[0]?.type === 'nativeMenu' && sent[0].rows).toHaveLength(MAX_ROWS);
  });
});

// RB-090-63: the main menu's submenus are bounded as the menu is - a long label shortened, a
// submenu's rows within the core's bound - and the core counts every level's rows.
describe('a menu with submenus', () => {
  it('bounds the labels and rows of a submenu too', async () => {
    const { sent, menu, span } = harness();
    const long = 'y'.repeat(MAX_TEXT + 3);
    const rows: NativeMenuRow[] = [
      {
        kind: 'submenu',
        label: long,
        rows: Array.from({ length: MAX_ROWS + 2 }, (_, i) => ({ kind: 'item', id: `s${i}`, label: long })),
      },
    ];
    await menu.open(span, rows, () => undefined);
    const sub = sent[0]?.type === 'nativeMenu' ? sent[0].rows[0] : undefined;
    expect(sub?.kind).toBe('submenu');
    if (sub?.kind !== 'submenu') return;
    expect([...sub.label]).toHaveLength(MAX_TEXT);
    expect(sub.rows).toHaveLength(MAX_ROWS);
    const first = sub.rows[0];
    expect(first?.kind === 'item' && [...first.label].length).toBe(MAX_TEXT);
  });

  // RB-090-63 review: what the core has no room for is one row that opens the command palette.
  it('ends a menu too long for the core with a row that opens the palette', async () => {
    const { sent, menu, span, push } = harness();
    const chosen: string[] = [];
    const rows: NativeMenuRow[] = [
      {
        kind: 'submenu',
        label: 'S',
        rows: Array.from({ length: MAX_ROWS + 10 }, (_, i) => ({ kind: 'item', id: `s${i}`, label: `${i}` })),
      },
    ];
    await menu.open(span, rows, (id) => chosen.push(id));
    const request = sent[0];
    const sub = request?.type === 'nativeMenu' ? request.rows[0] : undefined;
    if (sub?.kind !== 'submenu') throw new Error('a submenu');
    expect(sub.rows).toHaveLength(MAX_ROWS);
    const last = sub.rows.at(-1);
    if (last?.kind !== 'item') throw new Error('an item');
    expect(last.label).toBe('\u2026');
    push({ type: 'nativeMenuClosed', menu: request?.type === 'nativeMenu' ? request.menu : '', item: last.id });
    expect(chosen).toEqual([]);
    expect(sent.at(-1)).toEqual({ type: 'runCommand', id: 'app.command_palette' });
  });

  it('counts the rows of every level', () => {
    expect(
      countRows([
        { kind: 'item', id: 'a', label: 'A' },
        { kind: 'submenu', label: 'S', rows: [{ kind: 'separator' }, { kind: 'item', id: 'b', label: 'B' }] },
      ]),
    ).toBe(4);
  });
});

describe('capRows', () => {
  const item = (id: string): NativeMenuRow => ({ kind: 'item', id, label: id });
  const more = (count: number): NativeMenuRow => ({ kind: 'item', id: 'more', label: `${count} more` });

  it('leaves rows that fit as they are', () => {
    const rows = [item('a'), item('b')];
    expect(capRows(rows, 2, more)).toEqual(rows);
  });

  it('ends a long list with a row that counts what was cut, no heading or line dangling', () => {
    const rows: NativeMenuRow[] = [
      item('a'),
      { kind: 'separator' },
      { kind: 'heading', label: 'Shop' },
      item('b'),
      item('c'),
    ];
    expect(capRows(rows, 4, more)).toEqual([item('a'), { kind: 'item', id: 'more', label: '2 more' }]);
    expect(capRows(rows, 5, more)).toEqual(rows);
    const cut = capRows([...rows, item('d')], 5, more);
    expect(cut).toHaveLength(5);
    expect(cut.at(-1)).toEqual({ kind: 'item', id: 'more', label: '2 more' });
  });
});
