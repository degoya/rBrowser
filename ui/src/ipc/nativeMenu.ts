// The native menus of the toolbar's pulldowns (RB-090-41), part of the shell UI <-> core
// protocol (`protocol.ts` folds them into its unions and re-exports the types); the mirror of
// `rb_ipc::NativeMenuRow`, `ShellToCore::NativeMenu` and `CoreToShell::NativeMenuClosed`.

import type { MapRect } from './layout';

/** One row of a native menu. */
export type NativeMenuRow =
  /**
   * A row that can be chosen; `id` comes back in `nativeMenuClosed`. `checked` makes it a check
   * row, with `radio` one of a set of choices (RB-090-63); `accel` is the command's shortcut in
   * rb-commands' text form, shown beside the label.
   */
  | { kind: 'item'; id: string; label: string; checked?: boolean; radio?: boolean; accel?: string; disabled?: boolean }
  /** The title of the rows after it. */
  | { kind: 'heading'; label: string }
  | { kind: 'separator' }
  /** A row that opens the rows under it (RB-090-63). */
  | { kind: 'submenu'; label: string; rows: NativeMenuRow[] };

/** `rows` open as a native menu under `anchor`, a button's rectangle (DIP from the shell view's
 *  corner), in front of the views; answered with `ack`. */
export type NativeMenuRequest = { type: 'nativeMenu'; menu: string; anchor: MapRect; rows: NativeMenuRow[] };

/** The menu `menu` closed; `item` is the row chosen, absent without a choice. */
export type NativeMenuEvent = { type: 'nativeMenuClosed'; menu: string; item?: string };
