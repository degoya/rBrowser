// Links out of rBrowser's own surfaces (RB-080-92): a finding's help page, a plugin panel's
// report or help link. The shell's pages allow no popup, so a link to a new window does
// nothing there; every such link goes through the core's `app.open_link` instead, which opens
// it in the system's browser or the active view (`ui.openLinksIn`, `ask` asks here first).
// Only a web address is sent; the core checks it again.

import type { JsonValue, ShellToCore } from '../ipc/protocol';

/** The command that opens a link. */
export const OPEN_LINK_COMMAND = 'app.open_link';
/** The setting that says where. */
export const OPEN_LINKS_IN_KEY = 'ui.openLinksIn';
/** The longest address that opens (the core's `MAX_URL_CHARS`). */
export const MAX_URL_CHARS = 2048;

/** Where a link opens. */
export type LinkTarget = 'system' | 'activeView';
/** What `ui.openLinksIn` holds. */
export type OpenLinksIn = LinkTarget | 'ask';

/** The setting's value in `settings` (the shell's user settings by key); `system` otherwise. */
export function openLinksIn(settings: Readonly<Record<string, JsonValue>>): OpenLinksIn {
  const value = settings[OPEN_LINKS_IN_KEY];
  return value === 'activeView' || value === 'ask' ? value : 'system';
}

/** Whether `url` is an address worth sending: http(s) with a host, no credentials, no white
 * space or control characters, not too long. */
export function isOpenableUrl(url: unknown): url is string {
  if (typeof url !== 'string' || url.length === 0 || url.length > MAX_URL_CHARS || /[\s\p{Cc}]/u.test(url)) {
    return false;
  }
  try {
    const parsed = new URL(url);
    return (
      (parsed.protocol === 'https:' || parsed.protocol === 'http:') &&
      parsed.hostname !== '' &&
      parsed.username === '' &&
      parsed.password === ''
    );
  } catch {
    return false;
  }
}

/** The host `url` names, for the chooser's question; the text itself when it is none. */
export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/** The request that opens `url` at `where` (the setting's place when absent). */
export function openLinkRequest(url: string, where?: LinkTarget): ShellToCore {
  return {
    type: 'runCommand',
    id: OPEN_LINK_COMMAND,
    args: where === undefined ? { url } : { url, where },
  };
}
