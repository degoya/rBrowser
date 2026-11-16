// A toolbar pulldown as the core's native menu (RB-090-41): the theme, the environment, the
// layout preset, the bookmarks and the overflow. The toolbar is a strip 44 px tall, so a menu it
// drew itself would be cut off at its edge, and a `<select>`'s list was Chromium's popup - in
// rBrowser's theme only in part, with other colours under the mouse, standing out of the window.
// The button names its rectangle and the rows (`nativeMenu`); the core shows them as a native
// menu below it, in front of the views and painted in the theme, like the canvas zoom's presets
// and the view menus. The row chosen comes back once the menu closed (`nativeMenuClosed`), and
// the pulldown runs it through the one command registry; the keyboard focus goes back to the
// button, with a choice and without one (Esc). The core takes a menu only from a pulldown it
// knows (`PulldownKind`), with at most `MAX_ROWS` rows of at most `MAX_TEXT` characters
// (`layout::native_menu`, RB-090-45): a longer label is shortened here, a longer list cut.
//
// Every opening has a name of its own, `<kind>-<n>` (RB-090-54): a click on the button while its
// menu is open closes that menu - the core tells so a moment later - and opens it again, and the
// first menu's late `nativeMenuClosed` must not end the second, nor take its choice.

import { onScopeDispose } from 'vue';
import type { NativeMenuRow, ShellToCore } from '../ipc/protocol';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';

export interface NativeMenuOptions {
  /** Sends the request; `false` when the core refused it (`shell.send`). */
  send: (message: ShellToCore) => Promise<boolean>;
  /** The core's events; the window's in the app. */
  subscribe?: Subscribe;
}

/** The pulldowns the core draws a native menu for (`native_menu::PULLDOWNS`). */
export type PulldownKind = 'theme' | 'environment' | 'layout' | 'bookmarks' | 'overflow';

/** The most rows of one menu, and characters of one label, the core takes. */
export const MAX_ROWS = 200;
export const MAX_TEXT = 256;

/** `label` within `MAX_TEXT` characters, an ellipsis where it was cut. */
function clip(label: string): string {
  const chars = [...label];
  return chars.length <= MAX_TEXT ? label : `${chars.slice(0, MAX_TEXT - 1).join('')}\u2026`;
}

/** The row with its label within the core's bound. */
function bounded(row: NativeMenuRow): NativeMenuRow {
  return row.kind === 'separator' ? row : { ...row, label: clip(row.label) };
}

/**
 * `rows` within `max` rows: a longer list ends with `more(count)` in place of the rows cut,
 * `count` the choosable ones among them; no heading or separator is left dangling before it.
 */
export function capRows(
  rows: NativeMenuRow[],
  max: number,
  more: (count: number) => NativeMenuRow,
): NativeMenuRow[] {
  if (rows.length <= max) {
    return rows;
  }
  const kept = rows.slice(0, Math.max(0, max - 1));
  while (kept.length > 0 && kept.at(-1)?.kind !== 'item') {
    kept.pop();
  }
  const items = (list: NativeMenuRow[]) => list.filter((row) => row.kind === 'item').length;
  return [...kept, more(items(rows) - items(kept))];
}

/** The keys that open a pulldown besides Enter and Space, which are its button's click. */
const OPENS = new Set(['ArrowDown', 'ArrowUp', 'F4']);

/** Whether a key on a pulldown's button opens its menu, as it would open a select's list. */
export function isOpeningKey(event: KeyboardEvent): boolean {
  return !event.ctrlKey && !event.metaKey && OPENS.has(event.key);
}

/**
 * What a pulldown offers, for the overflow menu (`overflowEntries.ts`): a group that left the row
 * stays mounted, and its button's rows become a headed block there, as a select's options did.
 */
export interface MenuSource {
  /** The rows as the menu would show them now. */
  rows: () => NativeMenuRow[];
  /** Runs the row `id`, as a choice from the menu would. */
  choose: (id: string) => void;
}

const sources = new WeakMap<Element, MenuSource>();

/** Makes `button`'s rows readable for the overflow menu; returns the function that stops it. */
export function provideMenuSource(button: Element, source: MenuSource): () => void {
  sources.set(button, source);
  return () => {
    if (sources.get(button) === source) {
      sources.delete(button);
    }
  };
}

/** The rows `button` offers, when it is a native-menu pulldown. */
export function menuSourceOf(button: Element): MenuSource | undefined {
  return sources.get(button);
}

/** The menus opened so far; each opening's name is its own. */
let made = 0;

/**
 * One pulldown's menu. `open(anchor, rows, choose)` asks for `rows` below `anchor` (the button or
 * the element around it) and calls `choose` with the id of the row chosen. Listens to the core
 * until the calling scope ends; only the newest opening is heard.
 */
export function useNativeMenu(kind: PulldownKind, options: NativeMenuOptions) {
  let pending: { menu: string; anchor: HTMLElement; choose: (id: string) => void } | null = null;

  const stop = (options.subscribe ?? windowEvents)((message) => {
    if (message.type !== 'nativeMenuClosed' || pending === null || message.menu !== pending.menu) {
      return;
    }
    const { anchor, choose } = pending;
    pending = null;
    focusIn(anchor);
    if (message.item !== undefined) {
      choose(message.item);
    }
  });
  onScopeDispose(stop);

  async function open(anchor: HTMLElement, rows: NativeMenuRow[], choose: (id: string) => void): Promise<void> {
    const box = anchor.getBoundingClientRect();
    made += 1;
    const menu = `${kind}-${made}`;
    pending = { menu, anchor, choose };
    const shown = await options.send({
      type: 'nativeMenu',
      menu,
      anchor: {
        x: Math.round(box.left),
        y: Math.round(box.top),
        width: Math.round(box.width),
        height: Math.round(box.height),
      },
      rows: rows.slice(0, MAX_ROWS).map(bounded),
    });
    if (!shown && pending?.menu === menu) {
      pending = null;
    }
  }

  return { open, isOpen: () => pending !== null };
}

/** The keyboard focus back on the button in `anchor` (or `anchor` itself). */
function focusIn(anchor: HTMLElement): void {
  const button = anchor.matches('button') ? anchor : anchor.querySelector('button');
  (button ?? anchor).focus();
}
