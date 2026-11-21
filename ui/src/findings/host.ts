// The findings panel's channel on the shell's IPC (RB-060-01): requests go out with the shell's
// `send` (which reports a refusal in the status line), `findings` and `checksProgress` events come
// from the core's event stream, and the last run and its progress are asked for with `ask`
// (`getFindings`, `getChecksProgress`) when the window opens.

import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import type { Subscribe } from '../whichkey/whichkey';
import type { FindingsChannel, FindingsEvent, ProgressEvent } from './types';

/** The command that shows and hides the panel (`Ctrl+Shift+Space P F`). */
export const PANEL_COMMAND = 'panels.findings';

/** A channel over `send` (`false`: refused), `subscribe` (the core's events), `ask` (a
 * request with its answer; without it there is no last run) and `binding` (a command's shortcut
 * for the tooltips, RB-092-05). */
export function findingsChannel(
  send: (message: ShellToCore) => Promise<boolean>,
  subscribe: Subscribe,
  ask?: (message: ShellToCore) => Promise<CoreToShell>,
  binding?: (command: string) => string | null,
): FindingsChannel {
  return {
    binding(command) {
      return binding?.(command) ?? null;
    },
    async last() {
      if (ask === undefined) {
        return null;
      }
      const answer = await ask({ type: 'getFindings' });
      return answer.type === 'findings' ? answer : null;
    },
    async progress() {
      if (ask === undefined) {
        return null;
      }
      const answer = await ask({ type: 'getChecksProgress' });
      return answer.type === 'checksProgress' ? answer : null;
    },
    async request(message) {
      if (!(await send(message))) {
        throw new Error(`the core refused ${message.type}`);
      }
    },
    on(listener: (event: FindingsEvent | ProgressEvent) => void) {
      return subscribe((message: CoreToShell) => {
        if (message.type === 'findings' || message.type === 'checksProgress') {
          listener(message);
        }
      });
    },
  };
}
