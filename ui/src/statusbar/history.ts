// What the message history shows (RB-080-15): the messages newest first, of one level or all,
// and the copied text of what is shown.

import type { NotificationLevel, StatusMessage } from '../ipc/protocol';
import { messageLine } from './level';

export type LevelFilter = NotificationLevel | 'all';

/** `items` (oldest first) newest first, only those of `filter`. */
export function shown(items: readonly StatusMessage[], filter: LevelFilter): StatusMessage[] {
  return items.filter((message) => filter === 'all' || message.level === filter).reverse();
}

/** The copied text of `messages`: one line each, as shown. */
export function copiedText(
  messages: readonly StatusMessage[],
  word: (level: NotificationLevel) => string,
  text: (message: StatusMessage) => string,
): string {
  return messages.map((message) => messageLine(message.at, word(message.level), text(message))).join('\n');
}
