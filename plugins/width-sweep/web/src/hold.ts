// Holding one width for a person to look at it (RB-093-01, the command `show`: "Show at N px" on
// a width sweep finding). The component has the host's width lease set the width, then evaluates
// this in the plugin's isolated world; its promise settles - and the call goes on to end the
// lease - at the first of:
//
// - escape: Escape pressed in the view since the hold began (the sweep's listener, reset here);
// - resized: the page left the width it had once settled - the lease was ended from outside (the
//   cancel command), or another `show` or a sweep set another width; the component then leaves
//   the lease alone, it is no longer this call's;
// - elapsed: `ms` went by, so the view never stays at the width longer than the call may run.

import { listen, settle, state, type SweepWindow } from './detect';

export type HoldEnd = 'escape' | 'resized' | 'elapsed';

export interface HoldOptions {
  /** The width the lease set. */
  width: number;
  /** The longest hold, milliseconds. */
  ms: number;
}

export interface HoldAnswer {
  end: HoldEnd;
  /** The width the page had once settled. */
  width: number;
}

/** How often the hold looks at the page, milliseconds. */
export const HOLD_POLL_MS = 100;

/** Holds the width the lease set until Escape, another width or the time given. */
export async function hold(options: HoldOptions, doc: Document = document, win: SweepWindow = window): Promise<HoldAnswer> {
  const held = state(win);
  held.escape = false;
  listen(doc, held);
  await settle(win, options.width);
  const width = Math.round(win.innerWidth);
  for (let waited = 0; waited < options.ms; waited += HOLD_POLL_MS) {
    await new Promise<void>((resolve) => win.setTimeout(resolve, HOLD_POLL_MS));
    if (held.escape) return { end: 'escape', width };
    if (Math.round(win.innerWidth) !== width) return { end: 'resized', width };
  }
  return { end: 'elapsed', width };
}
