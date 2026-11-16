import { describe, expect, it } from 'vitest';

import { type Connected, type RunnerDeps, runFlow } from './runner.ts';
import type { JobView, RunJob, ViewResult } from './result.ts';

const TOKEN = 'c0ffee'.repeat(11);
const FILE = '/home/dev/site/.rbrowser/flows/smoke.ts';

function view(id: number, label: string): JobView {
  return {
    id,
    label,
    width: 300 + id,
    height: 600,
    engine: 'chromium',
    breakpoint: label.toLowerCase(),
    endpoint: { url: `http://127.0.0.1:${9000 + id}`, headers: { Authorization: `Bearer ${TOKEN}` } },
  };
}

function job(source: string, overrides: Partial<RunJob> = {}): RunJob {
  return {
    runId: '20261111-000000-abcd',
    flow: { name: 'smoke', source, file: FILE },
    views: [view(1, 'Mobile'), view(2, 'Tablet'), view(3, 'Desktop')],
    env: 'https://a.test',
    timeoutMs: 2000,
    parallel: 0,
    screenshotOnFailure: true,
    outDir: '/out/run',
    ...overrides,
  };
}

interface FakePage {
  view: number;
  visited: string[];
  screenshot(): Promise<Buffer>;
  wait(ms: number): Promise<void>;
}

/** A fake connection per view; records concurrency, closes and screenshots. */
function deps(options: { failConnect?: number; screenshotFails?: boolean; closeHangs?: boolean } = {}) {
  const state = {
    running: 0,
    max: 0,
    closed: [] as number[],
    written: [] as string[],
    status: [] as string[],
    reported: [] as ViewResult[],
  };
  const runner: RunnerDeps = {
    expect: (value: unknown) => ({
      toBe(expected: unknown) {
        if (value !== expected) {
          throw new Error(`expected ${String(value)} to be ${String(expected)}`);
        }
      },
    }),
    now: () => Date.now(),
    async connect(target: JobView): Promise<Connected> {
      if (target.id === options.failConnect) {
        throw new Error(`connect ECONNREFUSED with Bearer ${TOKEN}`);
      }
      state.running += 1;
      state.max = Math.max(state.max, state.running);
      const page: FakePage = {
        view: target.id,
        visited: [],
        screenshot: async () => {
          if (options.screenshotFails === true) {
            throw new Error('no screenshot');
          }
          return Buffer.from('png');
        },
        wait: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
      };
      return {
        page,
        close: async () => {
          state.running -= 1;
          state.closed.push(target.id);
          if (options.closeHangs === true) {
            await new Promise(() => undefined);
          }
        },
      };
    },
    async writeFile(path: string) {
      state.written.push(path);
    },
    onStatus: (viewId, status) => state.status.push(`${viewId}:${status}`),
    onView: (result) => state.reported.push(result),
  };
  return { runner, state };
}

const byView = (results: ViewResult[]) => new Map(results.map((r) => [r.viewId, r]));

describe('runFlow', () => {
  it('runs every view at once with its own page and view', async () => {
    const { runner, state } = deps();
    const results = await runFlow(
      job('await page.wait(30);\nexpect(page.view).toBe(view.id);\nawait step("done", () => 1);\n'),
      runner,
    );
    expect(results.map((r) => [r.viewId, r.status])).toEqual([
      [1, 'passed'],
      [2, 'passed'],
      [3, 'passed'],
    ]);
    expect(state.max).toBe(3);
    expect(state.closed.sort()).toEqual([1, 2, 3]);
    expect(results[0]?.steps.map((s) => s.name)).toEqual(['done']);
    expect(results[0]).toMatchObject({ label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', error: null, screenshot: null });
    expect(state.status).toContain('1:running');
    expect(state.status).toContain('3:passed');
  });

  it('reports each view as soon as it ends, before the run does', async () => {
    const { runner, state } = deps();
    const results = await runFlow(job('await page.wait(view.id === 1 ? 1 : 60);\n'), runner);
    expect(state.reported.map((r) => r.viewId)[0]).toBe(1);
    expect(state.reported.map((r) => r.viewId).sort()).toEqual([1, 2, 3]);
    expect(new Set(state.reported)).toEqual(new Set(results));
  });

  it('reports every view of a flow that does not compile', async () => {
    const { runner, state } = deps();
    await runFlow(job('const = ;\n'), runner);
    expect(state.reported.map((r) => [r.viewId, r.status])).toEqual([
      [1, 'failed'],
      [2, 'failed'],
      [3, 'failed'],
    ]);
  });

  it('does not wait for a close that hangs', async () => {
    const { runner } = deps({ closeHangs: true });
    // The close never resolves: the run returning at all is the proof.
    const results = await runFlow(job('expect(1).toBe(1);\n'), { ...runner }, { closeTimeoutMs: 30 });
    expect(results.every((r) => r.status === 'passed')).toBe(true);
  });

  it('runs at most `parallel` views at once', async () => {
    const { runner, state } = deps();
    await runFlow(job('await page.wait(20);\n', { parallel: 1 }), runner);
    expect(state.max).toBe(1);
  });

  it('fails only the view whose expect fails, with step, line and screenshot', async () => {
    const { runner, state } = deps();
    const source = [
      'await step("check width", async () => {',
      '  expect(view.id === 2 ? "wide" : "ok").toBe("ok");',
      '});',
    ].join('\n');
    const results = byView(await runFlow(job(source), runner));
    expect(results.get(1)?.status).toBe('passed');
    expect(results.get(3)?.status).toBe('passed');
    const failed = results.get(2);
    expect(failed?.status).toBe('failed');
    expect(failed?.error).toMatchObject({ step: 'check width', line: 2, file: FILE });
    expect(failed?.error?.message).toContain('expected wide to be ok');
    expect(failed?.screenshot).toBe('tablet-2.png');
    expect(state.written).toEqual(['/out/run/tablet-2.png']);
  });

  it('marks a view that runs too long as timedOut and takes its screenshot', async () => {
    const { runner } = deps();
    const results = byView(
      await runFlow(job('await step("slow", () => page.wait(view.id === 1 ? 500 : 1));\n', { timeoutMs: 100 }), runner),
    );
    expect(results.get(1)).toMatchObject({ status: 'timedOut', screenshot: 'mobile-1.png' });
    expect(results.get(1)?.error).toMatchObject({ step: 'slow' });
    expect(results.get(1)?.error?.message).toContain('100 ms');
    expect(results.get(2)?.status).toBe('passed');
  });

  it('fails a view it cannot reach without the token in the error', async () => {
    const { runner } = deps({ failConnect: 3 });
    const results = byView(await runFlow(job('expect(1).toBe(1);\n'), runner));
    expect(results.get(3)?.status).toBe('failed');
    expect(results.get(3)?.screenshot).toBeNull();
    expect(JSON.stringify(results.get(3))).not.toContain(TOKEN);
    expect(results.get(3)?.error?.message).toContain('<redacted>');
  });

  it('fails every view with the line of a syntax error and connects to none', async () => {
    const { runner, state } = deps();
    const results = await runFlow(job('const a = 1;\nconst = ;\n'), runner);
    expect(results.every((r) => r.status === 'failed' && r.error?.line === 2)).toBe(true);
    expect(state.max).toBe(0);
  });

  it('keeps a failed view failed when its screenshot fails, and takes none when off', async () => {
    const broken = deps({ screenshotFails: true });
    const one = byView(await runFlow(job('throw new Error("x");\n'), broken.runner));
    expect(one.get(1)).toMatchObject({ status: 'failed', screenshot: null });
    const off = deps();
    await runFlow(job('throw new Error("x");\n', { screenshotOnFailure: false }), off.runner);
    expect(off.state.written).toEqual([]);
  });
});
