import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';

import { describe, expect, it } from 'vitest';

import { ErrorCode, RpcError } from '../../rpc.ts';
import { Router } from '../../router.ts';
import { realFlowsDeps } from './real.ts';
import { CLOSE_TIMEOUT_MS, SCREENSHOT_TIMEOUT_MS, VIEW_OVERHEAD_MS } from './result.ts';
import { type FlowsDeps, type WorkerChild, nodeArgs, registerFlows, runBudgetMs, workerEnv } from './spawn.ts';

const TOKEN = 'be5'.repeat(22);

function params(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    runId: '20261111-101500-a1b2',
    flow: { name: 'smoke', source: 'await page.goto(env);\n', file: '/site/.rbrowser/flows/smoke.ts' },
    views: [1, 2].map((id) => ({
      id,
      label: `V${id}`,
      width: 400,
      height: 800,
      engine: 'chromium',
      breakpoint: null,
      endpoint: { url: `http://127.0.0.1:${9000 + id}`, headers: { Authorization: `Bearer ${TOKEN}` } },
    })),
    env: null,
    timeoutMs: 1000,
    parallel: 0,
    screenshotOnFailure: true,
    outDir: '/site/.rbrowser/out/flows/20261111-101500-a1b2',
    projectRoot: '/site',
    ...overrides,
  };
}

class FakeChild extends EventEmitter implements WorkerChild {
  stdin = new PassThrough();
  stdout = new PassThrough();
  stderr = new PassThrough();
  killed = false;
  written = '';

  constructor() {
    super();
    this.stdin.on('data', (chunk: Buffer) => (this.written += chunk.toString()));
  }

  kill(): boolean {
    this.killed = true;
    setImmediate(() => this.emit('close', null));
    return true;
  }

  line(value: unknown): void {
    this.stdout.write(`${JSON.stringify(value)}\n`);
  }
}

function setup(script: (child: FakeChild) => void, overrides: Partial<FlowsDeps> = {}) {
  const children: { args: string[]; env: Record<string, string>; child: FakeChild }[] = [];
  const removed: string[] = [];
  const made: string[] = [];
  const deps: FlowsDeps = {
    nodePath: '/node',
    workerScript: '/sidecar/dist/flow-worker.js',
    sidecarDir: '/sidecar',
    netFlag: true,
    baseEnv: { SystemRoot: 'C:\\Windows', PATH: '/bin', SECRET_THING: 'x' },
    graceMs: 50,
    viewOverheadMs: 0,
    spawn(args, env) {
      const child = new FakeChild();
      children.push({ args, env, child });
      setImmediate(() => script(child));
      return child;
    },
    makeDir: async (path) => {
      made.push(path);
    },
    removeDir: async (path) => {
      removed.push(path);
    },
    ...overrides,
  };
  const router = new Router();
  registerFlows(router, deps);
  const progress: Record<string, unknown>[] = [];
  const controller = new AbortController();
  const call = (p: unknown) =>
    router.call('flows.run', p, { signal: controller.signal, progress: (x) => progress.push(x) });
  return { call, children, removed, made, progress, controller };
}

describe('flows.run', () => {
  it('starts the worker under the permission model with only the project and the run folder', async () => {
    const { call, children, removed, made } = setup((child) => {
      child.line({ type: 'result', views: [] });
      child.emit('close', 0);
    });
    await call(params());
    const [{ args, env, child }] = children as [(typeof children)[0]];
    expect(args).toEqual(nodeArgs(
      { sidecarDir: '/sidecar', workerScript: '/sidecar/dist/flow-worker.js', netFlag: true },
      '/site',
      '/site/.rbrowser/out/flows/20261111-101500-a1b2',
    ));
    expect(args).toContain('--permission');
    expect(args).toContain('--allow-fs-read=/site');
    expect(args).toContain('--allow-fs-write=/site/.rbrowser/out/flows/20261111-101500-a1b2');
    expect(args).toContain('--allow-net');
    expect(args.some((a) => a.startsWith('--allow-child-process') || a.startsWith('--allow-worker'))).toBe(false);
    expect(env).toEqual(workerEnv({ SystemRoot: 'C:\\Windows' }, '/site/.rbrowser/out/flows/20261111-101500-a1b2/.tmp'));
    expect(env.SECRET_THING).toBeUndefined();
    expect(env.PATH).toBeUndefined();
    const job = JSON.parse(child.written) as Record<string, unknown>;
    expect(job.projectRoot).toBeUndefined();
    expect(job.runId).toBe('20261111-101500-a1b2');
    expect(made).toEqual(['/site/.rbrowser/out/flows/20261111-101500-a1b2/.tmp']);
    expect(removed).toEqual(made);
  });

  it('forwards the status of each view and answers the results', async () => {
    const views = [
      { viewId: 2, status: 'passed' },
      { viewId: 1, status: 'failed' },
    ];
    const { call, progress } = setup((child) => {
      child.line({ type: 'status', viewId: 1, status: 'running' });
      child.line({ type: 'result', views });
      child.emit('close', 0);
    });
    await expect(call(params())).resolves.toEqual({ runId: '20261111-101500-a1b2', views: [views[1], views[0]] });
    expect(progress).toEqual([{ runId: '20261111-101500-a1b2', viewId: 1, status: 'running' }]);
  });

  it('kills a worker that outlives the timeout and fails what it did not report', async () => {
    const { call, children } = setup(() => {});
    const answer = (await call(params({ timeoutMs: 20 }))) as { views: { viewId: number; status: string }[] };
    expect(children[0]?.child.killed).toBe(true);
    expect(answer.views.map((v) => [v.viewId, v.status])).toEqual([
      [1, 'timedOut'],
      [2, 'timedOut'],
    ]);
  });

  it('keeps the views a killed worker reported and times out only the rest', async () => {
    const reported = { viewId: 1, label: 'V1', status: 'failed', error: { message: 'expect failed' } };
    const { call, children } = setup((child) => child.line({ type: 'view', view: reported }));
    const answer = (await call(params({ timeoutMs: 20 }))) as { views: { viewId: number; status: string }[] };
    expect(children[0]?.child.killed).toBe(true);
    expect(answer.views[0]).toEqual(reported);
    expect(answer.views.map((v) => [v.viewId, v.status])).toEqual([
      [1, 'failed'],
      [2, 'timedOut'],
    ]);
  });

  it('keeps the views a worker reported before it crashed', async () => {
    const reported = { viewId: 2, label: 'V2', status: 'passed' };
    const { call } = setup((child) => {
      child.line({ type: 'view', view: reported });
      child.emit('close', 1);
    });
    const answer = (await call(params())) as { views: { viewId: number; status: string }[] };
    expect(answer.views.map((v) => [v.viewId, v.status])).toEqual([
      [1, 'failed'],
      [2, 'passed'],
    ]);
  });

  it('fails every view when the worker ends without a result', async () => {
    const { call } = setup((child) => {
      child.stderr.write('Error: boom\n');
      child.emit('close', 1);
    });
    const answer = (await call(params())) as { views: { status: string; error: { message: string } }[] };
    expect(answer.views.every((v) => v.status === 'failed')).toBe(true);
    expect(answer.views[0]?.error.message).toContain('the flow worker ended with 1');
  });

  it('kills the worker when rBrowser cancels', async () => {
    const { call, children, controller } = setup(() => controller.abort());
    await expect(call(params())).rejects.toMatchObject({ code: ErrorCode.RequestCancelled });
    expect(children[0]?.child.killed).toBe(true);
  });

  it('refuses bad parameters', async () => {
    const { call } = setup(() => {});
    for (const bad of [
      { runId: '../x' },
      { views: [] },
      { timeoutMs: 0 },
      { outDir: 'relative/out' },
      { flow: { name: 'a', source: 1, file: 'a.ts' } },
      { views: [{ id: 1, label: 'x', width: 1, height: 1, engine: 'chromium', breakpoint: null }] },
    ]) {
      await expect(call(params(bad))).rejects.toBeInstanceOf(RpcError);
    }
  });
});

describe('runBudgetMs', () => {
  it('gives each round of parallel views one timeout and the time after it', () => {
    const views = new Array(5).fill(null);
    expect(runBudgetMs({ views, parallel: 0, timeoutMs: 100 }, 10)).toBe(110);
    expect(runBudgetMs({ views, parallel: 2, timeoutMs: 100 }, 10)).toBe(330);
    expect(runBudgetMs({ views, parallel: 9, timeoutMs: 100 }, 0)).toBe(100);
  });

  it('leaves room for a failure screenshot and the close after each timeout', () => {
    expect(VIEW_OVERHEAD_MS).toBe(SCREENSHOT_TIMEOUT_MS + CLOSE_TIMEOUT_MS);
    expect(realFlowsDeps(import.meta.url).viewOverheadMs).toBe(VIEW_OVERHEAD_MS);
  });
});

describe('workerEnv', () => {
  it('keeps only what Windows needs and points the temp folder at the run', () => {
    expect(workerEnv({ SystemRoot: 'C:\\Windows', HOME: '/home/x', NODE_OPTIONS: '--require x' }, '/t')).toEqual({
      SystemRoot: 'C:\\Windows',
      TEMP: '/t',
      TMP: '/t',
      TMPDIR: '/t',
    });
  });
});
