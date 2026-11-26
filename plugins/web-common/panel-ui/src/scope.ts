// The views a test run takes (RB-099-10), the model of the one pick list - the shell's
// `ScopePicker.vue` (checks, visual regression, flows) and a plugin panel's `ScopeSelect.vue`
// (Lighthouse): every view, the views the grid shows, or chosen breakpoints. The core resolves
// the scope (`rb_config::resolve_scope`); here it is only chosen, sent and remembered - per
// function, in the page's storage where it has one (no setting), else for as long as the page
// lives.

/** The scope a run is sent with: the core's `ViewScope`. */
export type ViewScope = 'all' | 'visible' | string[];

/** What the pick list shows. */
export type ScopeMode = 'all' | 'visible' | 'chosen';

/** A choice of the pick list: the mode, and the breakpoints of `chosen`. */
export interface ScopeChoice {
  mode: ScopeMode;
  breakpoints: string[];
}

/** The texts of a panel's pick list, from its own locales. */
export interface ScopeTexts {
  label: string;
  all: string;
  visible: string;
  chosen: string;
  breakpoints: string;
  pick: string;
  none: string;
}

/** The modes in the pick list's order. */
export const SCOPE_MODES: readonly ScopeMode[] = ['all', 'visible', 'chosen'];

/** The most breakpoints a choice names. */
export const MAX_CHOSEN = 32;

/** The storage key of function `name`'s last choice. */
export const SCOPE_STORAGE_PREFIX = 'rb.scope.';

/** Every view: the choice before any other. */
export function allViews(): ScopeChoice {
  return { mode: 'all', breakpoints: [] };
}

/** The scope `choice` runs with; `null` for chosen breakpoints without one (nothing to run). */
export function scopeOf(choice: ScopeChoice): ViewScope | null {
  if (choice.mode === 'chosen') {
    return choice.breakpoints.length === 0 ? null : [...choice.breakpoints];
  }
  return choice.mode;
}

/** The arguments `args` with the scope of `choice` under `key` - none for every view, so a run
 * over all views is sent as it always was. */
export function withScope<T extends Record<string, unknown>>(args: T, choice: ScopeChoice, key = 'scope'): T {
  const scope = scopeOf(choice);
  return scope === null || scope === 'all' ? args : { ...args, [key]: scope };
}

/** The choice a scope stands for (a stored one, a run's summary); every view for anything else. */
export function choiceOf(scope: unknown): ScopeChoice {
  if (scope === 'visible') {
    return { mode: 'visible', breakpoints: [] };
  }
  if (Array.isArray(scope) && scope.length > 0 && scope.every((id) => typeof id === 'string' && id !== '')) {
    return { mode: 'chosen', breakpoints: [...new Set(scope as string[])].slice(0, MAX_CHOSEN) };
  }
  return allViews();
}

/** `choice` with only the breakpoints of `known`; chosen ones of which none is left fall back to
 * every view. */
export function keepKnown(choice: ScopeChoice, known: readonly string[]): ScopeChoice {
  if (choice.mode !== 'chosen') {
    return choice;
  }
  const kept = choice.breakpoints.filter((id) => known.includes(id));
  return kept.length === 0 ? allViews() : { mode: 'chosen', breakpoints: kept };
}

type ScopeStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** Function `name`'s last choice in `storage`; every view without one or without a storage. */
export function recallScope(storage: ScopeStorage | null, name: string): ScopeChoice {
  try {
    const text = storage?.getItem(SCOPE_STORAGE_PREFIX + name) ?? null;
    return text === null ? allViews() : choiceOf(JSON.parse(text));
  } catch {
    return allViews();
  }
}

/** Keeps `choice` as function `name`'s last; a storage that refuses keeps nothing. */
export function rememberScope(storage: ScopeStorage | null, name: string, choice: ScopeChoice): void {
  const scope = scopeOf(choice);
  try {
    storage?.setItem(SCOPE_STORAGE_PREFIX + name, JSON.stringify(scope ?? 'all'));
  } catch {
    // A blocked storage: the choice holds while the page lives.
  }
}
