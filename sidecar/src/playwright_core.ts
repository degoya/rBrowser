// The real dependencies of the playwright module: the pinned playwright-core beside the bundle
// (`sidecar/node_modules/playwright-core`, kept out of the esbuild bundle) and the pinned,
// verified WebKit download (RB-040-04; Playwright's own installer checks no digest and is never
// run).

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

import { type BrowserEntry, type PlaywrightDeps, parseBrowsersJson } from './modules/playwright.ts';
import { ErrorCode, RpcError } from './rpc.ts';
import { installWebkit, realInstallDeps } from './webkit/install.ts';
import { readTextOrNull } from './webkit/pin.ts';

function packageDir(): string {
  const require = createRequire(import.meta.url);
  return dirname(require.resolve('playwright-core/package.json'));
}

export function playwrightVersion(): string {
  const data = JSON.parse(readFileSync(join(packageDir(), 'package.json'), 'utf8')) as { version?: unknown };
  return typeof data.version === 'string' ? data.version : 'unknown';
}

function entries(): BrowserEntry[] {
  return parseBrowsersJson(readFileSync(join(packageDir(), 'browsers.json'), 'utf8'));
}

/** The download of `playwright.install`: only WebKit, the pinned archive, verified (webkit/install.ts). */
function runInstall(browser: string, signal: AbortSignal, onLine: (line: string) => void): Promise<void> {
  const browsersPath = process.env.PLAYWRIGHT_BROWSERS_PATH;
  const webkit = entries().find((entry) => entry.name === browser);
  if (browser !== 'webkit' || webkit === undefined || browsersPath === undefined || browsersPath === '') {
    return Promise.reject(new RpcError(ErrorCode.InstallFailed, `no pinned download for ${browser}`));
  }
  return installWebkit(realInstallDeps(), browsersPath, webkit.revision, signal, onLine);
}

export function realPlaywrightDeps(env: NodeJS.ProcessEnv): PlaywrightDeps {
  const browsersPath = env.PLAYWRIGHT_BROWSERS_PATH;
  return {
    browsersPath: browsersPath === undefined || browsersPath === '' ? undefined : browsersPath,
    entries,
    readText: readTextOrNull,
    runInstall,
  };
}
