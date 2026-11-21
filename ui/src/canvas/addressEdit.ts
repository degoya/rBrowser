// The address field of a decoupled view's label on the canvas board (RB-092-03): the core's
// `uiCommand` `view.edit_address` names the view and its address and opens the field
// (`BoardAddressField.vue`); Enter sends the typed text back as the command's `url`. The core
// checks it as the address bar's input, loads it in that view only and names a refusal's reason
// in the status bar.
import type { JsonValue, ShellToCore, ViewId } from '../ipc/protocol';

/** The command that opens the field and, with `url`, loads the typed address. */
export const EDIT_ADDRESS = 'view.edit_address';

/** The open field: its view and the address it starts with. */
export interface AddressEdit {
  view: ViewId;
  url: string;
}

/** The field the core's `uiCommand` arguments open; `null` when they name no view. */
export function addressEditOf(args: JsonValue | undefined): AddressEdit | null {
  if (args === undefined || args === null || typeof args !== 'object' || Array.isArray(args)) {
    return null;
  }
  const { view, url } = args;
  if (typeof view !== 'number' || !Number.isInteger(view) || view < 0) {
    return null;
  }
  return { view, url: typeof url === 'string' ? url : '' };
}

/** Enter: the typed text for `view`, unchanged. */
export function submitMessage(view: ViewId, text: string): ShellToCore {
  return { type: 'runCommand', id: EDIT_ADDRESS, args: { view, url: text } };
}
