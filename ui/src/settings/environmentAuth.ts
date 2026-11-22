// Basic auth per environment in Settings > Project > Environments (RB-094-24): which
// environments the section lists and how a switch changes `basicAuth.environments`. The user
// and password go through the shell bar's own messages (`../basic-auth/basicAuth.ts`), so both
// store them in the same place - the platform's credential store, never a project file.

import type { JsonValue } from '../ipc/protocol';

/** The key that lists the environments behind Basic auth. */
export { SETTINGS_KEY as BASIC_AUTH_KEY } from '../basic-auth/basicAuth';
/** The project's environments. */
export const ENVIRONMENTS_KEY = 'environments';

/** The ids of the environments in the value of `environments`, in order. */
export function environmentIdsOf(list: JsonValue | undefined): string[] {
  if (!Array.isArray(list)) {
    return [];
  }
  return list.flatMap((entry) =>
    typeof entry === 'object' && entry !== null && !Array.isArray(entry) && typeof entry.id === 'string'
      ? [entry.id]
      : [],
  );
}

/** `basicAuth.environments` with `id` switched on or off; the others stay as they were. */
export function withListed(listed: JsonValue | undefined, id: string, on: boolean): string[] {
  const ids = Array.isArray(listed) ? listed.filter((entry): entry is string => typeof entry === 'string') : [];
  if (on) {
    return ids.includes(id) ? ids : [...ids, id];
  }
  return ids.filter((entry) => entry !== id);
}
