// "Devices & engines" (RB-094-20): the main menu's submenu and the end of the toolbar's "+" menu
// (`addMenu.ts`), so no device or engine is reachable only by a shortcut. "Add Android device"
// lists the devices adb listed last (`state.devices`, taken again by the core whenever one of
// the two menus opens), each with its state - a device that is not ready is named, not offered;
// with none, "No device - how to connect" opens the handbook's steps and "Try again" lists anew
// and adds the one ready device; before the first listing it says it is looking. With `engines.android.enabled` off the row opens that setting.
// "Download adb" (`tools.adb.install`, the person's consent), then Safari and iOS for the active
// view: off macOS shown, but not a choice, with "macOS only" in the label.

import type { AndroidDeviceRow } from '../ipc/protocol';
import { commandMessage, settingOn, type MenuContext, type MenuNode } from './menuModel';

/** Adds a device as a view (`{ serial }`; without, the one ready device). */
export const ANDROID_ADD = 'view.add_android';
/** Downloads the Android Platform Tools after the person's click. */
export const ADB_INSTALL = 'tools.adb.install';
/** The iOS simulators and devices for the active view (macOS). */
export const IOS_DEVICES = 'ios.devices';
/** The engine of the active view; Safari here. */
export const SET_ENGINE = 'view.set_engine';
/** Opens a setting's field. */
export const SETTINGS_OPEN = 'settings.open';
/** Opens a link of rBrowser's own surfaces. */
export const OPEN_LINK = 'app.open_link';
/** The switch of Android views. */
export const ANDROID_SETTING = 'engines.android.enabled';
/** The handbook's steps to bring a phone to adb. */
export const CONNECT_HELP_URL = 'https://github.com/degoya/rBrowser/wiki/engines#connect-a-device';

/** The word of each device state. */
const STATE_KEYS: Readonly<Record<AndroidDeviceRow['state'], string>> = {
  ready: 'shell.devices.state.ready',
  unauthorized: 'shell.devices.state.unauthorized',
  offline: 'shell.devices.state.offline',
  noPermissions: 'shell.devices.state.noPermissions',
  other: 'shell.devices.state.other',
};

/** The text of a device's row: its name and its state. */
export function deviceLabel(ctx: MenuContext, device: AndroidDeviceRow): string {
  return ctx.t('shell.devices.android_row', {
    name: device.name,
    state: ctx.t(STATE_KEYS[device.state]),
  });
}

/** The rows of "Add Android device": each device, or why there is none and what to do. */
export function androidRows(ctx: MenuContext): MenuNode[] {
  if (ctx.state?.devices === undefined) {
    // The core has not listed yet: looking, not "none" (design.md: loading, empty and failed).
    return [
      { kind: 'row', label: ctx.t('shell.devices.listing'), message: commandMessage(ANDROID_ADD), disabled: true },
      { kind: 'row', label: ctx.t('shell.devices.try_again'), message: commandMessage(ANDROID_ADD) },
    ];
  }
  const devices = ctx.state.devices.android;
  if (devices.length === 0) {
    const reason = ctx.state?.devices?.androidReason;
    return [
      ...(reason === undefined || reason === '' ? [] : [{ kind: 'heading' as const, label: reason }]),
      {
        kind: 'row',
        label: ctx.t('shell.devices.none'),
        message: commandMessage(OPEN_LINK, { url: CONNECT_HELP_URL }),
      },
      { kind: 'row', label: ctx.t('shell.devices.try_again'), message: commandMessage(ANDROID_ADD) },
    ];
  }
  return devices.map((device) => ({
    kind: 'row',
    label: deviceLabel(ctx, device),
    message: commandMessage(ANDROID_ADD, { serial: device.serial }),
    disabled: device.state !== 'ready',
  }));
}

/** Android: the device submenu while Android views are on, else the row to their setting. */
function androidSection(ctx: MenuContext): MenuNode[] {
  if (!settingOn(ctx, ANDROID_SETTING)) {
    return [
      {
        kind: 'row',
        label: ctx.t('shell.devices.android_off'),
        message: commandMessage(SETTINGS_OPEN, { key: ANDROID_SETTING }),
      },
    ];
  }
  return [{ kind: 'submenu', labelKey: 'shell.devices.android_add', children: [{ kind: 'list', covers: [], build: androidRows }] }];
}

/** Safari and iOS for the active view: a choice on macOS with an active view, else named. */
function appleRows(ctx: MenuContext): MenuNode[] {
  const apple = ctx.state?.devices?.apple === true;
  const enabled = () => apple && (ctx.state?.activeView ?? null) !== null;
  return [
    {
      kind: 'command',
      id: SET_ENGINE,
      labelKey: apple ? 'shell.devices.safari' : 'shell.devices.safari_mac_only',
      args: { engine: 'safari' },
      enabled,
    },
    {
      kind: 'command',
      id: IOS_DEVICES,
      labelKey: apple ? 'shell.devices.ios' : 'shell.devices.ios_mac_only',
      enabled,
    },
  ];
}

/** "Devices & engines": Android, the adb download, Safari and iOS. */
export const DEVICES_MENU: readonly MenuNode[] = [
  { kind: 'list', covers: [ANDROID_ADD, SETTINGS_OPEN], build: androidSection },
  { kind: 'command', id: ADB_INSTALL, labelKey: 'shell.devices.download_adb' },
  { kind: 'separator' },
  { kind: 'list', covers: [SET_ENGINE, IOS_DEVICES], build: appleRows },
];
