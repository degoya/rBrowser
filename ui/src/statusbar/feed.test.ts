import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IpcClient } from '../ipc/client';
import { createMockTransport, type MockTransport } from '../ipc/mockTransport';
import type { NotificationLevel, StatusMessage } from '../ipc/protocol';
import { clampSize, createStatusFeed, DEFAULT_INFO_TIMEOUT_SEC } from './feed';

const START = new Date(2026, 10, 12, 9, 0, 0).getTime();

function message(id: number, level: NotificationLevel, at = Date.now()): StatusMessage {
  return { id, at, level, messageKey: `test.message.${id}` };
}

async function setup(history: StatusMessage[] = [], settings: Record<string, unknown> = {}) {
  const mock: MockTransport = createMockTransport({ statusMessages: history, settings });
  const feed = createStatusFeed(new IpcClient(mock));
  await feed.start();
  return { mock, feed };
}

function emit(mock: MockTransport, m: StatusMessage): void {
  mock.emit({ type: 'statusMessage', ...m });
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(START);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('the status feed', () => {
  it('reads the history on start and shows the newest warning or error', async () => {
    const { feed } = await setup([message(1, 'error', START - 60_000), message(2, 'info', START - 60_000)]);
    expect(feed.items.value.map((m) => m.id)).toEqual([1, 2]);
    // The old info faded long ago; the error stays until it is closed.
    expect(feed.current.value?.id).toBe(1);
    expect(feed.unread.value).toBe(2);
  });

  it('keeps three messages in order and counts them unread until the history is seen', async () => {
    const { feed, mock } = await setup();
    emit(mock, message(1, 'info'));
    emit(mock, message(2, 'warning'));
    emit(mock, message(3, 'error'));
    expect(feed.items.value.map((m) => m.id)).toEqual([1, 2, 3]);
    expect(feed.unread.value).toBe(3);
    mock.emit({ type: 'uiCommand', id: 'status.history' });
    expect(feed.unread.value).toBe(0);
    emit(mock, message(4, 'info'));
    expect(feed.unread.value).toBe(1);
  });

  it('takes a message once, whether from the history or as an event', async () => {
    const { feed, mock } = await setup([message(1, 'info')]);
    emit(mock, message(1, 'info'));
    emit(mock, message(3, 'info'));
    emit(mock, message(2, 'info'));
    expect(feed.items.value.map((m) => m.id)).toEqual([1, 2, 3]);
  });

  it('lets an info fade after the timeout and keeps an error until it is closed', async () => {
    const { feed, mock } = await setup();
    emit(mock, message(1, 'error'));
    emit(mock, message(2, 'success'));
    expect(feed.current.value?.id).toBe(2);
    vi.advanceTimersByTime(DEFAULT_INFO_TIMEOUT_SEC * 1000 + 10);
    expect(feed.current.value?.id).toBe(1);
    vi.advanceTimersByTime(3_600_000);
    expect(feed.current.value?.id).toBe(1);
    feed.dismiss();
    expect(feed.current.value).toBeNull();
  });

  it('shows an info only while it is the newest message', async () => {
    const { feed, mock } = await setup();
    emit(mock, message(1, 'info'));
    emit(mock, message(2, 'error'));
    feed.dismiss();
    expect(feed.current.value).toBeNull();
  });

  it('follows the timeout and the history size of the settings', async () => {
    const { feed, mock } = await setup([], {
      'ui.statusBar.infoTimeoutSec': 0,
      'ui.statusBar.historySize': 10,
    });
    for (let id = 1; id <= 12; id += 1) {
      emit(mock, message(id, 'info'));
    }
    expect(feed.items.value.map((m) => m.id)).toEqual([3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    // 0 keeps an info until the next message.
    vi.advanceTimersByTime(3_600_000);
    expect(feed.current.value?.id).toBe(12);
  });

  it('takes a changed size from settingsChanged and asks only when it names its keys (RB-090-36)', async () => {
    const { feed, mock } = await setup();
    const asked = mock.sent.length;
    mock.emit({
      type: 'settingsChanged',
      keys: ['ui.statusBar.historySize'],
      values: [{ key: 'ui.statusBar.historySize', value: 10, origin: 'user' }],
    });
    for (let id = 1; id <= 12; id += 1) {
      emit(mock, message(id, 'info'));
    }
    expect(feed.items.value).toHaveLength(10);
    mock.emit({ type: 'settingsChanged', keys: ['sync.scroll'] });
    expect(mock.sent.slice(asked)).toEqual([]);
  });

  it('stops listening when stopped', async () => {
    const { feed, mock } = await setup();
    feed.stop();
    emit(mock, message(1, 'error'));
    expect(feed.items.value).toEqual([]);
  });
});

// Charge 10 review S3: a hand-edited size is held to the core's bounds.
describe('clampSize', () => {
  it('holds the history size to 10 to 1000', () => {
    expect(clampSize(1)).toBe(10);
    expect(clampSize(10_000_000)).toBe(1000);
    expect(clampSize(200)).toBe(200);
  });
});
