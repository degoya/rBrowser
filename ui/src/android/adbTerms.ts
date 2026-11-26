// Google's terms before the adb download (RB-099-13, the owner's decision of 2026-11-25):
// `tools.adb.install` without the acceptance downloads nothing - the core sends the question as
// `uiCommand` with what the download would be (version, size in MB, source host, licence) and
// Google's terms page, `AdbTermsBar.vue` shows it, and only its "Accept and download" sends the
// command again with `{ accepted: true }`. Cancel sends nothing.

import type { JsonValue, ShellToCore } from '../ipc/protocol';
import { isOpenableUrl } from '../links/open';
import { ADB_INSTALL } from '../menu/devicesMenu';

/** What the terms bar shows: the download and the page of its terms. */
export interface AdbTerms {
  version: string;
  size: string;
  license: string;
  from: string;
  terms: string;
}

const FIELDS = ['version', 'size', 'license', 'from', 'terms'] as const;

/** The question in the arguments of the core's `uiCommand`; `null` when they are not one. */
export function adbTermsOf(args: JsonValue | undefined): AdbTerms | null {
  if (args === undefined || args === null || typeof args !== 'object' || Array.isArray(args)) {
    return null;
  }
  const terms: Partial<AdbTerms> = {};
  for (const field of FIELDS) {
    const value = args[field];
    if (typeof value !== 'string' || value.length === 0) {
      return null;
    }
    terms[field] = value;
  }
  return isOpenableUrl(terms.terms) ? (terms as AdbTerms) : null;
}

/** The person accepted the terms: the command the core downloads on. */
export function acceptance(): ShellToCore {
  return { type: 'runCommand', id: ADB_INSTALL, args: { accepted: true } };
}
