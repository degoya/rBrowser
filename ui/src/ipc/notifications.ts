// The messages for the person (rb-ipc `Notification`, `StatusMessage`): a notification, and the
// same numbered and stamped as the core's message log keeps it for the status bar (RB-080-15).
// Kept beside `protocol.ts`, which re-exports them.

export type NotificationLevel = 'info' | 'warning' | 'error' | 'success';

export interface Notification {
  level: NotificationLevel;
  messageKey: string;
  args?: Record<string, string>;
  settingsKey?: string;
}

/** A notification as the core's message log keeps it: numbered from 1, stamped. */
export interface StatusMessage extends Notification {
  id: number;
  /** Milliseconds since the Unix epoch. */
  at: number;
}
