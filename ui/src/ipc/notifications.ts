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

/**
 * A status message's button (rb-ipc `StatusAction`, RB-090-71): a click runs `command` with
 * `args` through the registry (`runCommand`), as the toolbar and the palette do. The arguments
 * are flat here: a recursive `JsonValue` inside the status feed's refs is too deep for Vue's
 * type unwrapping (TS2589).
 */
export interface StatusAction {
  command: string;
  labelKey: string;
  labelArgs?: Record<string, string>;
  args?: Record<string, string | number | boolean | null>;
}

/**
 * A status message's copy button (rb-ipc `StatusCopy`, RB-093-04): the status bar asks the core
 * for the masked `report.md` of `report` (`bugReport`, `getMarkdown`) and puts it on the
 * clipboard itself - the core has none.
 */
export interface StatusCopy {
  labelKey: string;
  /** The saved report's folder name. */
  report: string;
}

/** A notification as the core's message log keeps it: numbered from 1, stamped. */
export interface StatusMessage extends Notification {
  id: number;
  /** Milliseconds since the Unix epoch. */
  at: number;
  /** A button that runs a command (RB-090-71). */
  action?: StatusAction;
  /** A button that copies a saved bug report's Markdown (RB-093-04). */
  copy?: StatusCopy;
}
