// The devices the menus offer (RB-094-20): rb-ipc `DevicesMenu`, re-exported by `protocol.ts`.
// The core takes the Android list again in the background when the main menu or the toolbar's
// "+" opens, so a menu is built from what adb listed last and never waits on it.

/** rb-ipc `AndroidDeviceState`: whether a view can be opened on the device. */
export type AndroidDeviceState = 'ready' | 'unauthorized' | 'offline' | 'noPermissions' | 'other';

/** rb-ipc `AndroidDeviceRow`. */
export interface AndroidDeviceRow {
  /** What `view.add_android { serial }` takes. */
  serial: string;
  /** The model, else the serial. */
  name: string;
  state: AndroidDeviceState;
}

/** rb-ipc `DevicesMenu`. */
export interface DevicesMenu {
  /** The devices of the last `adb devices -l`; empty while Android views are off. */
  android: AndroidDeviceRow[];
  /** Why the list is empty or could not be read (no adb, adb failed). */
  androidReason?: string;
  /** Safari and iOS views run in this build (macOS only). */
  apple: boolean;
}
