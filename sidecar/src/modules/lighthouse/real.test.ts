import { tmpdir } from 'node:os';

import { describe, expect, it } from 'vitest';

import { RpcError } from '../../rpc.ts';
import { LighthouseCode } from './jobs.ts';
import { readStart } from './params.ts';
import { type Libraries, browserFinder, configOf, realRunner, systemBrowser } from './real.ts';

const DESKTOP = {
  throttling: { rttMs: 40, throughputKbps: 10240, cpuSlowdownMultiplier: 1 },
  emulatedUserAgent: 'Desktop UA',
};

function request(formFactor: 'mobile' | 'desktop', agent?: string) {
  return readStart({
    url: 'https://example.com/',
    settings: {
      formFactor,
      screenEmulation: { width: formFactor === 'mobile' ? 375 : 1440, height: 800, deviceScaleFactor: 2, mobile: formFactor === 'mobile' },
      throttlingMethod: 'simulate',
      onlyCategories: ['performance'],
      ...(agent === undefined ? {} : { emulatedUserAgent: agent }),
    },
  });
}

describe('configOf', () => {
  it("extends Lighthouse's default config with the breakpoint's emulation", () => {
    expect(configOf(request('mobile', 'Phone UA').settings, DESKTOP)).toEqual({
      extends: 'lighthouse:default',
      settings: {
        formFactor: 'mobile',
        screenEmulation: { width: 375, height: 800, deviceScaleFactor: 2, mobile: true, disabled: false },
        throttlingMethod: 'simulate',
        onlyCategories: ['performance'],
        emulatedUserAgent: 'Phone UA',
        output: ['json', 'html'],
      },
    });
  });

  it("keeps Lighthouse's mobile user agent and throttling when none is given", () => {
    const settings = configOf(request('mobile').settings, DESKTOP).settings;
    expect(settings).not.toHaveProperty('emulatedUserAgent');
    expect(settings).not.toHaveProperty('throttling');
  });

  it("takes the desktop preset's throttling and user agent for a desktop breakpoint", () => {
    const settings = configOf(request('desktop').settings, DESKTOP).settings;
    expect(settings.throttling).toEqual(DESKTOP.throttling);
    expect(settings.emulatedUserAgent).toBe('Desktop UA');
  });
});

describe('realRunner', () => {
  it('refuses to run without a browser, before loading Lighthouse', async () => {
    let loaded = false;
    const runner = realRunner({
      findBrowser: () => null,
      load: async () => {
        loaded = true;
        return {} as Libraries;
      },
      killTree: async () => 0,
      log: () => {},
      profiles: { root: tmpdir() },
      dangling: { open: () => {} },
    });
    const error = await runner(request('mobile'), new AbortController().signal).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(RpcError);
    expect((error as RpcError).code).toBe(LighthouseCode.NoBrowser);
    expect(loaded).toBe(false);
  });

  it('launches over a pipe, runs Lighthouse on its page and closes the browser', async () => {
    const calls: string[] = [];
    let launched: Record<string, unknown> = {};
    const libraries: Libraries = {
      desktop: DESKTOP,
      launch: async (options) => {
        launched = options;
        return {
          newPage: async () => ({ page: true }),
          process: () => ({ pid: 7, exitCode: null, signalCode: null }),
          close: async () => {
            calls.push('close');
          },
        };
      },
      lighthouse: async (url, flags, config, page) => {
        calls.push(`run ${url} ${JSON.stringify(flags)} ${String((config as { extends: string }).extends)} ${JSON.stringify(page)}`);
        return {
          report: ['{"a":1}', '<html></html>'],
          lhr: { finalDisplayedUrl: 'https://example.com/', lighthouseVersion: '13.6.0' },
        };
      },
    };
    const runner = realRunner({
      findBrowser: () => ({ path: '/chrome', headless: 'shell', source: 'playwright' }),
      load: async () => libraries,
      killTree: async (pid) => {
        calls.push(`kill tree ${pid}`);
        return 1;
      },
      log: () => {},
      profiles: { root: tmpdir() },
      dangling: { open: () => {} },
    });
    const output = await runner(request('mobile'), new AbortController().signal);
    expect(output).toEqual({ json: '{"a":1}', html: '<html></html>', finalUrl: 'https://example.com/', lighthouseVersion: '13.6.0' });
    expect(launched).toMatchObject({ executablePath: '/chrome', headless: 'shell', pipe: true });
    expect(calls).toEqual([
      'run https://example.com/ {"logLevel":"error","enableErrorReporting":false} lighthouse:default {"page":true}',
      'close',
    ]);
  });

  it('closes the browser when the run is cancelled', async () => {
    const calls: string[] = [];
    const controller = new AbortController();
    const libraries: Libraries = {
      desktop: DESKTOP,
      launch: async () => ({
        newPage: async () => ({}),
        process: () => null,
        close: async () => {
          calls.push('close');
        },
      }),
      lighthouse: () => {
        controller.abort();
        return Promise.reject(new Error('Target closed'));
      },
    };
    const runner = realRunner({
      findBrowser: () => ({ path: '/chrome', headless: true, source: 'installed' }),
      load: async () => libraries,
      killTree: async () => 0,
      log: () => {},
      profiles: { root: tmpdir() },
      dangling: { open: () => {} },
    });
    await expect(runner(request('mobile'), controller.signal)).rejects.toThrow('Target closed');
    expect(calls[0]).toBe('close');
  });
});

describe('systemBrowser', () => {
  const files = (present: string[]) => ({
    env: { LOCALAPPDATA: 'C:\\Users\\Jane\\AppData\\Local', SystemRoot: 'C:\\Windows' } as Record<string, string | undefined>,
    platform: 'win32' as NodeJS.Platform,
    home: 'C:\\Users\\Jane',
    isFile: (path: string) => present.includes(path),
    list: () => [],
  });
  const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';

  // RB-080-58 review 5: once per sidecar start, not per run.
  it('logs the chosen browser once while it stays the same', () => {
    const lines: string[] = [];
    const find = browserFinder((line) => lines.push(line), () => files([edge]));
    expect(find()?.path).toBe(edge);
    expect(find()?.path).toBe(edge);
    expect(lines).toEqual([`lighthouse: browser ${edge} (installed)`]);
  });

  // RB-080-58 review 1.
  it('refuses a set browser that does not exist with a code of its own', () => {
    const lines: string[] = [];
    const deps = files([edge]);
    deps.env.CHROME_PATH = 'C:\\Users\\Jane\\gone\\chrome.exe';
    const find = browserFinder((line) => lines.push(line), () => deps);
    let error: unknown;
    try {
      find();
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(RpcError);
    expect((error as RpcError).code).toBe(LighthouseCode.BrowserMissing);
    expect((error as RpcError).message).toBe('~\\gone\\chrome.exe does not exist');
    expect(lines).toHaveLength(1);
    expect(lines[0]).not.toContain('Jane');
  });

  it('reads this machine', () => {
    expect(() => systemBrowser(() => {})).not.toThrow();
  });
});
