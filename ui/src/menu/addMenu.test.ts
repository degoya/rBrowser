// The toolbar's "+" menu (RB-080-41, RB-094-20): the hidden breakpoints with their size, "Add
// all" when more than one, "New size...", then the devices and engines of the main menu.

import { describe, expect, it } from 'vitest';
import type { NativeMenuRow, StateSnapshot } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { ADD_MENU } from './addMenu';
import { buildMenu, type MenuContext } from './menuModel';
import { registryInfos } from './testRegistry';

const t = (key: string, args?: Record<string, unknown>) => (args === undefined ? key : `${key} ${JSON.stringify(args)}`);

function context(hidden: string[]): MenuContext {
  const sample = sampleState();
  const state: StateSnapshot = { ...sample, layout: { ...sample.layout, hidden } };
  return { t, state, settings: { 'engines.android.enabled': true }, commands: registryInfos() };
}

type Item = Extract<NativeMenuRow, { kind: 'item' }>;
const items = (rows: NativeMenuRow[]): Item[] =>
  rows.flatMap((row) => (row.kind === 'item' ? [row] : row.kind === 'submenu' ? items(row.rows) : []));

describe('the "+" menu', () => {
  it('adds each hidden breakpoint by its id, all of them, or a new size', () => {
    const sample = sampleState();
    const hidden = sample.breakpoints.slice(0, 2).map((breakpoint) => breakpoint.id);
    expect(hidden.length).toBe(2);
    const menu = buildMenu(ADD_MENU, context(hidden));
    const sent = items(menu.rows).map((row) => menu.actions.get(row.id));
    for (const id of hidden) {
      expect(sent).toContainEqual({ type: 'runCommand', id: 'view.add', args: { breakpoint: id } });
    }
    expect(sent).toContainEqual({ type: 'runCommand', id: 'view.add', args: { breakpoint: 'all' } });
    expect(sent).toContainEqual({ type: 'runCommand', id: 'view.add_size' });
    const first = items(menu.rows)[0];
    expect(first?.label).toContain('sizes.pick.breakpoint');
  });

  it('says every breakpoint is open when none is hidden', () => {
    const menu = buildMenu(ADD_MENU, context([]));
    const first = items(menu.rows)[0];
    expect(first).toMatchObject({ label: 'views.menu.all_open', disabled: true });
  });

  it('ends in the devices and engines', () => {
    const menu = buildMenu(ADD_MENU, context([]));
    expect(menu.rows).toContainEqual({ kind: 'heading', label: 'shell.menu.devices' });
    const sent = items(menu.rows).map((row) => menu.actions.get(row.id));
    expect(sent).toContainEqual({ type: 'runCommand', id: 'tools.adb.install' });
    expect(menu.rows.some((row) => row.kind === 'submenu' && row.label === 'shell.devices.android_add')).toBe(true);
  });
});
