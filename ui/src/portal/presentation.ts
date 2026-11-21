// The presentation over the internet in the Portal panel and the activity bar (RB-096-05): the
// commands its buttons run - the consent and the download of cloudflared only as the person's
// own arguments to `portal.present.start` - whether it runs, and the link's countdown.

import type { PortalPresentation, ShellToCore } from '../ipc/protocol';
import { secondsLeft } from './portal';

export const START_COMMAND = 'portal.present.start';
export const NEW_LINK_COMMAND = 'portal.present.new_link';
export const STOP_COMMAND = 'portal.present.stop';

/** Whether a presentation runs: its tunnel opens or viewers can join. */
export function isPresenting(presentation: PortalPresentation | undefined): boolean {
  return presentation?.phase === 'starting' || presentation?.phase === 'live';
}

/** `portal.present.start`, with the consent and the download the person gave. */
export function startMessage(given: { consent: boolean; download: boolean }): ShellToCore {
  const args: Record<string, boolean> = {};
  if (given.consent) args.consent = true;
  if (given.download) args.download = true;
  return { type: 'runCommand', id: START_COMMAND, args };
}

/** `portal.present.new_link`: a fresh one-time link, the one before void. */
export function newLinkMessage(): ShellToCore {
  return { type: 'runCommand', id: NEW_LINK_COMMAND };
}

/** `portal.present.stop`: every viewer disconnected, the tunnel closed. */
export function stopMessage(): ShellToCore {
  return { type: 'runCommand', id: STOP_COMMAND };
}

/** Whole seconds the open link has left; 0 without one. */
export function linkSecondsLeft(presentation: PortalPresentation | undefined, nowMs: number): number {
  const link = presentation?.link;
  return link === undefined ? 0 : secondsLeft(link.expiresAtMs, nowMs);
}
