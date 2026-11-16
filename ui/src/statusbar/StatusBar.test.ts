import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ui from '@nuxt/ui/vue-plugin';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import { createMockTransport, type MockTransport } from '../ipc/mockTransport';
import type { StatusMessage } from '../ipc/protocol';
import StatusBar from './StatusBar.vue';

let current: VueWrapper | null = null;

async function flush(): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

async function setup(history: StatusMessage[] = []) {
  const mock: MockTransport = createMockTransport({ statusMessages: history });
  const wrapper = mount(StatusBar, {
    props: { client: new IpcClient(mock) },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  await flush();
  return { wrapper, mock };
}

function emit(mock: MockTransport, message: StatusMessage): void {
  mock.emit({ type: 'statusMessage', ...message });
}

afterEach(() => {
  current?.unmount();
  current = null;
  vi.useRealTimers();
});

describe('the status bar', () => {
  it('shows an error whole, with its level as a word, at once to a screen reader, until closed', async () => {
    const { wrapper, mock } = await setup();
    emit(mock, { id: 1, at: Date.now(), level: 'error', messageKey: 'shell.error.no_answer' });
    await flush();
    const alert = wrapper.get('[data-testid="status-alert"]');
    expect(alert.attributes('role')).toBe('alert');
    expect(alert.attributes('aria-live')).toBe('assertive');
    expect(alert.get('[data-testid="status-level"]').text()).toBe('Error');
    expect(alert.get('[data-testid="status-text"]').text()).toBe('The rBrowser core did not answer.');
    expect(alert.get('[data-testid="status-line"]').attributes('title')).toBe('The rBrowser core did not answer.');
    expect(wrapper.get('[data-testid="status-polite"]').text()).toBe('');
    await wrapper.get('[data-testid="status-close"]').trigger('click');
    await flush();
    expect(wrapper.get('[data-testid="status-alert"]').text()).toBe('');
  });

  it('says an info politely and lets it fade', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'Date'] });
    const { wrapper, mock } = await setup();
    emit(mock, { id: 1, at: Date.now(), level: 'success', messageKey: 'diagnostics.created', args: { path: 'C:/d.zip' } });
    await flush();
    const polite = wrapper.get('[data-testid="status-polite"]');
    expect(polite.attributes('role')).toBe('status');
    expect(polite.attributes('aria-live')).toBe('polite');
    expect(polite.text()).toContain('Success');
    expect(polite.text()).toContain('C:/d.zip');
    await vi.advanceTimersByTimeAsync(6_100);
    await flush();
    expect(wrapper.get('[data-testid="status-polite"]').text()).toBe('');
  });

  it('counts three messages unread and opens the history, which reads them', async () => {
    const { wrapper, mock } = await setup();
    for (const [id, level] of [[1, 'info'], [2, 'warning'], [3, 'error']] as const) {
      emit(mock, { id, at: Date.now(), level, messageKey: 'shell.error.no_answer' });
    }
    await flush();
    const button = wrapper.get('[data-testid="status-history"]');
    expect(button.text()).toBe('3');
    expect(button.attributes('aria-label')).toBe('Message history, 3 unread');
    await button.trigger('click');
    await flush();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'status.history' });
    expect(wrapper.get('[data-testid="status-history"]').attributes('aria-label')).toBe('Message history');
  });

  it("opens the settings at the message's setting (RB-010-08)", async () => {
    const { wrapper, mock } = await setup([
      {
        id: 1,
        at: Date.now(),
        level: 'warning',
        messageKey: 'diagnostics.cefAge.notice',
        args: { version: '156.0.26', date: '2026-09-01', days: '70' },
        settingsKey: 'updates.cefAgeWarning',
      },
    ]);
    expect(wrapper.get('[data-testid="status-polite"]').text()).toContain('CEF 156.0.26');
    const link = wrapper.get('[data-testid="status-settings"]');
    expect(link.attributes('aria-label')).toBe('Open the setting updates.cefAgeWarning');
    await link.trigger('click');
    await flush();
    expect(mock.sent).toContainEqual({ type: 'openSettings', key: 'updates.cefAgeWarning' });
  });

  it('shows what the shell reported itself, through the core', async () => {
    const { wrapper, mock } = await setup();
    await mock.send({ type: 'reportStatus', level: 'warning', messageKey: 'shell.error.not_implemented' });
    await flush();
    expect(wrapper.get('[data-testid="status-polite"]').text()).toContain('Not available yet.');
    expect(wrapper.find('[data-testid="status-settings"]').exists()).toBe(false);
  });
});
