// "Devices & engines" (RB-094-20): the Android devices with their state, the way to connect one
// when none is listed, the setting while Android is off, the adb download, and Safari and iOS -
// named but no choice off macOS.

import { describe, expect, it } from 'vitest';
import type { DevicesMenu, NativeMenuRow, StateSnapshot } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { CONNECT_HELP_URL, DEVICES_MENU } from './devicesMenu';
import { MAIN_MENU } from './mainMenu';
import { buildMenu, type MenuContext } from './menuModel';
import { registryInfos } from './testRegistry';

const t = (key: string, args?: Record<string, unknown>) => (args === undefined ? key : `${key} ${JSON.stringify(args)}`);

function context(devices: DevicesMenu | undefined, android: boolean, activeView: number | null = 1): MenuContext {
  const sample = sampleState();
  const state: StateSnapshot = { ...sample, activeView, ...(devices === undefined ? {} : { devices }) };
  return { t, state, settings: { 'engines.android.enabled': android }, commands: registryInfos() };
}

type Item = Extract<NativeMenuRow, { kind: 'item' }>;
const items = (rows: NativeMenuRow[]): Item[] =>
  rows.flatMap((row) => (row.kind === 'item' ? [row] : row.kind === 'submenu' ? items(row.rows) : []));

function built(ctx: MenuContext) {
  const menu = buildMenu(DEVICES_MENU, ctx);
  const row = (label: string) => items(menu.rows).find((item) => item.label.startsWith(label));
  return { menu, row, message: (item: Item | undefined) => (item === undefined ? undefined : menu.actions.get(item.id)) };
}

const PHONE: DevicesMenu = {
  android: [
    { serial: 'R5CX', name: 'SM-S938B', state: 'ready' },
    { serial: 'EMU1', name: 'Pixel 8', state: 'unauthorized' },
  ],
  apple: false,
};

describe('the devices menu', () => {
  it('lists each Android device with its state and adds the ready one by its serial', () => {
    const { menu, row, message } = built(context(PHONE, true));
    const submenu = menu.rows.find((entry) => entry.kind === 'submenu');
    expect(submenu).toMatchObject({ label: 'shell.devices.android_add' });
    const ready = row('shell.devices.android_row {"name":"SM-S938B"');
    expect(ready?.label).toContain('shell.devices.state.ready');
    expect(ready?.disabled).toBeUndefined();
    expect(message(ready)).toEqual({ type: 'runCommand', id: 'view.add_android', args: { serial: 'R5CX' } });
    const waiting = row('shell.devices.android_row {"name":"Pixel 8"');
    expect(waiting?.label).toContain('shell.devices.state.unauthorized');
    expect(waiting?.disabled, 'named, not offered').toBe(true);
  });

  it('says how to connect a device when none is listed, with the reason, and tries again', () => {
    const { menu, row, message } = built(context({ android: [], androidReason: 'No adb was found.', apple: false }, true));
    const submenu = menu.rows.find((entry) => entry.kind === 'submenu');
    expect(submenu?.kind === 'submenu' ? submenu.rows[0] : undefined).toEqual({ kind: 'heading', label: 'No adb was found.' });
    expect(message(row('shell.devices.none'))).toEqual({ type: 'runCommand', id: 'app.open_link', args: { url: CONNECT_HELP_URL } });
    expect(message(row('shell.devices.try_again'))).toEqual({ type: 'runCommand', id: 'view.add_android' });
    // Before the core listed anything: looking, not "none".
    const early = built(context(undefined, true));
    expect(early.row('shell.devices.none')).toBeUndefined();
    expect(early.row('shell.devices.listing')?.disabled).toBe(true);
    expect(early.message(early.row('shell.devices.try_again'))).toEqual({ type: 'runCommand', id: 'view.add_android' });
  });

  it('opens the Android setting while Android views are off', () => {
    const { menu, row, message } = built(context(PHONE, false));
    expect(menu.rows.some((entry) => entry.kind === 'submenu')).toBe(false);
    expect(message(row('shell.devices.android_off'))).toEqual({
      type: 'runCommand',
      id: 'settings.open',
      args: { key: 'engines.android.enabled' },
    });
  });

  it('downloads adb on a click', () => {
    const { row, message } = built(context(PHONE, true));
    expect(message(row('shell.devices.download_adb'))).toEqual({ type: 'runCommand', id: 'tools.adb.install' });
  });

  it('names Safari and iOS off macOS as macOS only, no choice; on macOS for the active view', () => {
    const off = built(context(PHONE, true));
    expect(off.row('shell.devices.safari_mac_only')?.disabled).toBe(true);
    expect(off.row('shell.devices.ios_mac_only')?.disabled).toBe(true);
    const mac = built(context({ ...PHONE, apple: true }, true));
    const safari = mac.row('shell.devices.safari');
    expect(safari?.disabled).toBeUndefined();
    expect(mac.message(safari)).toEqual({ type: 'runCommand', id: 'view.set_engine', args: { engine: 'safari' } });
    expect(mac.message(mac.row('shell.devices.ios'))).toEqual({ type: 'runCommand', id: 'ios.devices' });
    // No active view: nothing to set the engine of.
    const none = built(context({ ...PHONE, apple: true }, true, null));
    expect(none.row('shell.devices.safari')?.disabled).toBe(true);
  });

  it('is a submenu of the main menu', () => {
    const menu = buildMenu(MAIN_MENU, context(PHONE, true));
    expect(menu.rows.some((row) => row.kind === 'submenu' && row.label === 'shell.menu.devices')).toBe(true);
  });
});
