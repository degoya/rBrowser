// `playwright.install`: the Playwright browser builds rBrowser needs (WebKit, RB-040-04), in
// `PLAYWRIGHT_BROWSERS_PATH` (rb-sidecar sets it: `%LOCALAPPDATA%\rBrowser\ms-playwright` or the
// setting `engines.playwrightBrowsersPath`). The download is the pinned archive, checked against
// its SHA-256 (webkit/install.ts, RB-040-04); WebKit is installed when rBrowser's marker in its
// directory names that SHA-256 (webkit/pin.ts, `PIN_MARKER`), another build when Playwright's
// `INSTALLATION_COMPLETE` is there.

import { join } from 'node:path';

import { ErrorCode, RpcError } from '../rpc.ts';
import type { CallContext, Router } from '../router.ts';
import { type Pin, WEBKIT_PIN, isPinnedBuild } from '../webkit/pin.ts';

/** The browsers this method installs: Chromium is CEF and Firefox the installed one (BiDi). */
export const INSTALLABLE = ['webkit'] as const;

export interface BrowserEntry {
  name: string;
  revision: string;
  version: string | null;
}

export interface BrowserState extends BrowserEntry {
  installed: boolean;
  path: string | null;
}

export interface PlaywrightDeps {
  /** `PLAYWRIGHT_BROWSERS_PATH`; unset only outside rBrowser. */
  browsersPath: string | undefined;
  /** The builds of the pinned playwright-core (its `browsers.json`). */
  entries(): BrowserEntry[];
  /** A small file's text; null when it cannot be read. */
  readText(path: string): string | null;
  /** This system's pinned WebKit archive (webkit/pin.ts `pinFor`, RB-097-02); undefined is the
   *  Windows one, null none - WebKit is never installed then. */
  webkitPin?: Pin | null;
  /** Runs the download; `onLine` gets every output line. Rejects on failure. */
  runInstall(browser: string, signal: AbortSignal, onLine: (line: string) => void): Promise<void>;
}

/** Reads playwright-core's `browsers.json`; `host` is Playwright's name of this system
 *  (`mac14-arm64`, webkit/pin.ts `hostPlatform`), whose `revisionOverrides` entry wins
 *  (RB-097-02: macOS 14 keeps an older WebKit). */
export function parseBrowsersJson(text: string, host: string | null = null): BrowserEntry[] {
  const data = JSON.parse(text) as { browsers?: unknown };
  if (!Array.isArray(data.browsers)) {
    throw new Error('browsers.json has no browsers list');
  }
  return data.browsers.flatMap((entry: unknown) => {
    const e = entry as Record<string, unknown>;
    if (typeof e.name !== 'string' || typeof e.revision !== 'string') {
      return [];
    }
    const version = typeof e.browserVersion === 'string' ? e.browserVersion : null;
    return [{ name: e.name, revision: overridden(e.revisionOverrides, host) ?? e.revision, version }];
  });
}

/** The revision `overrides` give `host`, when they give it one as text. */
function overridden(overrides: unknown, host: string | null): string | null {
  if (host === null || typeof overrides !== 'object' || overrides === null) return null;
  const revision = (overrides as Record<string, unknown>)[host];
  return typeof revision === 'string' ? revision : null;
}

/** The percentage of a Playwright progress line (`|■■■   | 42% of 55.3 MiB`), else null. */
export function parseProgress(line: string): number | null {
  const match = /\|\s*(\d{1,3})% of /.exec(line);
  return match?.[1] === undefined ? null : Math.min(100, Number(match[1]));
}

function directory(browsersPath: string, entry: BrowserEntry): string {
  return join(browsersPath, `${entry.name}-${entry.revision}`);
}

function isInstalled(deps: PlaywrightDeps, entry: BrowserEntry, dir: string): boolean {
  if (entry.name === 'webkit') {
    const pin = deps.webkitPin === undefined ? WEBKIT_PIN : deps.webkitPin;
    return pin !== null && isPinnedBuild(dir, (path) => deps.readText(path), pin);
  }
  return deps.readText(join(dir, 'INSTALLATION_COMPLETE')) !== null;
}

/** Every build of the pinned Playwright and whether it is installed. */
export function browserStates(deps: PlaywrightDeps): BrowserState[] {
  const root = deps.browsersPath;
  return deps.entries().map((entry) => {
    if (root === undefined) {
      return { ...entry, installed: false, path: null };
    }
    const path = directory(root, entry);
    return { ...entry, installed: isInstalled(deps, entry, path), path };
  });
}

function browserParam(params: unknown): string {
  if (params === undefined || params === null) {
    return 'webkit';
  }
  if (typeof params !== 'object' || Array.isArray(params)) {
    throw new RpcError(ErrorCode.InvalidParams, 'params must be an object');
  }
  const browser = (params as Record<string, unknown>).browser ?? 'webkit';
  if (!(INSTALLABLE as readonly unknown[]).includes(browser)) {
    throw new RpcError(ErrorCode.InvalidParams, `browser must be one of ${INSTALLABLE.join(', ')}`, { browser });
  }
  return browser as string;
}

async function install(deps: PlaywrightDeps, params: unknown, ctx: CallContext): Promise<unknown> {
  const browser = browserParam(params);
  const root = deps.browsersPath;
  if (root === undefined) {
    throw new RpcError(ErrorCode.NoBrowsersPath, 'PLAYWRIGHT_BROWSERS_PATH is not set');
  }
  const entry = deps.entries().find((e) => e.name === browser);
  if (entry === undefined) {
    throw new RpcError(ErrorCode.InstallFailed, `the pinned Playwright has no ${browser} build`);
  }
  const path = directory(root, entry);
  const answer = (alreadyInstalled: boolean) => ({
    browser, revision: entry.revision, version: entry.version, path, alreadyInstalled,
  });
  if (isInstalled(deps, entry, path)) {
    return answer(true);
  }
  ctx.progress({ browser, stage: 'download', percent: 0 });
  await deps.runInstall(browser, ctx.signal, (line) => {
    const percent = parseProgress(line);
    if (percent !== null) {
      ctx.progress({ browser, stage: 'download', percent });
    }
  });
  if (!isInstalled(deps, entry, path)) {
    throw new RpcError(ErrorCode.InstallFailed, `the ${browser} install finished without its marker`);
  }
  ctx.progress({ browser, stage: 'done', percent: 100 });
  return answer(false);
}

export function registerPlaywright(router: Router, deps: PlaywrightDeps): void {
  router.register('playwright.install', (params, ctx) => install(deps, params, ctx));
}
