// The toolbar's menus as data (RB-090-63): a tree of registry commands becomes the rows of the
// core's native menu - titles and shortcuts from the registry, check and radio marks from the
// state and the settings, submenus without rows dropped, lines tidied - and every row runs what
// it names.

import { describe, expect, it } from 'vitest';
import type { CommandInfo, NativeMenuRow } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { MAIN_MENU } from './mainMenu';
import { buildMenu, type MenuContext, type MenuNode } from './menuModel';

function info(id: string, group: CommandInfo['group'], bindings: string[] = []): CommandInfo {
  return { id, titleKey: `title.${id}`, group, bindings, defaultBindings: bindings, changed: false, mcp: { omitted: 'x' } };
}

function context(extra: Partial<MenuContext> = {}): MenuContext {
  return {
    t: (key) => key,
    state: sampleState(),
    settings: {},
    commands: [
      info('sync.toggle', 'sync', ['Ctrl+Shift+Space S S']),
      info('layout.mode.grid', 'layout'),
      info('plugin.overlays.stacking', 'overlays', ['Ctrl+Shift+Space O Z']),
    ],
    ...extra,
  };
}

const items = (rows: NativeMenuRow[]): Extract<NativeMenuRow, { kind: 'item' }>[] =>
  rows.flatMap((row) => (row.kind === 'item' ? [row] : row.kind === 'submenu' ? items(row.rows) : []));

describe('buildMenu', () => {
  it("names a command by the registry's title with its shortcut, and runs it", () => {
    const tree: MenuNode[] = [{ kind: 'command', id: 'sync.toggle', checked: () => true }];
    const built = buildMenu(tree, context());
    expect(built.rows).toEqual([
      { kind: 'item', id: 'm0', label: 'title.sync.toggle', checked: true, accel: 'Ctrl+Shift+Space S S' },
    ]);
    expect(built.actions.get('m0')).toEqual({ type: 'runCommand', id: 'sync.toggle' });
  });

  // RB-090-63 review: a radio row chooses - choosing the checked one again keeps it. Focus mode
  // is a toggle command, so its row says `enabled: true`.
  it('keeps focus mode on when its radio row is chosen again', () => {
    const sample = sampleState();
    const state = { ...sample, layout: { ...sample.layout, mode: 'focus' as const } };
    const built = buildMenu(MAIN_MENU, context({ state, commands: [info('layout.focus_mode', 'layout')] }));
    const row = items(built.rows).find((candidate) => candidate.label === 'title.layout.focus_mode');
    expect(row).toMatchObject({ radio: true, checked: true });
    expect(built.actions.get(row?.id ?? '')).toEqual({
      type: 'runCommand',
      id: 'layout.focus_mode',
      args: { enabled: true },
    });
  });

  it('draws a choice as a radio row and passes the arguments', () => {
    const tree: MenuNode[] = [
      { kind: 'command', id: 'layout.mode.grid', radio: true, checked: () => false, args: { a: 1 } },
    ];
    const built = buildMenu(tree, context());
    expect(built.rows[0]).toMatchObject({ radio: true, checked: false });
    expect(built.actions.get('m0')).toEqual({ type: 'runCommand', id: 'layout.mode.grid', args: { a: 1 } });
  });

  it("shows a plugin's command only while the registry has it, a core command always", () => {
    const tree: MenuNode[] = [
      { kind: 'command', id: 'plugin.peek.toggle' },
      { kind: 'command', id: 'app.about' },
    ];
    const rows = items(buildMenu(tree, context()).rows);
    expect(rows.map((row) => row.label)).toEqual(['commands.app.about']);
  });

  it('drops a submenu without rows and tidies the lines around what is left', () => {
    const tree: MenuNode[] = [
      { kind: 'separator' },
      { kind: 'submenu', labelKey: 'empty', children: [{ kind: 'command', id: 'plugin.gone.x' }] },
      { kind: 'separator' },
      { kind: 'command', id: 'sync.toggle' },
      { kind: 'separator' },
      { kind: 'separator' },
      { kind: 'heading', label: 'H' },
    ];
    const rows = buildMenu(tree, context()).rows;
    expect(rows.map((row) => row.kind)).toEqual(['item']);
  });

  it("puts a plugin's commands no node names into its leader group's submenu", () => {
    const tree: MenuNode[] = [{ kind: 'submenu', labelKey: 'overlays', children: [{ kind: 'plugins', group: 'overlays' }] }];
    const rows = buildMenu(tree, context()).rows;
    expect(rows[0]).toMatchObject({
      kind: 'submenu',
      label: 'overlays',
      rows: [{ kind: 'item', label: 'title.plugin.overlays.stacking', accel: 'Ctrl+Shift+Space O Z' }],
    });
  });
});

describe('the main menu', () => {
  it('lists the bookmarks on its top level, the areas as submenus', () => {
    const state = sampleState();
    const built = buildMenu(MAIN_MENU, context({ state }));
    const top = built.rows;
    const firstSubmenu = top.findIndex((row) => row.kind === 'submenu');
    const bookmark = state.bookmarks.find((entry) => entry.url !== undefined);
    if (bookmark !== undefined) {
      const at = top.findIndex((row) => row.kind === 'item' && row.label === bookmark.label);
      expect(at).toBeGreaterThanOrEqual(0);
      expect(at).toBeLessThan(firstSubmenu);
    }
    const submenus = top.flatMap((row) => (row.kind === 'submenu' ? [row.label] : []));
    expect(submenus).toEqual([
      'shell.menu.view',
      'shell.menu.devices',
      'shell.menu.layout',
      'shell.menu.sync',
      'shell.menu.emulation',
      'shell.menu.environments',
      'shell.menu.capture',
      'shell.menu.checks',
      'shell.menu.overlays',
      'shell.menu.tools',
      'shell.menu.flows',
      'shell.menu.panels',
      'shell.menu.plugins',
      'shell.menu.app',
    ]);
  });

  it('checks the layout mode in effect as a radio choice and the sync switch', () => {
    const state = sampleState();
    state.layout.mode = 'canvas';
    state.sync.enabled = true;
    const rows = items(buildMenu(MAIN_MENU, context({ state })).rows);
    const row = (label: string) => rows.find((candidate) => candidate.label === label);
    expect(row('commands.layout.mode.canvas')).toMatchObject({ radio: true, checked: true });
    expect(row('title.layout.mode.grid')).toMatchObject({ radio: true, checked: false });
    expect(row('title.sync.toggle')).toMatchObject({ checked: true });
  });

  // RB-090-73: the Lighthouse bar is switched in View beside the other bars, checked while
  // `ui.viewFooter.lighthouse` shows it (on by default).
  it('checks the Lighthouse bar switch by its own setting', () => {
    const commands = [info('ui.lighthouse_bar.toggle', 'overlays', ['Ctrl+Shift+Space O Q'])];
    const row = (settings: MenuContext['settings']) =>
      items(buildMenu(MAIN_MENU, context({ commands, settings })).rows).find(
        (candidate) => candidate.label === 'shell.toolbar.lighthouseBar',
      );
    expect(row({})).toMatchObject({ checked: true, accel: 'Ctrl+Shift+Space O Q' });
    expect(row({ 'ui.viewFooter.lighthouse': false })).toMatchObject({ checked: false });
    expect(row({ 'ui.viewFooter.vitals': false })).toMatchObject({ checked: true });
  });

  it('stays within the bounds the core takes', () => {
    const rows = buildMenu(MAIN_MENU, context()).rows;
    const count = (list: NativeMenuRow[]): number =>
      list.reduce((sum, row) => sum + 1 + (row.kind === 'submenu' ? count(row.rows) : 0), 0);
    expect(count(rows)).toBeLessThan(800);
    const depth = (list: NativeMenuRow[]): number =>
      1 + Math.max(0, ...list.map((row) => (row.kind === 'submenu' ? depth(row.rows) : 0)));
    expect(depth(rows)).toBeLessThanOrEqual(4);
  });

  // RB-090-57: an environment or preset with a name past the core's row id limit is a row like
  // any other - its id is its place, the name only in its label and the message it sends.
  it('names environment and preset rows by their place, not by a long name', () => {
    const long = 'n'.repeat(300);
    const state = sampleState();
    state.environments = [...state.environments, { ...state.environments[0]!, id: long }];
    state.layout = { ...state.layout, presets: [...state.layout.presets, long] };
    const built = buildMenu(MAIN_MENU, context({ state }));
    const rows = items(built.rows).filter((row) => row.label === long);
    expect(rows).toHaveLength(2);
    for (const row of rows) {
      expect(row.id).toMatch(/^m\d+$/);
    }
    expect(rows.map((row) => built.actions.get(row.id))).toEqual(
      expect.arrayContaining([
        { type: 'runCommand', id: 'layout.select_preset', args: { id: long } },
        { type: 'runCommand', id: 'environments.select', args: { id: long } },
      ]),
    );
  });
});
