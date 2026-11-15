import { win32, posix } from 'node:path';

import { describe, expect, it } from 'vitest';

import { type BrowserDeps, describeSearch, findBrowser, maskPath, searchBrowser } from './browser.ts';

function deps(files: string[], env: Record<string, string>, platform: NodeJS.Platform, dirs: Record<string, string[]> = {}): BrowserDeps {
  return {
    env,
    platform,
    home: platform === 'win32' ? 'C:\\Users\\me' : '/home/me',
    isFile: (path) => files.includes(path),
    list: (dir) => dirs[dir] ?? [],
  };
}

describe('findBrowser', () => {
  it('takes CHROME_PATH first when it is a file', () => {
    const d = deps(['/opt/chrome/chrome', '/usr/bin/google-chrome'], { CHROME_PATH: '/opt/chrome/chrome' }, 'linux');
    expect(findBrowser(d)).toEqual({ path: '/opt/chrome/chrome', headless: true, source: 'CHROME_PATH' });
  });

  // RB-080-58 review 1: an explicit choice that does not work is an error, never a silent fallback.
  it('answers a CHROME_PATH that is no file as missing, without looking further', () => {
    const d = deps(['/usr/bin/google-chrome'], { CHROME_PATH: '/nope' }, 'linux');
    const search = searchBrowser(d);
    expect(search.browser).toBeNull();
    expect(search.missing).toBe('/nope');
    expect(search.probed).toEqual(['/nope']);
    expect(findBrowser(d)).toBeNull();
  });

  it("prefers Playwright's newest headless shell, then its Chromium", () => {
    const root = 'C:\\pw';
    const shell = win32.join(root, 'chromium_headless_shell-1254', 'chrome-headless-shell-win64', 'chrome-headless-shell.exe');
    const chromium = win32.join(root, 'chromium-1254', 'chrome-win64', 'chrome.exe');
    const dirs = { [root]: ['webkit-2388', 'chromium_headless_shell-1248', 'chromium_headless_shell-1254', 'chromium-1254'] };
    const env = { PLAYWRIGHT_BROWSERS_PATH: root, ProgramFiles: 'C:\\Program Files' };
    expect(findBrowser(deps([shell, chromium], env, 'win32', dirs))).toEqual({
      path: shell,
      headless: 'shell',
      source: 'playwright',
    });
    expect(findBrowser(deps([chromium], env, 'win32', dirs))).toEqual({ path: chromium, headless: true, source: 'playwright' });
  });

  it('falls back to an installed Chrome, then Edge, on Windows', () => {
    const env = { ProgramFiles: 'C:\\Program Files', 'ProgramFiles(x86)': 'C:\\Program Files (x86)', LOCALAPPDATA: 'C:\\Users\\me\\AppData\\Local' };
    const chrome = win32.join('C:\\Users\\me\\AppData\\Local', 'Google', 'Chrome', 'Application', 'chrome.exe');
    const edge = win32.join('C:\\Program Files (x86)', 'Microsoft', 'Edge', 'Application', 'msedge.exe');
    expect(findBrowser(deps([edge, chrome], env, 'win32'))).toEqual({ path: chrome, headless: true, source: 'installed' });
    expect(findBrowser(deps([edge], env, 'win32'))).toEqual({ path: edge, headless: true, source: 'installed' });
  });

  it("looks in Playwright's default folder when the variable is unset", () => {
    const root = posix.join('/home/me', '.cache', 'ms-playwright');
    const shell = posix.join(root, 'chromium_headless_shell-1254', 'chrome-headless-shell-linux64', 'chrome-headless-shell');
    const d = deps([shell], {}, 'linux', { [root]: ['chromium_headless_shell-1254'] });
    expect(findBrowser(d)?.path).toBe(shell);
  });

  it('answers null when there is none', () => {
    expect(findBrowser(deps([], {}, 'win32'))).toBeNull();
  });

  // RB-080-58: rBrowser starts the sidecar with an allowlisted environment that carried neither
  // `ProgramFiles` nor `ProgramFiles(x86)`, so a machine-wide Chrome or Edge was never looked at.
  it('finds a machine-wide Chrome or Edge on Windows without the ProgramFiles variables', () => {
    const env = { LOCALAPPDATA: 'C:\\Users\\me\\AppData\\Local', SystemRoot: 'C:\\Windows' };
    const chrome = 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
    const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
    expect(findBrowser(deps([chrome, edge], env, 'win32'))).toEqual({ path: chrome, headless: true, source: 'installed' });
    expect(findBrowser(deps([edge], env, 'win32'))).toEqual({ path: edge, headless: true, source: 'installed' });
  });

  it('takes the program folders of the system drive when Windows is not on C:', () => {
    const edge = 'D:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
    expect(findBrowser(deps([edge], { SystemRoot: 'D:\\Windows' }, 'win32'))?.path).toBe(edge);
    expect(findBrowser(deps([edge], { SystemDrive: 'D:' }, 'win32'))?.path).toBe(edge);
  });

  // RB-080-58 review 6: an empty SystemDrive counts as missing.
  it('falls back to SystemRoot when SystemDrive is empty', () => {
    const edge = 'D:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
    expect(findBrowser(deps([edge], { SystemDrive: '', SystemRoot: 'D:\\Windows' }, 'win32'))?.path).toBe(edge);
  });

  it('answers every path it looked at, each once', () => {
    const env = { ProgramFiles: 'C:\\Program Files', SystemRoot: 'C:\\Windows' };
    const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
    const none = searchBrowser(deps([], env, 'win32'));
    expect(none.browser).toBeNull();
    expect(none.missing).toBeUndefined();
    expect(none.probed).toContain('C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe');
    expect(none.probed).toContain(edge);
    expect(none.probed).toContain('C:\\Users\\me\\AppData\\Local\\ms-playwright\\chromium-*');
    expect(new Set(none.probed).size).toBe(none.probed.length);
    expect(searchBrowser(deps([edge], env, 'win32')).probed.at(-1)).toBe(edge);
  });
});

// RB-080-58 review 5: the log carries no Windows user name and no probe list for a found browser.
describe('the log line of a search', () => {
  const env = { LOCALAPPDATA: 'C:\\Users\\Jane Doe\\AppData\\Local', USERPROFILE: 'C:\\Users\\Jane Doe', SystemRoot: 'C:\\Windows' };
  const d = (files: string[], extra: Record<string, string> = {}) => ({
    ...deps(files, { ...env, ...extra }, 'win32'),
    home: 'C:\\Users\\Jane Doe',
  });

  it('masks the profile folders', () => {
    expect(maskPath('C:\\Users\\Jane Doe\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe', d([]))).toBe(
      '%LOCALAPPDATA%\\Google\\Chrome\\Application\\chrome.exe',
    );
    expect(maskPath('c:\\users\\jane doe\\tools\\chrome.exe', d([]))).toBe('~\\tools\\chrome.exe');
    expect(maskPath('C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe', d([]))).toBe(
      'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    );
    expect(maskPath('/home/me/.cache/x', deps([], {}, 'linux'))).toBe('~/.cache/x');
  });

  it('names a found browser without the paths looked at', () => {
    const chrome = 'C:\\Users\\Jane Doe\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe';
    const line = describeSearch(searchBrowser(d([chrome])), d([]));
    expect(line).toBe('lighthouse: browser %LOCALAPPDATA%\\Google\\Chrome\\Application\\chrome.exe (installed)');
  });

  it('lists the paths looked at, masked, only when none is found', () => {
    const line = describeSearch(searchBrowser(d([])), d([]));
    expect(line).toMatch(/^lighthouse: no browser found; looked at: /);
    expect(line).toContain('%LOCALAPPDATA%\\ms-playwright\\chromium-*');
    expect(line).not.toContain('Jane');
  });

  it('names a missing explicit browser, masked', () => {
    const line = describeSearch(searchBrowser(d([], { CHROME_PATH: 'C:\\Users\\Jane Doe\\chrome.exe' })), d([]));
    expect(line).toBe('lighthouse: the browser of engines.lighthouseBrowserPath does not exist: ~\\chrome.exe');
  });
});
