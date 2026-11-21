// RB-094-20: an error of an action the person started shows at the top with its buttons; one
// nobody started, a warning, or the setting off shows nothing there.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import { SHOWN_MS } from './toastModel';
import ErrorToast from './ErrorToast.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  vi.useRealTimers();
  current?.unmount();
  current = null;
  document.body.innerHTML = '';
});

const FAILED: CoreToShell = {
  type: 'notification',
  level: 'error',
  messageKey: 'engines.android.notice.add_failed',
  args: { error: 'No Android device is connected over USB with USB debugging on.' },
  settingsKey: 'engines.android.adbPath',
};

async function mounted(settings: Record<string, unknown> = {}) {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(ErrorToast, { mock: { settings }, mount: { props: { subscribe } } });
  current = result.wrapper;
  return { ...result, push: (message: CoreToShell) => push(message) };
}

const toast = () => document.body.querySelector('[data-testid="error-toast"]');

describe('the error toast', () => {
  it("shows the error of a command the person ran, with Retry, Settings and Download", async () => {
    const { shell, push, mock, wrapper } = await mounted();
    await shell.send({ type: 'runCommand', id: 'view.add_android' });
    push(FAILED);
    await settle();
    expect(toast()).not.toBeNull();
    expect(wrapper.get('[data-testid="error-toast-text"]').text()).toContain('No Android device is connected');
    expect(document.body.querySelector('[role="alert"]')).not.toBeNull();
    await wrapper.get('[data-testid="error-toast-retry"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'view.add_android' });
    expect(toast(), 'a button closes it').toBeNull();
  });

  it('opens the setting and downloads adb from its buttons', async () => {
    const { shell, push, mock, wrapper } = await mounted();
    await shell.send({ type: 'runCommand', id: 'view.add_android' });
    push(FAILED);
    await settle();
    await wrapper.get('[data-testid="error-toast-settings"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'openSettings', key: 'engines.android.adbPath' });
    push(FAILED);
    await settle();
    await wrapper.get('[data-testid="error-toast-download"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'tools.adb.install' });
  });

  it('follows a chord of the leader key, without Retry', async () => {
    const { push, wrapper } = await mounted();
    push({ type: 'whichKey', typed: 'Ctrl+Shift+Space V', entries: [] });
    push(FAILED);
    await settle();
    expect(toast()).not.toBeNull();
    expect(wrapper.find('[data-testid="error-toast-retry"]').exists()).toBe(false);
  });

  it('shows nothing for an error nobody started, a warning, or with the setting off', async () => {
    const quiet = await mounted();
    quiet.push(FAILED);
    await settle();
    expect(toast(), 'nothing started').toBeNull();
    await quiet.shell.send({ type: 'runCommand', id: 'view.add_android' });
    quiet.push({ ...FAILED, level: 'warning' });
    await settle();
    expect(toast(), 'a warning').toBeNull();
    quiet.wrapper.unmount();
    current = null;
    const off = await mounted({ 'ui.notifications.errorToast': false });
    await off.shell.send({ type: 'runCommand', id: 'view.add_android' });
    off.push(FAILED);
    await settle();
    expect(toast(), 'switched off').toBeNull();
  });

  it('goes after a while, and at once with its close button or Escape', async () => {
    const { shell, push, wrapper } = await mounted();
    await shell.send({ type: 'runCommand', id: 'view.add_android' });
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    push(FAILED);
    await nextTick();
    expect(toast()).not.toBeNull();
    vi.advanceTimersByTime(SHOWN_MS);
    await nextTick();
    expect(toast(), 'gone after SHOWN_MS').toBeNull();
    vi.useRealTimers();
    push(FAILED);
    await settle();
    await wrapper.get('[data-testid="error-toast-close"]').trigger('click');
    await settle();
    expect(toast()).toBeNull();
    push(FAILED);
    await settle();
    await wrapper.get('[data-testid="error-toast"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(toast()).toBeNull();
  });
});
