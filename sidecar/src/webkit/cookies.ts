// Cookies copied from Chromium into a WebKit view (RB-040-05): the parameters of
// `webkit.view.cookies`, checked before anything reaches Playwright. The values are secrets:
// they go into the view's context and nowhere else - no error, log line or answer names one.

import type { CookieLike } from './types.ts';
import { bool, finite, invalid, object, text } from './params.ts';

/** The most cookies in one call (Chromium keeps at most 3300 per profile). */
export const MAX_COOKIES = 4000;
/** The longest name plus value (RFC 6265bis' limit). */
const MAX_PAIR = 4096;
/** The longest domain. */
const MAX_DOMAIN = 253;
/** The longest path. */
const MAX_PATH = 1024;
/** The latest expiry accepted: the year 9999, in seconds since the epoch. */
const MAX_EXPIRES = 253_402_300_799;
const SAME_SITE = ['Strict', 'Lax', 'None'] as const;

/** One cookie of `webkit.view.cookies`, as Playwright's `addCookies` takes it. */
export function cookie(value: unknown, at: number): CookieLike {
  const what = `cookies[${at}]`;
  const raw = object(value, what);
  const name = text(raw.name, `${what}.name`, MAX_PAIR);
  // The value's own text never reaches the message.
  if (typeof raw.value !== 'string' || name.length + raw.value.length > MAX_PAIR) {
    throw invalid(`${what}.value must be a string; name and value at most ${MAX_PAIR} characters`);
  }
  const domain = text(raw.domain, `${what}.domain`, MAX_DOMAIN);
  if (domain.replace(/^\./, '') === '') throw invalid(`${what}.domain must not be empty`);
  const path = text(raw.path, `${what}.path`, MAX_PATH);
  if (!path.startsWith('/')) throw invalid(`${what}.path must start with /`);
  const expires = finite(raw.expires, `${what}.expires`, -1, MAX_EXPIRES);
  const sameSite = raw.sameSite;
  if (!(SAME_SITE as readonly unknown[]).includes(sameSite)) {
    throw invalid(`${what}.sameSite must be one of ${SAME_SITE.join(', ')}`);
  }
  return {
    name,
    value: raw.value,
    domain,
    path,
    expires,
    httpOnly: bool(raw.httpOnly, `${what}.httpOnly`),
    secure: bool(raw.secure, `${what}.secure`),
    sameSite: sameSite as CookieLike['sameSite'],
  };
}

/** The `cookies` list of `webkit.view.cookies`. */
export function cookies(value: unknown): CookieLike[] {
  if (!Array.isArray(value) || value.length > MAX_COOKIES) {
    throw invalid(`cookies must be a list of at most ${MAX_COOKIES} cookies`);
  }
  return value.map((entry, at) => cookie(entry, at));
}

/** How many cookies a context took and refused. */
export interface CookieOutcome {
  added: number;
  refused: number;
}

/** Adds `list` to `context` one by one, so one cookie WebKit refuses costs only itself. */
export async function addEach(
  context: { addCookies(cookies: CookieLike[]): Promise<void> },
  list: CookieLike[],
): Promise<CookieOutcome> {
  const outcome: CookieOutcome = { added: 0, refused: 0 };
  for (const entry of list) {
    try {
      await context.addCookies([entry]);
      outcome.added += 1;
    } catch {
      // Playwright's message may quote the cookie: counted, never passed on.
      outcome.refused += 1;
    }
  }
  return outcome;
}
