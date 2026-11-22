import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { ErrorCode, RpcError } from '../rpc.ts';
import { Router } from '../router.ts';
import {
  type PlaywrightDeps,
  browserStates,
  parseBrowsersJson,
  parseProgress,
  registerPlaywright,
} from './playwright.ts';
import { MAC_PINS, PIN_MARKER, WEBKIT_PIN } from '../webkit/pin.ts';

const BROWSERS_JSON = JSON.stringify({
  browsers: [
    { name: 'chromium', revision: '1254', browserVersion: '158.0.8127.18' },
    { name: 'webkit', revision: '2388', browserVersion: '27.2' },
  ],
});

/** The files in the browsers folder and their text. */
function deps(overrides: Partial<PlaywrightDeps> = {}): PlaywrightDeps & { marker: Map<string, string> } {
  const marker = new Map<string, string>();
  return {
    marker,
    browsersPath: '/pw',
    entries: () => parseBrowsersJson(BROWSERS_JSON),
    readText: (path) => marker.get(path) ?? null,
    runInstall: async () => {},
    ...overrides,
  };
}

function ctx() {
  const progress: Record<string, unknown>[] = [];
  return { progress, ctx: { signal: new AbortController().signal, progress: (p: Record<string, unknown>) => progress.push(p) } };
}

const WEBKIT_DIR = join('/pw', 'webkit-2388');
const WEBKIT_MARKER = join(WEBKIT_DIR, PIN_MARKER);
const PLAYWRIGHT_MARKER = join(WEBKIT_DIR, 'INSTALLATION_COMPLETE');

describe('parseBrowsersJson', () => {
  it('reads name, revision and version', () => {
    expect(parseBrowsersJson(BROWSERS_JSON)[1]).toEqual({ name: 'webkit', revision: '2388', version: '27.2' });
  });

  it('takes the revision of the host when browsers.json overrides it (RB-097-02)', () => {
    const json = JSON.stringify({
      browsers: [{ name: 'webkit', revision: '2388', revisionOverrides: { mac14: '2251', 'mac14-arm64': '2251' } }],
    });
    expect(parseBrowsersJson(json, 'mac14-arm64')[0]?.revision).toBe('2251');
    expect(parseBrowsersJson(json, 'mac15')[0]?.revision).toBe('2388');
    expect(parseBrowsersJson(json, null)[0]?.revision).toBe('2388');
    expect(parseBrowsersJson(json)[0]?.revision).toBe('2388');
    const odd = JSON.stringify({ browsers: [{ name: 'webkit', revision: '2388', revisionOverrides: { mac14: 7 } }] });
    expect(parseBrowsersJson(odd, 'mac14')[0]?.revision).toBe('2388');
  });

  it('refuses a file without a browsers list', () => {
    expect(() => parseBrowsersJson('{}')).toThrow('browsers.json has no browsers list');
  });
});

describe('parseProgress', () => {
  it('reads the percentage of a Playwright progress line', () => {
    expect(parseProgress('|■■■■■■■■                | 42% of 55.3 MiB')).toBe(42);
    expect(parseProgress('|■■■■■■■■■■■■■■■■■■■■■■■■| 100% of 55.3 MiB')).toBe(100);
  });

  it('ignores other lines', () => {
    expect(parseProgress('Downloading WebKit 27.2 (playwright build v2388)')).toBeNull();
  });
});

describe('browserStates', () => {
  it('marks WebKit installed only when rBrowser\'s marker names the pinned archive', () => {
    const d = deps();
    d.marker.set(PLAYWRIGHT_MARKER, '');
    expect(browserStates(d)[1]?.installed).toBe(false);
    d.marker.set(WEBKIT_MARKER, '0'.repeat(64));
    expect(browserStates(d)[1]?.installed).toBe(false);
    d.marker.set(WEBKIT_MARKER, `${WEBKIT_PIN.sha256}\n`);
    expect(browserStates(d)).toEqual([
      { name: 'chromium', revision: '1254', version: '158.0.8127.18', installed: false, path: join('/pw', 'chromium-1254') },
      { name: 'webkit', revision: '2388', version: '27.2', installed: true, path: WEBKIT_DIR },
    ]);
  });

  it('holds WebKit to this system\'s pin: a macOS build by its own archive, none without a pin (RB-097-02)', () => {
    const mac = deps({ webkitPin: MAC_PINS['mac15-arm64'] });
    mac.marker.set(WEBKIT_MARKER, WEBKIT_PIN.sha256);
    expect(browserStates(mac)[1]?.installed).toBe(false);
    mac.marker.set(WEBKIT_MARKER, MAC_PINS['mac15-arm64'].sha256);
    expect(browserStates(mac)[1]?.installed).toBe(true);
    const none = deps({ webkitPin: null });
    none.marker.set(WEBKIT_MARKER, WEBKIT_PIN.sha256);
    expect(browserStates(none)[1]?.installed).toBe(false);
  });

  it('reports nothing installed without a browsers path', () => {
    expect(browserStates(deps({ browsersPath: undefined })).every((b) => !b.installed && b.path === null)).toBe(true);
  });
});

describe('playwright.install', () => {
  it('recognises an existing installation and downloads nothing', async () => {
    let runs = 0;
    const d = deps({ runInstall: async () => { runs += 1; } });
    d.marker.set(WEBKIT_MARKER, WEBKIT_PIN.sha256);
    const router = new Router();
    registerPlaywright(router, d);
    const { ctx: c } = ctx();
    expect(await router.call('playwright.install', {}, c)).toEqual({
      browser: 'webkit', revision: '2388', version: '27.2', path: WEBKIT_DIR, alreadyInstalled: true,
    });
    expect(runs).toBe(0);
  });

  it('installs and reports progress', async () => {
    const d = deps();
    d.runInstall = async (browser, _signal, onLine) => {
      expect(browser).toBe('webkit');
      onLine('Downloading WebKit 27.2');
      onLine('|■■■■■■                  | 30% of 55.3 MiB');
      onLine('|■■■■■■■■■■■■■■■■■■■■■■■■| 100% of 55.3 MiB');
      d.marker.set(WEBKIT_MARKER, WEBKIT_PIN.sha256);
    };
    const router = new Router();
    registerPlaywright(router, d);
    const { ctx: c, progress } = ctx();
    const result = await router.call('playwright.install', { browser: 'webkit' }, c);
    expect(result).toMatchObject({ alreadyInstalled: false, path: WEBKIT_DIR });
    expect(progress).toEqual([
      { browser: 'webkit', stage: 'download', percent: 0 },
      { browser: 'webkit', stage: 'download', percent: 30 },
      { browser: 'webkit', stage: 'download', percent: 100 },
      { browser: 'webkit', stage: 'done', percent: 100 },
    ]);
  });

  it('fails when the install leaves no completion marker', async () => {
    const router = new Router();
    registerPlaywright(router, deps());
    const { ctx: c } = ctx();
    await expect(router.call('playwright.install', {}, c)).rejects.toMatchObject({ code: ErrorCode.InstallFailed });
  });

  it('refuses a browser other than webkit', async () => {
    const router = new Router();
    registerPlaywright(router, deps());
    const { ctx: c } = ctx();
    await expect(router.call('playwright.install', { browser: 'chromium' }, c)).rejects.toMatchObject({
      code: ErrorCode.InvalidParams,
    });
    await expect(router.call('playwright.install', 'webkit', c)).rejects.toBeInstanceOf(RpcError);
  });

  it('refuses to install without a browsers path', async () => {
    const router = new Router();
    registerPlaywright(router, deps({ browsersPath: undefined }));
    const { ctx: c } = ctx();
    await expect(router.call('playwright.install', {}, c)).rejects.toMatchObject({ code: ErrorCode.NoBrowsersPath });
  });
});
