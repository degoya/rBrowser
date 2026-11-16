// The real Lighthouse run (RB-060-07): a headless Chromium of its own (browser.ts), started by
// puppeteer-core over a pipe - no remote-debugging port, so no other local process can reach it
// - with a fresh temporary profile that puppeteer removes again; Lighthouse navigates its page
// with the breakpoint's emulation and hands back the JSON result (LHR) and the HTML report.
// Nothing of the browser outlives its run - done, failed or cancelled: it is closed, else its
// whole process tree killed (end.ts, tree.ts), its profile removed, and its start and end are
// logged with the pid (RB-080-67).
// Lighthouse and puppeteer-core load on the first run only (a dynamic import the bundle leaves
// alone), so the sidecar starts as fast as before; neither sends anything anywhere
// (`enableErrorReporting: false`: Lighthouse's Sentry stays unloaded).

import { statSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';

import { type Browser, type BrowserDeps, describeSearch, maskPath, searchBrowser } from './browser.ts';
import { type BrowserProcess, type EndDeps, endBrowser, makeProfile, removeProfile } from './end.ts';
import { LighthouseCode, LighthouseError, type RunOutput, type Runner } from './jobs.ts';
import type { RunSettings } from './params.ts';

/** What the desktop preset of Lighthouse sets beyond the form factor. */
export interface DesktopPreset {
  throttling: unknown;
  emulatedUserAgent: unknown;
}

interface LaunchedBrowser {
  newPage(): Promise<unknown>;
  close(): Promise<void>;
  /** The browser's main process (puppeteer's `Browser.process()`). */
  process(): BrowserProcess | null;
}

/** The parts of puppeteer-core and Lighthouse a run uses. */
export interface Libraries {
  desktop: DesktopPreset;
  launch(options: Record<string, unknown>): Promise<LaunchedBrowser>;
  lighthouse(
    url: string,
    flags: Record<string, unknown>,
    config: unknown,
    page: unknown,
  ): Promise<{ report: unknown; lhr: { finalDisplayedUrl?: unknown; lighthouseVersion?: unknown } } | undefined>;
}

export interface RealDeps extends EndDeps {
  findBrowser(): Browser | null;
  load(): Promise<Libraries>;
  /** Where each run's profile folder goes (the temporary folder). */
  profiles: { root: string };
  /** Opened when a measurement is cancelled under Lighthouse (dangling.ts). */
  dangling: { open(): void };
}

/** The Lighthouse config of one run: the default config with the breakpoint's emulation. */
export function configOf(
  settings: RunSettings,
  desktop: DesktopPreset,
): { extends: 'lighthouse:default'; settings: Record<string, unknown> } {
  const out: Record<string, unknown> = {
    formFactor: settings.formFactor,
    screenEmulation: settings.screenEmulation,
    throttlingMethod: settings.throttlingMethod,
    onlyCategories: settings.onlyCategories,
  };
  if (settings.formFactor === 'desktop') {
    out.throttling = desktop.throttling;
    out.emulatedUserAgent = desktop.emulatedUserAgent;
  }
  if (settings.emulatedUserAgent !== null) {
    out.emulatedUserAgent = settings.emulatedUserAgent;
  }
  out.output = ['json', 'html'];
  return { extends: 'lighthouse:default', settings: out };
}

/** The specifiers stay out of the bundle (scripts/bundle.ts) through a variable. */
const LIGHTHOUSE = 'lighthouse';
const PUPPETEER = 'puppeteer-core';

/** Loads Lighthouse and puppeteer-core from the sidecar's node_modules. */
export async function loadLibraries(): Promise<Libraries> {
  const lh = (await import(LIGHTHOUSE)) as {
    default: Libraries['lighthouse'];
    desktopConfig: { settings: DesktopPreset };
  };
  const puppeteer = (await import(PUPPETEER)) as { default: { launch: Libraries['launch'] } };
  return {
    desktop: { throttling: lh.desktopConfig.settings.throttling, emulatedUserAgent: lh.desktopConfig.settings.emulatedUserAgent },
    launch: (options) => puppeteer.default.launch(options),
    lighthouse: lh.default,
  };
}

/** The machine's side of a search: the process's environment and file system. */
export function machine(): BrowserDeps {
  return {
    env: process.env,
    platform: process.platform,
    home: homedir(),
    isFile: (path) => {
      try {
        return statSync(path).isFile();
      } catch {
        return false;
      }
    },
    list: (dir) => {
      try {
        return readdirSync(dir);
      } catch {
        return [];
      }
    },
  };
}

/**
 * A browser search per run that logs its outcome (browser.ts, masked) only when it differs from
 * the last one - once per sidecar start while it stays the same (RB-080-58). A set browser that
 * does not exist throws `BrowserMissing`, never falls back.
 */
export function browserFinder(log: (line: string) => void, deps: () => BrowserDeps): () => Browser | null {
  let last = '';
  return () => {
    const machineDeps = deps();
    const search = searchBrowser(machineDeps);
    const line = describeSearch(search, machineDeps);
    if (line !== last) {
      last = line;
      log(line);
    }
    if (search.missing !== undefined) {
      throw new LighthouseError(LighthouseCode.BrowserMissing, `${maskPath(search.missing, machineDeps)} does not exist`);
    }
    return search.browser;
  };
}

/** One line into rBrowser's log: the sidecar's stderr. */
export const stderr = (line: string) => {
  process.stderr.write(`${line}\n`);
};

/** The browser of this machine, searched once (for the opt-in real run's check). */
export function systemBrowser(log: (line: string) => void = stderr): Browser | null {
  return browserFinder(log, machine)();
}

/** The sidecar's one finder: its log line lands in rBrowser's log through stderr. */
export const sidecarBrowser = browserFinder(stderr, machine);

/** Runs Lighthouse in a headless browser of its own per run; nothing of it outlives the run. */
export function realRunner(deps: RealDeps): Runner {
  return async (request, signal): Promise<RunOutput> => {
    const browser = deps.findBrowser();
    if (browser === null) {
      throw new LighthouseError(
        LighthouseCode.NoBrowser,
        'no Chromium-based browser to run Lighthouse in: install Google Chrome or Microsoft Edge, or set engines.lighthouseBrowserPath',
      );
    }
    const libraries = await deps.load();
    signal.throwIfAborted();
    const profile = await makeProfile(deps.profiles.root);
    try {
      return await measure(libraries, browser, profile, request, signal, deps);
    } finally {
      await removeProfile(profile, deps.log);
    }
  };
}

/** One run in a browser of its own with `profile`, ended whatever happens. */
async function measure(
  libraries: Libraries,
  browser: Browser,
  profile: string,
  request: Parameters<Runner>[0],
  signal: AbortSignal,
  deps: RealDeps,
): Promise<RunOutput> {
  const launched = await libraries.launch({
    executablePath: browser.path,
    headless: browser.headless,
    pipe: true,
    defaultViewport: null,
    userDataDir: profile,
  });
  deps.log(`lighthouse: browser started pid=${launched.process()?.pid ?? '?'}`);
  // One end per browser, whether the abort or the run's own end comes first.
  let ending: Promise<void> | null = null;
  const close = () => (ending ??= endBrowser(launched, deps));
  let measuring = false;
  const onAbort = () => {
    if (measuring) {
      deps.dangling.open();
    }
    void close();
  };
  signal.addEventListener('abort', onAbort, { once: true });
  try {
    // Cancelled while the browser started: no Lighthouse in it.
    signal.throwIfAborted();
    const page = await launched.newPage();
    const config = configOf(request.settings, libraries.desktop);
    const flags = { logLevel: 'error', enableErrorReporting: false };
    measuring = true;
    const result = await libraries.lighthouse(request.url, flags, config, page).finally(() => {
      measuring = false;
    });
    const report = result?.report;
    if (result === undefined || !Array.isArray(report) || typeof report[0] !== 'string' || typeof report[1] !== 'string') {
      throw new LighthouseError(LighthouseCode.RunFailed, 'Lighthouse returned no report');
    }
    return {
      json: report[0],
      html: report[1],
      finalUrl: typeof result.lhr.finalDisplayedUrl === 'string' ? result.lhr.finalDisplayedUrl : request.url,
      lighthouseVersion: typeof result.lhr.lighthouseVersion === 'string' ? result.lhr.lighthouseVersion : '',
    };
  } finally {
    signal.removeEventListener('abort', onAbort);
    await close();
  }
}
