// The "+" button's part of the add menu (RB-080-41, `AddViewMenu.vue`): the commands that open
// it and the new-size picker, and the hidden breakpoints the button counts. The menu's rows are
// the core's (`layout::view_menu::add_items`): it shows them as a native menu (RB-080-46).

import type { BreakpointSummary, StateSnapshot } from '../ipc/protocol';

/** The command that opens the "+" menu (`uiCommand`). */
export const OPEN_COMMAND = 'view.add';
/** The command that opens the new-size picker (`uiCommand`, the palette's `newViewProvider.ts`). */
export const NEW_SIZE_COMMAND = 'view.add_size';
/** What the picker's "Custom..." opens: the size input (`uiCommand`, `NewSizeBar.vue`). */
export const CUSTOM_SIZE_COMMAND = 'view.add_size.custom';

/** The hidden breakpoints of `state`, by width then config order. */
export function hiddenBreakpoints(state: StateSnapshot | null): BreakpointSummary[] {
  const hidden = state?.layout.hidden ?? [];
  if (!state || hidden.length === 0) {
    return [];
  }
  return state.breakpoints
    .filter((breakpoint) => hidden.includes(breakpoint.id))
    .map((breakpoint, index) => ({ breakpoint, index }))
    .sort((a, b) => a.breakpoint.width - b.breakpoint.width || a.index - b.index)
    .map((entry) => entry.breakpoint);
}
