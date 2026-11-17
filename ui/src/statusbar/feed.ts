// The status bar's messages in a shell page (RB-080-15): the core's log, read on start and kept
// current by its `statusMessage` events, oldest first, capped at `ui.statusBar.historySize`.
// `current` is the message the bar shows: the newest warning or error not closed yet, or the
// newest message when it is an info or success that has not faded (`ui.statusBar.infoTimeoutSec`;
// a newer message replaces it at once). `unread` counts what arrived since the history was last
// seen (`status.history`).

import { computed, ref, watch } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { CoreToShell, SettingValue, StatusMessage } from '../ipc/protocol';
import { changedValues, touches } from '../state/settingsChanged';
import { fades } from './level';

/** `ui.statusBar.infoTimeoutSec` when the settings do not say. */
export const DEFAULT_INFO_TIMEOUT_SEC = 6;
/** `ui.statusBar.historySize` when the settings do not say. */
export const DEFAULT_HISTORY_SIZE = 200;
/** The bounds the core holds `ui.statusBar.historySize` to (`rb_config::MIN_HISTORY_SIZE`). */
export const MIN_HISTORY_SIZE = 10;
export const MAX_HISTORY_SIZE = 1000;
/** The command that opens the history; its `uiCommand` marks every message read. */
export const HISTORY_COMMAND = 'status.history';

const TIMEOUT_KEY = 'ui.statusBar.infoTimeoutSec';
const SIZE_KEY = 'ui.statusBar.historySize';

export type StatusFeed = ReturnType<typeof createStatusFeed>;

function numberOf(values: SettingValue[], key: string, fallback: number): number {
  const value = values.find((entry) => entry.key === key)?.value;
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : fallback;
}

/** `ui.statusBar.historySize` within the bounds the core holds it to (10 to 1000). */
export function clampSize(size: number): number {
  return Math.min(MAX_HISTORY_SIZE, Math.max(MIN_HISTORY_SIZE, Math.round(size)));
}

export function createStatusFeed(client: IpcClient, now: () => number = () => Date.now()) {
  const items = ref<StatusMessage[]>([]);
  const dismissed = ref(new Set<number>());
  const lastSeen = ref(0);
  const infoTimeoutSec = ref(DEFAULT_INFO_TIMEOUT_SEC);
  const historySize = ref(DEFAULT_HISTORY_SIZE);
  // The time `current` is judged at: moved on by every change and by the fade timer.
  const clock = ref(now());
  const stops: (() => void)[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;

  const faded = (message: StatusMessage): boolean =>
    infoTimeoutSec.value > 0 && clock.value - message.at >= infoTimeoutSec.value * 1000;

  const current = computed<StatusMessage | null>(() => {
    const list = items.value;
    const newest = list.at(-1);
    for (let index = list.length - 1; index >= 0; index -= 1) {
      const message = list[index];
      if (message === undefined || dismissed.value.has(message.id)) {
        continue;
      }
      if (!fades(message.level)) {
        return message;
      }
      if (message === newest && !faded(message)) {
        return message;
      }
    }
    return null;
  });

  const unread = computed(() => items.value.filter((message) => message.id > lastSeen.value).length);

  const trim = (): void => {
    const extra = items.value.length - historySize.value;
    if (extra > 0) {
      items.value.splice(0, extra);
      const kept = new Set(items.value.map((message) => message.id));
      for (const id of dismissed.value) {
        if (!kept.has(id)) {
          dismissed.value.delete(id);
        }
      }
    }
  };

  function take(message: StatusMessage): void {
    if (items.value.some((known) => known.id === message.id)) {
      return;
    }
    items.value.push(message);
    items.value.sort((a, b) => a.id - b.id);
    trim();
    clock.value = now();
  }

  function dismiss(): void {
    const shown = current.value;
    if (shown !== null) {
      dismissed.value.add(shown.id);
    }
    clock.value = now();
  }

  function markRead(): void {
    lastSeen.value = items.value.at(-1)?.id ?? lastSeen.value;
  }

  /** Takes the timeout and the size from `values`; a key left out keeps what it has. */
  function takeSettings(values: SettingValue[]): void {
    infoTimeoutSec.value = numberOf(values, TIMEOUT_KEY, infoTimeoutSec.value);
    historySize.value = clampSize(numberOf(values, SIZE_KEY, historySize.value));
    trim();
    clock.value = now();
  }

  async function loadSettings(): Promise<void> {
    try {
      takeSettings((await client.expect({ type: 'getSettings' }, 'settings')).values);
    } catch {
      // The defaults stay.
    }
  }

  const onEvent = (message: CoreToShell): void => {
    if (message.type === 'statusMessage') {
      const { type: _type, ...entry } = message;
      take(entry);
    } else if (message.type === 'uiCommand' && message.id === HISTORY_COMMAND) {
      markRead();
    } else if (message.type === 'settingsChanged' && touches(message, [TIMEOUT_KEY, SIZE_KEY])) {
      // The new values come with the event (RB-090-36); without them they are asked for.
      const changed = changedValues(message);
      if (changed) {
        takeSettings([...changed.values()]);
      } else {
        void loadSettings();
      }
    }
  };

  // The fade of the message shown: the clock moves on when it is due.
  const schedule = (message: StatusMessage | null): void => {
    clearTimeout(timer);
    timer = undefined;
    if (message === null || !fades(message.level) || infoTimeoutSec.value <= 0) {
      return;
    }
    const left = message.at + infoTimeoutSec.value * 1000 - clock.value;
    timer = setTimeout(() => {
      clock.value = now();
    }, Math.max(0, left) + 1);
  };

  async function start(): Promise<void> {
    stops.push(client.on(onEvent));
    stops.push(watch(current, schedule, { flush: 'sync' }));
    try {
      const answer = await client.expect({ type: 'getStatusMessages' }, 'statusMessages');
      answer.items.forEach(take);
    } catch {
      // A core that cannot answer: the events still arrive.
    }
    await loadSettings();
    schedule(current.value);
  }

  function stop(): void {
    stops.splice(0).forEach((fn) => fn());
    clearTimeout(timer);
  }

  return { items, current, unread, infoTimeoutSec, historySize, start, stop, dismiss, markRead };
}
