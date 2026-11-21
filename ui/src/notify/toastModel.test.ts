// RB-094-20: the error of an action the person started shows at the top too, with the message's
// buttons; anything else stays in the status bar alone.

import { describe, expect, it } from 'vitest';
import type { Notification, ShellToCore } from '../ipc/protocol';
import { ARMED_MS, errorToastOn, latest, toastOf } from './toastModel';

const ADD: ShellToCore = { type: 'runCommand', id: 'view.add_android' };
const failed = (extra: Partial<Notification> = {}): Notification => ({
  level: 'error',
  messageKey: 'engines.android.notice.add_failed',
  args: { error: 'No Android device is connected over USB.' },
  ...extra,
});

describe('the error toast', () => {
  it("shows an error that follows an action of the person's, with Retry", () => {
    const toast = toastOf(failed(), { at: 1_000, retry: ADD }, 5_000);
    expect(toast).toMatchObject({
      messageKey: 'engines.android.notice.add_failed',
      args: { error: 'No Android device is connected over USB.' },
    });
    expect(toast?.buttons).toEqual([{ kind: 'retry', labelKey: 'shell.toast.retry', message: ADD }]);
  });

  it("offers the notice's setting and, for adb, the download", () => {
    const toast = toastOf(failed({ settingsKey: 'engines.android.adbPath' }), { at: 1_000, retry: ADD }, 2_000);
    expect(toast?.buttons.map((button) => button.kind)).toEqual(['retry', 'settings', 'download']);
    expect(toast?.buttons[1]?.message).toEqual({ type: 'openSettings', key: 'engines.android.adbPath' });
    expect(toast?.buttons[2]?.message).toEqual({ type: 'runCommand', id: 'tools.adb.install' });
    const other = toastOf(failed({ settingsKey: 'engines.android.enabled' }), { at: 1_000 }, 2_000);
    expect(other?.buttons.map((button) => button.kind)).toEqual(['settings']);
  });

  it('leaves warnings, infos and errors nobody started to the status bar', () => {
    expect(toastOf(failed({ level: 'warning' }), { at: 1_000 }, 2_000)).toBeNull();
    expect(toastOf(failed({ level: 'info' }), { at: 1_000 }, 2_000)).toBeNull();
    expect(toastOf(failed(), null, 2_000), 'nothing started').toBeNull();
    expect(toastOf(failed(), { at: 1_000 }, 1_000 + ARMED_MS + 1), 'long after').toBeNull();
    expect(toastOf(failed(), { at: 1_000 }, 1_000 + ARMED_MS)).not.toBeNull();
  });

  it('takes the later of a command the shell sent and a chord of the leader key', () => {
    const sent = { at: 1_000, retry: ADD };
    const chord = { at: 3_000 };
    expect(latest(sent, chord)).toEqual(chord);
    expect(latest(sent, null)).toEqual(sent);
    expect(latest(null, null)).toBeNull();
    expect(latest({ at: 4_000, retry: ADD }, chord)).toEqual({ at: 4_000, retry: ADD });
  });

  it('is on unless the setting says off', () => {
    expect(errorToastOn({})).toBe(true);
    expect(errorToastOn({ 'ui.notifications.errorToast': true })).toBe(true);
    expect(errorToastOn({ 'ui.notifications.errorToast': false })).toBe(false);
  });
});
