// The error toast (RB-094-20, `ErrorToast.vue`): an error of an action the person started - a
// menu row, a toolbar button, the "+" menu, a leader chord - shows at the top of the window as
// well, in the toolbar's strip and never over the views, with the message's buttons: Retry (the
// command the shell sent), the notice's setting, and for a missing adb the download. The status
// bar stays the log of every message. "Started" is a command the shell sent or a chord of the
// leader key within `ARMED_MS` before the error: adb may take most of a minute to give up.
// `ui.notifications.errorToast` switches it off.

import type { JsonValue, Notification, ShellToCore } from '../ipc/protocol';

/** The setting that switches the toast. */
export const ERROR_TOAST_SETTING = 'ui.notifications.errorToast';
/** How long after an action its error still counts as that action's. */
export const ARMED_MS = 120_000;
/** How long the toast stays unless the pointer or the focus is on it. */
export const SHOWN_MS = 10_000;
/** The notices about a program that rBrowser can download: the setting, the command. */
const DOWNLOADS: Readonly<Record<string, string>> = { 'engines.android.adbPath': 'tools.adb.install' };

/** When the person last started something, and the command to run again. */
export interface Started {
  at: number;
  retry?: ShellToCore;
}

/** A button of the toast. */
export interface ToastButton {
  kind: 'retry' | 'settings' | 'download';
  labelKey: string;
  message: ShellToCore;
}

/** What the toast shows. */
export interface ToastModel {
  messageKey: string;
  args: Record<string, string>;
  buttons: ToastButton[];
}

/** Whether the toast is on (`ui.notifications.errorToast`, on unless off). */
export function errorToastOn(settings: Readonly<Record<string, JsonValue>>): boolean {
  return settings[ERROR_TOAST_SETTING] !== false;
}

/** The later of two starts. */
export function latest(first: Started | null, second: Started | null): Started | null {
  if (first === null || second === null) {
    return first ?? second;
  }
  return second.at > first.at ? second : first;
}

/** The toast for `notification` at `now`, `null` unless it is an error that follows `started`. */
export function toastOf(notification: Notification, started: Started | null, now: number): ToastModel | null {
  if (notification.level !== 'error' || started === null || now < started.at || now - started.at > ARMED_MS) {
    return null;
  }
  const buttons: ToastButton[] = [];
  if (started.retry !== undefined) {
    buttons.push({ kind: 'retry', labelKey: 'shell.toast.retry', message: started.retry });
  }
  const setting = notification.settingsKey;
  if (setting !== undefined) {
    buttons.push({ kind: 'settings', labelKey: 'shell.toast.settings', message: { type: 'openSettings', key: setting } });
    const download = DOWNLOADS[setting];
    if (download !== undefined) {
      buttons.push({ kind: 'download', labelKey: 'shell.toast.download', message: { type: 'runCommand', id: download } });
    }
  }
  return { messageKey: notification.messageKey, args: notification.args ?? {}, buttons };
}
