// The core's side of a native menu (RB-090-41), for tests: the last menu the shell asked for, and
// the menu closing with a row chosen or none - as the core pushes it, an `rb-ipc` event on the
// window.
import { EVENT_NAME, type NativeMenuRequest, type ShellToCore } from '../ipc/protocol';
import { settle } from './mountShell';

/** The last `nativeMenu` request in `sent`. */
export function lastNativeMenu(sent: readonly ShellToCore[]): NativeMenuRequest {
  const found = sent.findLast((message): message is NativeMenuRequest => message.type === 'nativeMenu');
  if (found === undefined) {
    throw new Error('no native menu was asked for');
  }
  return found;
}

/** The last menu asked for closes, with `item` chosen or (without it) none. */
export async function closeNativeMenu(sent: readonly ShellToCore[], item?: string): Promise<void> {
  const { menu } = lastNativeMenu(sent);
  const detail = item === undefined ? { type: 'nativeMenuClosed', menu } : { type: 'nativeMenuClosed', menu, item };
  window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail }));
  await settle();
}
