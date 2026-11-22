// Which Chromium Lighthouse runs in (RB-060-07). Lighthouse loads and measures the page itself,
// so it gets a headless browser of its own - started per run over a pipe (no debugging port),
// with a fresh profile - and never touches the grid's views. The executable is chosen from fixed
// places only, never from a parameter of a call or the project, in this order:
//
// 1. `CHROME_PATH` (Lighthouse's own convention). rBrowser starts the sidecar with a cleared
//    environment and sets it from the user setting `engines.lighthouseBrowserPath` only
//    (rb-sidecar, RB-080-58). A set one that is no file is `missing` - an explicit choice that
//    does not work is an error, never a silent fallback to another browser;
// 2. Playwright's builds in `PLAYWRIGHT_BROWSERS_PATH` (else Playwright's default folder): the
//    newest `chromium_headless_shell-*`, then the newest `chromium-*`;
// 3. an installed Google Chrome, then Microsoft Edge (part of every Windows 10 and 11), in the
//    per-user and the machine-wide program folders. The folders come from the environment and,
//    because that environment is an allowlist that once left `ProgramFiles` out (RB-080-58: no
//    machine-wide Chrome or Edge was ever found), also from the system drive itself. On macOS
//    (RB-097-04) Chrome, Edge, then Chromium in `/Applications`, then in `~/Applications`; there
//    an app bundle set as `CHROME_PATH` stands for the executable inside it.
//
// `searchBrowser` also answers every path it looked at; `describeSearch` makes the log line,
// the profile folders masked (`%LOCALAPPDATA%`, `~`) so no Windows user name reaches the log.

import { posix, win32 } from 'node:path';

export interface BrowserDeps {
  env: Record<string, string | undefined>;
  platform: NodeJS.Platform;
  home: string;
  isFile(path: string): boolean;
  /** The names in a folder; empty when it cannot be read. */
  list(dir: string): string[];
}

export interface Browser {
  path: string;
  /** `shell` for chrome-headless-shell, `true` (the new headless mode) for a full browser. */
  headless: true | 'shell';
  source: 'CHROME_PATH' | 'playwright' | 'installed';
}

const SHELL_DIRS: Partial<Record<NodeJS.Platform, string[]>> = {
  win32: ['chrome-headless-shell-win64/chrome-headless-shell.exe'],
  linux: ['chrome-headless-shell-linux64/chrome-headless-shell'],
  darwin: [
    'chrome-headless-shell-mac-arm64/chrome-headless-shell',
    'chrome-headless-shell-mac-x64/chrome-headless-shell',
  ],
};

const CHROMIUM_DIRS: Partial<Record<NodeJS.Platform, string[]>> = {
  win32: ['chrome-win64/chrome.exe', 'chrome-win/chrome.exe'],
  linux: ['chrome-linux64/chrome', 'chrome-linux/chrome'],
  // Chrome for Testing since Playwright 1.49 (RB-097-04), Chromium.app before.
  darwin: [
    'chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing',
    'chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing',
    'chrome-mac-arm64/Chromium.app/Contents/MacOS/Chromium',
    'chrome-mac/Chromium.app/Contents/MacOS/Chromium',
  ],
};

/** The macOS apps Lighthouse runs in, in this order, each machine-wide before per-user. */
const MAC_APPS = ['Google Chrome', 'Microsoft Edge', 'Chromium'];

function pathApi(platform: NodeJS.Platform) {
  return platform === 'win32' ? win32 : posix;
}

/** Playwright's browsers folder: the variable, else its per-platform default. */
function playwrightRoot(deps: BrowserDeps): string {
  const path = pathApi(deps.platform);
  const set = deps.env.PLAYWRIGHT_BROWSERS_PATH;
  if (set !== undefined && set !== '' && set !== '0') {
    return set;
  }
  if (deps.platform === 'win32') {
    return path.join(deps.env.LOCALAPPDATA ?? path.join(deps.home, 'AppData', 'Local'), 'ms-playwright');
  }
  if (deps.platform === 'darwin') {
    return path.join(deps.home, 'Library', 'Caches', 'ms-playwright');
  }
  return path.join(deps.home, '.cache', 'ms-playwright');
}

/** The newest `<prefix><revision>` folder's executable among `relatives`. */
function newest(deps: BrowserDeps, root: string, prefix: string, relatives: string[], probed: string[]): string | null {
  const path = pathApi(deps.platform);
  const revisions = deps
    .list(root)
    .filter((name) => name.startsWith(prefix) && /^\d+$/.test(name.slice(prefix.length)))
    .sort((a, b) => Number(b.slice(prefix.length)) - Number(a.slice(prefix.length)));
  if (revisions.length === 0) {
    probed.push(path.join(root, `${prefix}*`));
  }
  for (const dir of revisions) {
    for (const relative of relatives) {
      const candidate = path.join(root, dir, ...relative.split('/'));
      probed.push(candidate);
      if (deps.isFile(candidate)) {
        return candidate;
      }
    }
  }
  return null;
}

/** The program folders of Windows: the environment's, then the system drive's own. */
function windowsRoots(deps: BrowserDeps): string[] {
  const system = [deps.env.SystemDrive, deps.env.SystemRoot].find((value) => value !== undefined && value !== '');
  const drive = /^[A-Za-z]:/.exec(system ?? '')?.[0] ?? 'C:';
  const roots = [
    deps.env.LOCALAPPDATA,
    win32.join(deps.home, 'AppData', 'Local'),
    deps.env.ProgramFiles,
    deps.env.ProgramW6432,
    `${drive}\\Program Files`,
    deps.env['ProgramFiles(x86)'],
    `${drive}\\Program Files (x86)`,
  ];
  const seen = new Set<string>();
  return roots.filter((root): root is string => {
    if (root === undefined || root === '' || seen.has(root.toLowerCase())) {
      return false;
    }
    seen.add(root.toLowerCase());
    return true;
  });
}

function installed(deps: BrowserDeps): string[] {
  const path = pathApi(deps.platform);
  if (deps.platform === 'win32') {
    const roots = windowsRoots(deps);
    const chrome = roots.map((root) => path.join(root, 'Google', 'Chrome', 'Application', 'chrome.exe'));
    const edge = roots.map((root) => path.join(root, 'Microsoft', 'Edge', 'Application', 'msedge.exe'));
    return [...chrome, ...edge];
  }
  if (deps.platform === 'darwin') {
    const roots = ['/Applications', path.join(deps.home, 'Applications')];
    return MAC_APPS.flatMap((name) => roots.map((root) => appExecutable(path.join(root, `${name}.app`))));
  }
  return ['/usr/bin/google-chrome', '/usr/bin/google-chrome-stable', '/usr/bin/chromium', '/usr/bin/microsoft-edge'];
}

/** The executable of the macOS app bundle `app`: `<app>/Contents/MacOS/<its name>`. */
function appExecutable(app: string): string {
  const bundle = app.replace(/\/+$/, '');
  return posix.join(bundle, 'Contents', 'MacOS', posix.basename(bundle, '.app'));
}

/**
 * The file `CHROME_PATH` names: on macOS an app bundle (what a file picker hands over, a folder)
 * stands for its executable; everywhere else the path as it is.
 */
function chromePathFile(set: string, platform: NodeJS.Platform): string {
  return platform === 'darwin' && /\.app\/*$/.test(set) ? appExecutable(set) : set;
}

/** What a search found and every path it looked at, in order. */
export interface Search {
  browser: Browser | null;
  /** The `CHROME_PATH` that was set but is no file; nothing else was looked at then. */
  missing?: string;
  probed: string[];
}

/** The browser Lighthouse runs in (null when there is none) and the paths looked at. */
export function searchBrowser(deps: BrowserDeps): Search {
  const probed: string[] = [];
  const set = deps.env.CHROME_PATH;
  if (set !== undefined && set !== '') {
    const file = chromePathFile(set, deps.platform);
    probed.push(file);
    if (deps.isFile(file)) {
      return { browser: { path: file, headless: true, source: 'CHROME_PATH' }, probed };
    }
    return { browser: null, missing: set, probed };
  }
  const root = playwrightRoot(deps);
  const shell = newest(deps, root, 'chromium_headless_shell-', SHELL_DIRS[deps.platform] ?? [], probed);
  if (shell !== null) {
    return { browser: { path: shell, headless: 'shell', source: 'playwright' }, probed };
  }
  const chromium = newest(deps, root, 'chromium-', CHROMIUM_DIRS[deps.platform] ?? [], probed);
  if (chromium !== null) {
    return { browser: { path: chromium, headless: true, source: 'playwright' }, probed };
  }
  for (const candidate of installed(deps)) {
    probed.push(candidate);
    if (deps.isFile(candidate)) {
      return { browser: { path: candidate, headless: true, source: 'installed' }, probed };
    }
  }
  return { browser: null, probed };
}

/** The browser Lighthouse runs in, or null when there is none. */
export function findBrowser(deps: BrowserDeps): Browser | null {
  return searchBrowser(deps).browser;
}

/** `path` with the profile folders replaced: `%LOCALAPPDATA%`, then the home folder as `~`. */
export function maskPath(path: string, deps: BrowserDeps): string {
  const windows = deps.platform === 'win32';
  const fold = (text: string) => (windows ? text.toLowerCase() : text);
  const separators = windows ? ['\\', '/'] : ['/'];
  const prefixes: [string | undefined, string][] = [
    [deps.env.LOCALAPPDATA, '%LOCALAPPDATA%'],
    [deps.env.USERPROFILE, '~'],
    [deps.home, '~'],
  ];
  for (const [prefix, name] of prefixes) {
    if (prefix === undefined || prefix === '') {
      continue;
    }
    const rest = path.slice(prefix.length);
    if (fold(path.slice(0, prefix.length)) === fold(prefix) && (rest === '' || separators.includes(rest[0] ?? ''))) {
      return name + rest;
    }
  }
  return path;
}

/** One log line for a search: the browser taken, or - only then - every path looked at. */
export function describeSearch(search: Search, deps: BrowserDeps): string {
  if (search.missing !== undefined) {
    return `lighthouse: the browser of engines.lighthouseBrowserPath does not exist: ${maskPath(search.missing, deps)}`;
  }
  if (search.browser !== null) {
    return `lighthouse: browser ${maskPath(search.browser.path, deps)} (${search.browser.source})`;
  }
  return `lighthouse: no browser found; looked at: ${search.probed.map((path) => maskPath(path, deps)).join(' | ')}`;
}
