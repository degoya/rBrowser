// The toolbar's "+" menu (RB-080-41, built in the shell since RB-094-20): a row per hidden
// breakpoint with its size, "Add all" when more than one, "New size...", then "Devices &
// engines" (`devicesMenu.ts`) - the same rows the core's add menu of an empty cell and the free
// canvas starts with (`layout::view_menu::add_items`), and the devices and engines the main
// menu lists. The core draws it as a native menu below the button (`nativeMenu.ts`, kind `add`)
// and takes the device list again when it opens.

import { hiddenBreakpoints, NEW_SIZE_COMMAND, OPEN_COMMAND } from '../toolbar/addView';
import { DEVICES_MENU } from './devicesMenu';
import { commandMessage, namedCommands, type MenuContext, type MenuNode } from './menuModel';

/** `view.add`'s breakpoint that adds every hidden one. */
export const ADD_ALL = 'all';

/** A row per hidden breakpoint by width, "Add all" when more than one; a note when none. */
export function breakpointRows(ctx: MenuContext): MenuNode[] {
  const hidden = hiddenBreakpoints(ctx.state);
  if (hidden.length === 0) {
    return [
      { kind: 'row', label: ctx.t('views.menu.all_open'), message: commandMessage(NEW_SIZE_COMMAND), disabled: true },
    ];
  }
  const rows: MenuNode[] = hidden.map((breakpoint) => ({
    kind: 'row',
    label: ctx.t('sizes.pick.breakpoint', { label: breakpoint.label, width: breakpoint.width, height: breakpoint.height }),
    message: commandMessage(OPEN_COMMAND, { breakpoint: breakpoint.id }),
  }));
  if (hidden.length > 1) {
    rows.push({ kind: 'row', label: ctx.t('views.menu.add_all'), message: commandMessage(OPEN_COMMAND, { breakpoint: ADD_ALL }) });
  }
  return rows;
}

/** The "+" menu. */
export const ADD_MENU: readonly MenuNode[] = [
  { kind: 'list', covers: [OPEN_COMMAND], build: breakpointRows },
  { kind: 'command', id: NEW_SIZE_COMMAND, labelKey: 'views.menu.new_size' },
  { kind: 'separator' },
  // The heading with the rows under it: a list's rows are tidied alone, a lone heading dropped.
  {
    kind: 'list',
    covers: [...namedCommands(DEVICES_MENU)],
    build: (ctx) => [{ kind: 'heading', label: ctx.t('shell.menu.devices') }, ...DEVICES_MENU],
  },
];
