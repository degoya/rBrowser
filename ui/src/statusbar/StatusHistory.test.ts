import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ui from '@nuxt/ui/vue-plugin';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import { createMockTransport, type MockTransport } from '../ipc/mockTransport';
import type { StatusMessage } from '../ipc/protocol';
import StatusHistory from './StatusHistory.vue';

let current: VueWrapper | null = null;

async function flush(): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

const AT = Date.now();
const THREE: StatusMessage[] = [
  { id: 1, at: AT, level: 'info', messageKey: 'diagnostics.created', args: { path: 'C:/d.zip' } },
  { id: 2, at: AT, level: 'warning', messageKey: 'shell.error.not_implemented' },
  { id: 3, at: AT, level: 'error', messageKey: 'cdp.notice.off', settingsKey: 'cdp.enabled' },
];

async function setup(history: StatusMessage[] = THREE) {
  const mock: MockTransport = createMockTransport({ statusMessages: history });
  const wrapper = mount(StatusHistory, {
    props: { client: new IpcClient(mock) },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  await flush();
  return { wrapper, mock };
}

afterEach(() => {
  current?.unmount();
  current = null;
  vi.unstubAllGlobals();
});

describe('the message history', () => {
  it('lists three messages newest first, whole', async () => {
    const { wrapper } = await setup();
    const items = wrapper.findAll('[data-testid="status-history-item"]');
    expect(items.map((item) => item.get('[data-testid="status-level"]').text())).toEqual([
      'Error',
      'Warning',
      'Info',
    ]);
    expect(items[2]?.text()).toContain('C:/d.zip');
    expect(items[0]?.text()).toContain('Setting');
  });

  it('names the panel, not a setting, for a notice about a key that opens a panel (RB-094-27)', async () => {
    const { wrapper } = await setup([
      { id: 1, at: AT, level: 'error', messageKey: 'shell.error.no_answer', settingsKey: 'api-client.secrets' },
    ]);
    const item = wrapper.get('[data-testid="status-history-item"]');
    expect(item.text()).toContain('API client › Variables');
    expect(item.text()).not.toContain('Setting');
    expect(item.find('button[aria-label="API client › Variables"]').exists()).toBe(true);
  });

  it('takes a new message as it comes', async () => {
    const { wrapper, mock } = await setup();
    mock.emit({ type: 'statusMessage', id: 4, at: AT, level: 'success', messageKey: 'diagnostics.created' });
    await flush();
    expect(wrapper.findAll('[data-testid="status-history-item"]')).toHaveLength(4);
  });

  it('filters by level', async () => {
    const { wrapper } = await setup();
    const errors = wrapper.get('[data-testid="status-filter-error"]');
    await errors.trigger('click');
    await flush();
    expect(errors.attributes('aria-pressed')).toBe('true');
    expect(wrapper.findAll('[data-testid="status-history-item"]')).toHaveLength(1);
    await wrapper.get('[data-testid="status-filter-success"]').trigger('click');
    await flush();
    expect(wrapper.get('[data-testid="status-history-empty"]').text()).toBe('No messages of this level.');
  });

  it('copies what it shows', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
    const { wrapper } = await setup();
    await wrapper.get('[data-testid="status-copy"]').trigger('click');
    await flush();
    const copied = String((writeText.mock.calls[0] as unknown[] | undefined)?.[0] ?? '');
    expect(copied.split('\n')).toHaveLength(3);
    expect(copied).toContain('Error\tThe CDP proxy is off.');
    expect(wrapper.text()).toContain('Copied to the clipboard.');
  });

  it('says when there is nothing yet, and hides on Escape', async () => {
    const { wrapper, mock } = await setup([]);
    expect(wrapper.get('[data-testid="status-history-empty"]').text()).toBe('No messages yet.');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await flush();
    expect(mock.sent).toContainEqual({ type: 'dismissHistory' });
  });
});
