// Sessions in the shell (RB-020-07): the list the core sends, the badge of a view's session, and
// the commands the picker and the confirmation run. `default` is shared by every view without a
// session of its own and shows no badge.

import type { SessionInfo, ShellToCore, StateSnapshot, ViewInfo } from '../ipc/protocol';

export const ASSIGN_COMMAND = 'session.assign';
export const CLEAR_COMMAND = 'session.clear';
export const DEFAULT_SESSION = 'default';

/** What a cell label shows of a named session; `color` is null when the core sent none. */
export interface SessionBadge {
  name: string;
  color: string | null;
}

/** The project's sessions, `default` first and always there. */
export function sessionsOf(state: StateSnapshot | null): SessionInfo[] {
  const listed = state?.sessions ?? [];
  const named = listed.filter((session) => session.id !== DEFAULT_SESSION);
  return [{ id: DEFAULT_SESSION }, ...named];
}

/** The badge of `view`'s session; `null` for `default`. */
export function badgeOf(view: ViewInfo, sessions: SessionInfo[]): SessionBadge | null {
  if (view.session === DEFAULT_SESSION) {
    return null;
  }
  const color = sessions.find((session) => session.id === view.session)?.color ?? null;
  return { name: view.session, color };
}

/** `session.assign` for one view. */
export function assignMessage(view: number, session: string): ShellToCore {
  return { type: 'runCommand', id: ASSIGN_COMMAND, args: { target: view, session } };
}

/** `session.clear` once the user confirmed. */
export function clearMessage(session: string): ShellToCore {
  return { type: 'runCommand', id: CLEAR_COMMAND, args: { session } };
}
