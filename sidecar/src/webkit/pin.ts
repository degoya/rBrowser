// The WebKit build rBrowser installs (RB-040-04, threat model T11): the revision the pinned
// playwright-core names in its browsers.json, the Windows archive of that revision and our own
// SHA-256 of it; on macOS (RB-097-02) the archive of the host's macOS version and architecture
// (`MAC_PINS`, `hostPlatform`). Playwright's installer checks no digest, so rBrowser downloads, verifies and
// unpacks the archive itself (install.ts) and never runs Playwright's CLI.
//
// The source of these values is scripts/lib/tool-versions.sh (RB_WEBKIT_*);
// scripts/tool-versions.sh --check holds this copy to it, and pin.test.ts holds the revision to
// the installed playwright-core. A new playwright-core means a new archive: download it from one
// of the mirrors below (the URL is `<mirror>/builds/webkit/<revision>/webkit-win64.zip`), compare
// the SHA-256 of two mirrors, and move every value here and in tool-versions.sh.

import { readFileSync } from 'node:fs';
import { cpus, release } from 'node:os';
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

/** The macOS builds (RB-097-02): Playwright names the host by its macOS version and CPU
 *  (`hostPlatform`), and macOS 14 keeps an older revision (browsers.json `revisionOverrides`).
 *  Our SHA-256s, taken 2026-11-22; cdn.playwright.dev redirects to the Microsoft mirror, which
 *  served them. One line each: scripts/tool-versions.sh --check holds them to
 *  scripts/lib/tool-versions.sh (RB_WEBKIT_MAC_*). */
export const MAC_PINS = {
  mac14: { playwright: '1.65.1', revision: '2251', archive: 'webkit-mac-14.zip', sha256: '50ce5d7004274b01b106dda5ceee6703013b7bd0d0384457cd93a524ced34202', bytes: 86430592 },
  'mac14-arm64': { playwright: '1.65.1', revision: '2251', archive: 'webkit-mac-14-arm64.zip', sha256: '2754eca5bab8773a6b1315528e7d8f342977ace799b3a91314e1be0cb324ee49', bytes: 82217310 },
  mac15: { playwright: '1.65.1', revision: '2388', archive: 'webkit-mac-15.zip', sha256: 'c1220c739a2dbb6450059bfd678d5f5057e807d1d004464ef890ee1e6a9db727', bytes: 93121084 },
  'mac15-arm64': { playwright: '1.65.1', revision: '2388', archive: 'webkit-mac-15-arm64.zip', sha256: '50b4238e145b8018e48aacefc15e9b3a43fafd18eee42ed4cb29a2e5c5a2e385', bytes: 85207540 },
  mac26: { playwright: '1.65.1', revision: '2388', archive: 'webkit-mac-26.zip', sha256: '43603b6130fc1341e02b87dde0d635f02b6d2defb90bc4f4e4b4a2d101900872', bytes: 93747438 },
  'mac26-arm64': { playwright: '1.65.1', revision: '2388', archive: 'webkit-mac-26-arm64.zip', sha256: 'd3cc730027105a014d0cbf53925bd7eababbdf8dbccbb4a3a1713c5f631dbcaf', bytes: 85789503 },
} as const satisfies Record<string, Pin>;

/** A macOS host with a pinned build. */
export type MacHost = keyof typeof MAC_PINS;

/** The last macOS version Playwright builds WebKit for; a newer one takes its build. */
const LAST_MACOS = 26;

/** Playwright's name of this system for its downloads (playwright-core's `hostPlatform`):
 *  `win64` on Windows x64, `mac<version>[-arm64]` on macOS 14 and newer (by the Darwin version
 *  `release`, arm64 when a CPU is Apple's), null where rBrowser pins no WebKit build. */
export function hostPlatform(
  platform: NodeJS.Platform,
  arch: string,
  release: string,
  cpuModels: readonly string[],
): 'win64' | MacHost | null {
  if (platform === 'win32') return arch === 'x64' ? 'win64' : null;
  if (platform !== 'darwin') return null;
  const darwin = Number.parseInt(release.split('.')[0] ?? '', 10);
  // Darwin 23 is macOS 14; Playwright builds nothing older for this WebKit.
  if (!Number.isInteger(darwin) || darwin < 23) return null;
  const version = darwin < 25 ? 14 + (darwin - 23) : Math.min(darwin + 1, LAST_MACOS);
  const apple = cpuModels.some((model) => model.includes('Apple'));
  return `mac${version}${apple ? '-arm64' : ''}` as MacHost;
}

/** This machine's `hostPlatform`. */
export function currentHost(): 'win64' | MacHost | null {
  return hostPlatform(process.platform, process.arch, release(), cpus().map((cpu) => cpu.model));
}

/** Why there is no WebKit on this system. */
export const NO_BUILD = 'rBrowser pins a WebKit build for Windows x64 and macOS 14 or newer only';

/** The pinned archive of `host`; null without one. */
export function pinFor(host: 'win64' | MacHost | null): Pin | null {
  if (host === null) return null;
  return host === 'win64' ? WEBKIT_PIN : MAC_PINS[host];
}

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
