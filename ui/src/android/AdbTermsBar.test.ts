import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import AdbTermsBar from './AdbTermsBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const TERMS = 'https://developer.android.com/tools/releases/platform-tools';
const QUESTION: CoreToShell = {
  type: 'uiCommand',
  id: 'tools.adb.install',
  args: { version: '37.0.1', size: '8.0', license: 'LicenseRef-Android-SDK-License', from: 'dl.google.com', terms: TERMS },
};

async function mounted() {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(AdbTermsBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  const push2 = async (message: CoreToShell) => {
    push(message);
    await settle();
  };
  return { ...result, push: push2, open: () => push2(QUESTION) };
}

describe('the terms bar of the adb download (RB-099-13)', () => {
  it('stays hidden until the core asks, then names the download and links the terms', async () => {
    const { wrapper, push, open } = await mounted();
    await push({ type: 'uiCommand', id: 'layout.save_preset' });
    await push({ type: 'uiCommand', id: 'tools.adb.install' });
    expect(wrapper.find('[data-testid="adb-terms"]').exists()).toBe(false);
    await open();
    const text = wrapper.get('[data-testid="adb-terms-text"]').text();
    for (const part of ['37.0.1', '8.0 MB', 'dl.google.com', "Google's terms", 'LicenseRef-Android-SDK-License']) {
      expect(text).toContain(part);
    }
    const link = wrapper.get('[data-testid="adb-terms-link"]');
    expect(link.attributes('href')).toBe(TERMS);
    expect(link.text()).toBe('Read the terms');
    expect(wrapper.get('[data-testid="adb-terms"]').attributes('role')).toBe('alertdialog');
  });

  it('downloads only after the acceptance: Accept sends the command with it and closes', async () => {
    const { wrapper, mock, open } = await mounted();
    await open();
    expect(mock.sent.filter((message) => message.type === 'runCommand')).toEqual([]);
    await wrapper.get('[data-testid="adb-terms-accept"]').trigger('click');
    await settle();
    expect(mock.sent.filter((message) => message.type === 'runCommand')).toEqual([
      { type: 'runCommand', id: 'tools.adb.install', args: { accepted: true } },
    ]);
    expect(wrapper.find('[data-testid="adb-terms"]').exists()).toBe(false);
  });

  it('sends nothing on Cancel or Escape', async () => {
    const { wrapper, mock, open } = await mounted();
    const before = mock.sent.length;
    await open();
    await wrapper.get('[data-testid="adb-terms-cancel"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="adb-terms"]').exists()).toBe(false);
    await open();
    await wrapper.get('[data-testid="adb-terms"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="adb-terms"]').exists()).toBe(false);
    expect(mock.sent.slice(before)).toEqual([]);
  });

  it('is drawn with Nuxt UI buttons', async () => {
    const { wrapper, open } = await mounted();
    await open();
    for (const id of ['adb-terms-accept', 'adb-terms-cancel']) {
      expect(wrapper.get(`[data-testid="${id}"]`).attributes('data-slot')).toBe('base');
    }
  });
});
