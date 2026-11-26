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

/** The address a Basic auth credential belongs to, as the core names it (`AuthOrigin`):
 * `scheme://host:port`, the host lower case, the scheme's port when the URL names none; `null`
 * for anything but an http(s) URL (RB-099-04). */
export function authAddressOf(url: unknown): string | null {
  if (typeof url !== 'string') {
    return null;
  }
  let parsed: URL;
  try {
    parsed = new URL(url.trim());
  } catch {
    return null;
  }
  if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
    return null;
  }
  const port = parsed.port || (parsed.protocol === 'https:' ? '443' : '80');
  return `${parsed.protocol}//${parsed.hostname}:${port}`;
}

/** Per environment id, the other environments with its address, in their order: they share one
 * stored credential and one sign-in (RB-099-04). */
export function sharingOf(list: JsonValue | undefined): Record<string, string[]> {
  if (!Array.isArray(list)) {
    return {};
  }
  const addressed = list.flatMap((entry) =>
    typeof entry === 'object' && entry !== null && !Array.isArray(entry) && typeof entry.id === 'string'
      ? [{ id: entry.id, address: authAddressOf(entry.url) }]
      : [],
  );
  return Object.fromEntries(
    addressed.map(({ id, address }) => [
      id,
      address === null
        ? []
        : addressed.filter((other) => other.id !== id && other.address === address).map((other) => other.id),
    ]),
  );
}
