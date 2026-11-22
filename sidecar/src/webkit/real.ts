// The real dependencies of the WebKit module: playwright-core's `webkit` (beside the bundle, as
// the playwright module uses it) and the pinned build in `PLAYWRIGHT_BROWSERS_PATH`. The launch
// names the build's executable itself, so playwright-core neither looks for another build nor runs
// its host check (which would need Playwright's `winldd` download).

import { randomBytes } from 'node:crypto';
import { join } from 'node:path';

import type { BrowserEntry } from '../modules/playwright.ts';
import { ErrorCode, RpcError } from '../rpc.ts';
import { randomNames } from './agent.ts';
import { NO_BUILD, type Pin, WEBKIT_PIN, buildDirName, currentHost, executableName, isPinnedBuild, pinFor, readTextOrNull } from './pin.ts';
import type { ServiceDeps } from './service.ts';
import type { LauncherLike } from './types.ts';

/** The executable of the WebKit build `entries` name under `browsersPath`, when rBrowser
 *  installed it from `pin`, this system's pinned archive (`PIN_MARKER`; null: none here). */
export function installedExecutable(
  browsersPath: string | undefined,
  entries: BrowserEntry[],
  platform: NodeJS.Platform,
  readText: (path: string) => string | null,
  pin: Pin | null = WEBKIT_PIN,
): string {
  const webkit = entries.find((entry) => entry.name === 'webkit');
  if (browsersPath === undefined || webkit === undefined) {
    throw new RpcError(ErrorCode.WebkitNotInstalled, 'WebKit is not installed (no browsers folder)');
  }
  if (pin === null) {
    throw new RpcError(ErrorCode.WebkitNotInstalled, NO_BUILD);
  }
  const dir = join(browsersPath, buildDirName(webkit.revision));
  if (!isPinnedBuild(dir, readText, pin)) {
    throw new RpcError(ErrorCode.WebkitNotInstalled, `WebKit ${webkit.revision} is not installed`, {
      revision: webkit.revision,
    });
  }
  return join(dir, executableName(platform));
}

export function realServiceDeps(
  browsersPath: string | undefined,
  entries: () => BrowserEntry[],
  notify: (method: string, params: Record<string, unknown>) => void,
): ServiceDeps {
  return {
    launcher: async () => {
      const playwright = await import('playwright-core');
      // The one place the real type meets the module's interface; the fake tests the rest.
      return playwright.webkit as unknown as LauncherLike;
    },
    executablePath: () =>
      installedExecutable(browsersPath, entries(), process.platform, readTextOrNull, pinFor(currentHost())),
    notify,
    names: () => randomNames((bytes) => randomBytes(bytes)),
  };
}
