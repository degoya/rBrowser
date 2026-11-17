// RB-080-67: no process of a Lighthouse browser survives its run - done, failed or cancelled -
// and neither does its profile. A fake launcher counts what it starts and what ends. Puppeteer's
// graceful close comes first (review 3); the tree is killed only when the browser did not end
// within its grace, and only while its main process is known to live (review 4).

import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterAll, describe, expect, it } from 'vitest';

import { readStart } from './params.ts';
import { type Libraries, type RealDeps, realRunner } from './real.ts';

const REQUEST = readStart({
  url: 'https://example.com/',
  settings: {
    formFactor: 'mobile',
    screenEmulation: { width: 375, height: 800, deviceScaleFactor: 2, mobile: true },
    throttlingMethod: 'simulate',
  },
});

const REPORT = { report: ['{}', '<html></html>'], lhr: { finalDisplayedUrl: 'https://example.com/', lighthouseVersion: '13.6.0' } };

const ROOT = mkdtempSync(join(tmpdir(), 'rb-lh-tree-test-'));
afterAll(() => {
  rmSync(ROOT, { recursive: true, force: true });
});

/** Browsers of `size` processes; `hang`: its graceful close never finishes by itself. */
function machine(size = 6, hang = false) {
  const alive = new Set<number>();
  const trees = new Map<number, number[]>();
  const pending = new Map<number, () => void>();
  const profiles: unknown[] = [];
  const kills: number[] = [];
  let next = 100;
  let starts = 0;
  const end = (main: number, proc: { exitCode: number | null }) => {
    (trees.get(main) ?? []).forEach((member) => alive.delete(member));
    proc.exitCode = 0;
    pending.get(main)?.();
  };
  const procs = new Map<number, { pid: number; exitCode: number | null; signalCode: string | null }>();
  const launch: Libraries['launch'] = async (options) => {
    const main = next;
    next += size;
    starts += 1;
    profiles.push(options.userDataDir);
    const tree = Array.from({ length: size }, (_, index) => main + index);
    tree.forEach((pid) => alive.add(pid));
    trees.set(main, tree);
    const proc = { pid: main, exitCode: null as number | null, signalCode: null };
    procs.set(main, proc);
    return {
      newPage: async () => ({}),
      process: () => proc,
      close: () =>
        new Promise<void>((resolve) => {
          pending.set(main, resolve);
          if (!hang) {
            end(main, proc);
          }
        }),
    };
  };
  const killTree: RealDeps['killTree'] = async (pid, isAlive) => {
    if (!isAlive()) {
      return 0;
    }
    kills.push(pid);
    const left = (trees.get(pid) ?? []).filter((member) => alive.has(member)).length;
    const proc = procs.get(pid);
    if (proc !== undefined) {
      end(pid, proc);
    }
    return left;
  };
  return { alive, launch, killTree, kills, profiles, starts: () => starts };
}

function runnerOn(fake: ReturnType<typeof machine>, lighthouse: Libraries['lighthouse'], extra: { lines?: string[]; opened?: string[] } = {}) {
  const libraries: Libraries = { desktop: { throttling: {}, emulatedUserAgent: '' }, launch: fake.launch, lighthouse };
  return realRunner({
    findBrowser: () => ({ path: 'C:\\chrome.exe', headless: true, source: 'installed' }),
    load: async () => libraries,
    killTree: fake.killTree,
    log: (line) => extra.lines?.push(line),
    profiles: { root: ROOT },
    dangling: { open: () => extra.opened?.push('open') },
    closeGraceMs: 20,
  });
}

describe('a Lighthouse browser ends with its run', () => {
  it('closes gracefully, kills nothing and leaves no process and no profile after two runs', async () => {
    const fake = machine();
    const lines: string[] = [];
    const runner = runnerOn(fake, async () => REPORT, { lines });
    await runner(REQUEST, new AbortController().signal);
    await runner(REQUEST, new AbortController().signal);
    expect(fake.starts()).toBe(2);
    expect([...fake.alive]).toEqual([]);
    expect(fake.kills).toEqual([]);
    expect(lines).toEqual([
      'lighthouse: browser started pid=100',
      'lighthouse: browser ended pid=100 (closed)',
      'lighthouse: browser started pid=106',
      'lighthouse: browser ended pid=106 (closed)',
    ]);
    expect(fake.profiles).toHaveLength(2);
    for (const profile of fake.profiles) {
      expect(typeof profile).toBe('string');
      expect(String(profile).startsWith(ROOT)).toBe(true);
      expect(existsSync(String(profile))).toBe(false);
    }
  });

  it('kills the tree of a browser that does not close within its grace', async () => {
    const fake = machine(6, true);
    const lines: string[] = [];
    const runner = runnerOn(fake, async () => REPORT, { lines });
    await runner(REQUEST, new AbortController().signal);
    expect([...fake.alive]).toEqual([]);
    expect(fake.kills).toEqual([100]);
    expect(lines[1]).toBe('lighthouse: browser ended pid=100 (killed, processes=6)');
    expect(existsSync(String(fake.profiles[0]))).toBe(false);
  });

  it('leaves no process after a failed run', async () => {
    const fake = machine();
    const runner = runnerOn(fake, () => Promise.reject(new Error('Lighthouse failed')));
    await expect(runner(REQUEST, new AbortController().signal)).rejects.toThrow('Lighthouse failed');
    expect([...fake.alive]).toEqual([]);
  });

  it("leaves no process after a cancelled run and expects Lighthouse's dangling rejection", async () => {
    const fake = machine();
    const controller = new AbortController();
    const opened: string[] = [];
    const runner = runnerOn(
      fake,
      () => {
        controller.abort();
        return Promise.reject(new Error('Target closed'));
      },
      { opened },
    );
    await expect(runner(REQUEST, controller.signal)).rejects.toThrow('Target closed');
    expect([...fake.alive]).toEqual([]);
    expect(opened).toEqual(['open']);
  });

  it('runs no Lighthouse in a browser whose run was cancelled while it started', async () => {
    const fake = machine();
    const controller = new AbortController();
    let measured = false;
    const launch = fake.launch;
    fake.launch = async (options) => {
      const launched = await launch(options);
      controller.abort();
      return launched;
    };
    const opened: string[] = [];
    const runner = runnerOn(
      fake,
      async () => {
        measured = true;
        return REPORT;
      },
      { opened },
    );
    await expect(runner(REQUEST, controller.signal)).rejects.toThrow();
    expect(measured).toBe(false);
    expect(opened).toEqual([]);
    expect([...fake.alive]).toEqual([]);
  });

  it('removes the profile of a browser that did not start', async () => {
    const fake = machine();
    fake.launch = async (options) => {
      fake.profiles.push(options.userDataDir);
      throw new Error('no browser');
    };
    const runner = runnerOn(fake, async () => REPORT);
    await expect(runner(REQUEST, new AbortController().signal)).rejects.toThrow('no browser');
    expect(existsSync(String(fake.profiles[0]))).toBe(false);
  });
});
