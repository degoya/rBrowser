// The WebKit build rBrowser installs (RB-040-04, threat model T11): the revision the pinned
// playwright-core names in its browsers.json, the Windows archive of that revision and our own
// SHA-256 of it. Playwright's installer checks no digest, so rBrowser downloads, verifies and
// unpacks the archive itself (install.ts) and never runs Playwright's CLI.
//
// The source of these values is scripts/lib/tool-versions.sh (RB_WEBKIT_*);
// scripts/tool-versions.sh --check holds this copy to it, and pin.test.ts holds the revision to
// the installed playwright-core. A new playwright-core means a new archive: download it from one
// of the mirrors below (the URL is `<mirror>/builds/webkit/<revision>/webkit-win64.zip`), compare
// the SHA-256 of two mirrors, and move every value here and in tool-versions.sh.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/** A pinned archive. */
export interface Pin {
  playwright: string;
  revision: string;
  archive: string;
  sha256: string;
  bytes: number;
}

export const WEBKIT_PIN: Pin = {
  /** The playwright-core version the pin belongs to (sidecar/package.json). */
  playwright: '1.65.1',
  /** The `webkit` revision of that version's browsers.json. */
  revision: '2388',
  /** The archive for Windows x64 (Playwright's `win64` download path). */
  archive: 'webkit-win64.zip',
  /** Our SHA-256 of the archive, taken 2026-11-10 from both mirrors. */
  sha256: '0b390661995316d8f90fe0f7a0498f3a88c451c097fc8e4047342e1fbd36548b',
  /** Its size in bytes: the progress without a Content-Length, and the download's ceiling. */
  bytes: 65443702,
};

/** Playwright's download mirrors (playwright-core's `PLAYWRIGHT_CDN_MIRRORS`), tried in order. */
export const MIRRORS = [
  'https://cdn.playwright.dev/dbazure/download/playwright',
  'https://playwright.download.prss.microsoft.com/dbazure/download/playwright',
] as const;

/** The archive's URL on each mirror. */
export function archiveUrls(pin: Pin = WEBKIT_PIN): string[] {
  return MIRRORS.map((mirror) => `${mirror}/builds/webkit/${pin.revision}/${pin.archive}`);
}

/** The build's folder under the browsers path, as Playwright names it. */
export function buildDirName(revision: string = WEBKIT_PIN.revision): string {
  return `webkit-${revision}`;
}

/** The executable inside the build folder (Playwright's `EXECUTABLE_PATHS.webkit`). */
export function executableName(platform: NodeJS.Platform): string {
  return platform === 'win32' ? 'Playwright.exe' : 'pw_run.sh';
}

/** The marker Playwright writes after a complete install; its launcher looks for it too. */
export const MARKER = 'INSTALLATION_COMPLETE';

/** rBrowser's own marker, written last: the SHA-256 of the archive the build was unpacked from.
 *  Only a build whose marker names the pinned archive counts as installed - a folder Playwright
 *  or anything else left behind with `INSTALLATION_COMPLETE` does not. */
export const PIN_MARKER = 'RBROWSER_PIN';

/** Whether `dir` holds the build of `pin`, by its `PIN_MARKER`; `readText` answers null for a
 *  file it cannot read. */
export function isPinnedBuild(dir: string, readText: (path: string) => string | null, pin: Pin = WEBKIT_PIN): boolean {
  return readText(join(dir, PIN_MARKER))?.trim() === pin.sha256;
}

/** A small file's text, or null when it cannot be read. */
export function readTextOrNull(path: string): string | null {
  try {
    return readFileSync(path, 'utf8');
  } catch {
    return null;
  }
}
