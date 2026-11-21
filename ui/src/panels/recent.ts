// The plugin panels shown last (RB-090-38): their frames stay in the page, hidden, so switching
// back to one does not load its document again; `panels.keepFrames` says how many besides the
// shown one.

/** `recent` with `shown` first, when a plugin panel is shown, and at most `size` others behind it,
 * newest first. */
export function remember(recent: readonly string[], shown: string | null, size: number): string[] {
  const others = recent.filter((key) => key !== shown).slice(0, Math.max(0, size));
  return shown === null ? others : [shown, ...others];
}

/** `recent` without the panels no longer in the list (a plugin disabled or removed), so they hold
 * no `panels.keepFrames` place. */
export function present(recent: readonly string[], keys: readonly string[]): string[] {
  return recent.filter((key) => keys.includes(key));
}
