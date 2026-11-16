// A boolean setting's value as the core sent it: `true` or `false`, else the default - a missing
// key or an odd value never switches a feature (RB-080-60 review: one reader for the shell).

/** `value` when it is a boolean, `fallback` otherwise. */
export function booleanSetting(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback;
}
