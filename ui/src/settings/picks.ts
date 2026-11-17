// The logic of a pick list (RB-080-72): which values the core offers, by name; what a typed
// text means (the value itself, or a name typed in full); which chosen values are no longer
// offered. Pure, so the controls only render.

import type { DynamicData, DynamicOption } from './schema';

export interface Picks {
  options: DynamicOption[];
  /** Whether a value not offered may be added (`open`, or nothing offered to choose from). */
  open: boolean;
  /** The core still enumerates the values (the check rules). */
  loading: boolean;
}

/** The choices of `dynamic`; without data from the core any value may be typed. */
export function picksOf(dynamic: DynamicData | undefined): Picks {
  const options = dynamic?.options ?? [];
  return { options, open: dynamic === undefined || dynamic.open === true, loading: dynamic?.loading === true };
}

/** The name of `value` among the offered values; the value itself when it is not offered. */
export function nameOf(picks: Picks, value: string): string {
  return picks.options.find((option) => option.value === value)?.label ?? value;
}

/** Whether `value` is chosen but no longer offered (a removed plugin). Nothing is missing
 * while the core offers no list. */
export function isMissing(picks: Picks, value: string, known: boolean): boolean {
  return known && !picks.options.some((option) => option.value === value);
}

/** What `text` adds: an offered value, an offered name typed in full (any case), or - for an
 * open list - the text itself; `null` for an empty text or one a closed list does not offer. */
export function resolvePick(picks: Picks, text: string): string | null {
  const typed = text.trim();
  if (typed === '') {
    return null;
  }
  const exact = picks.options.find((option) => option.value === typed);
  if (exact !== undefined) {
    return exact.value;
  }
  const lower = typed.toLowerCase();
  const named = picks.options.find((option) => option.label.toLowerCase() === lower);
  if (named !== undefined) {
    return named.value;
  }
  return picks.open ? typed : null;
}
