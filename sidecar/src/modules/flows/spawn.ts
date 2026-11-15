// `flows.run` (RB-040-07): one flow worker process per run (`worker.ts`), so a flow - project
// code - never runs inside the sidecar itself. The worker runs under Node's permission model:
//
// - reads: the sidecar's own folder (its code and node_modules), the project, the run's folder;
// - writes: only the run's folder `.rbrowser/out/flows/<runId>` (screenshots; its `.tmp` is the
//   temp folder Playwright needs and is removed afterwards);
// - no child processes, worker threads, native addons, WASI or inspector;
// - network: allowed (`--allow-net` on Node 25+; older Node has no network permission) - the
//   flow drives the views over the CDP endpoints and pages load what they load.
//
// The environment is not the sidecar's: only `SystemRoot` (Windows needs it for sockets) and the
// temp folder pass. The worker gets the job on stdin; its status lines become `$/progress`, and
// each view's result line is kept as it comes. A worker that outlives its budget (`runBudgetMs`)
// by `graceMs` is killed - the views it reported keep their results, the others time out -, and
// so is one whose request rBrowser cancels.

import { ErrorCode, RpcError } from '../../rpc.ts';
import type { CallContext, Router } from '../../router.ts';
import { LineSplitter } from '../../framing.ts';
import type { ViewResult, ViewStatus } from './result.ts';
import { checkParams, type RunParams } from './params.ts';

/** The longest line read from a worker (a result with stacks of many views). */
const MAX_WORKER_LINE = 8 * 1024 * 1024;

/** What `flows.run` needs of a child process. */
export interface WorkerChild {
  stdin: NodeJS.WritableStream;
  stdout: NodeJS.ReadableStream;
  stderr: NodeJS.ReadableStream;
  kill(): boolean;
  on(event: 'close', listener: (code: number | null) => void): this;
  on(event: 'error', listener: (error: Error) => void): this;
}

export interface FlowsDeps {
  /** The Node that runs the sidecar (`process.execPath`). */
  nodePath: string;
  workerScript: string;
  /** The sidecar's folder: `dist/` and `node_modules/`. */
  sidecarDir: string;
  /** Whether this Node knows `--allow-net`. */
  netFlag: boolean;
  /** The sidecar's environment, from which the worker's is picked. */
  baseEnv: Record<string, string | undefined>;
  /** How long past the flow's timeout the worker may take before it is killed. */
  graceMs: number;
  /** What each round of views may take past its timeout (screenshot, close). */
  viewOverheadMs: number;
  spawn(args: string[], env: Record<string, string>): WorkerChild;
  makeDir(path: string): Promise<void>;
  removeDir(path: string): Promise<void>;
}

/**
 * How long all views of `job` may take: per round of `parallel` views one timeout and
 * `overheadMs` after it (the failure screenshot and the close, `VIEW_OVERHEAD_MS`).
 */
export function runBudgetMs(job: Pick<RunParams, 'views' | 'parallel' | 'timeoutMs'>, overheadMs: number): number {
  const lanes = job.parallel > 0 ? Math.min(job.parallel, job.views.length) : job.views.length;
  return Math.ceil(job.views.length / Math.max(lanes, 1)) * (job.timeoutMs + overheadMs);
}

/** The Node arguments of a worker for `projectRoot` writing into `outDir`. */
export function nodeArgs(
  deps: Pick<FlowsDeps, 'sidecarDir' | 'workerScript' | 'netFlag'>,
  projectRoot: string,
  outDir: string,
): string[] {
  return [
    '--permission',
    `--allow-fs-read=${deps.sidecarDir}`,
    `--allow-fs-read=${projectRoot}`,
    `--allow-fs-read=${outDir}`,
    `--allow-fs-write=${outDir}`,
    ...(deps.netFlag ? ['--allow-net'] : []),
    '--disable-warning=ExperimentalWarning',
    deps.workerScript,
  ];
}

/** The worker's environment: `SystemRoot` if set, and the temp folder `tmp`. */
export function workerEnv(base: Record<string, string | undefined>, tmp: string): Record<string, string> {
  const env: Record<string, string> = {};
  const root = base.SystemRoot ?? base.SYSTEMROOT;
  if (root !== undefined) {
    env.SystemRoot = root;
  }
  return { ...env, TEMP: tmp, TMP: tmp, TMPDIR: tmp };
}

export function registerFlows(router: Router, deps: FlowsDeps): void {
  router.register('flows.run', (params, ctx) => run(deps, checkParams(params), ctx));
}

async function run(deps: FlowsDeps, params: RunParams, ctx: CallContext): Promise<unknown> {
  const { projectRoot, ...job } = params;
  const tmp = `${job.outDir}/.tmp`;
  await deps.makeDir(tmp);
  const child = deps.spawn(nodeArgs(deps, projectRoot, job.outDir), workerEnv(deps.baseEnv, tmp));
  try {
    const views = await collect(child, job, deps, ctx);
    return { runId: job.runId, views };
  } finally {
    await deps.removeDir(tmp).catch(() => undefined);
  }
}

function collect(
  child: WorkerChild,
  job: Omit<RunParams, 'projectRoot'>,
  deps: Pick<FlowsDeps, 'graceMs' | 'viewOverheadMs'>,
  ctx: CallContext,
): Promise<ViewResult[]> {
  const budget = runBudgetMs(job, deps.viewOverheadMs);
  return new Promise((resolve, reject) => {
    let result: ViewResult[] | null = null;
    // Each view as the worker reported it on its own: kept when the worker is killed later.
    const early = new Map<number, ViewResult>();
    let failure: string | null = null;
    let ending: ViewStatus | null = null;
    const lines = new LineSplitter(
      MAX_WORKER_LINE,
      (line) => {
        const event = parse(line);
        if (event?.type === 'status') {
          ctx.progress({ runId: job.runId, viewId: event.viewId, status: event.status });
        } else if (event?.type === 'view' && isViewResult(event.view)) {
          early.set(event.view.viewId, event.view);
        } else if (event?.type === 'result' && Array.isArray(event.views)) {
          result = event.views as ViewResult[];
        } else if (event?.type === 'error') {
          failure = String(event.message);
        }
      },
      () => {},
    );
    const timer = setTimeout(() => {
      ending = 'timedOut';
      child.kill();
    }, budget + deps.graceMs);
    const onAbort = () => child.kill();
    ctx.signal.addEventListener('abort', onAbort, { once: true });
    child.stdout.on('data', (chunk: Buffer) => lines.push(chunk));
    // The worker's stderr (and the flow's console) is rBrowser's log; rb-sidecar masks it.
    child.stderr.on('data', (chunk: Buffer) => process.stderr.write(chunk));
    const finish = (code: number | null, error?: Error) => {
      clearTimeout(timer);
      ctx.signal.removeEventListener('abort', onAbort);
      if (ctx.signal.aborted) {
        reject(new RpcError(ErrorCode.RequestCancelled, 'cancelled'));
        return;
      }
      const reported: ViewResult[] = result ?? [...early.values()];
      const message =
        ending === 'timedOut'
          ? `the run took longer than ${budget} ms and its worker was stopped`
          : result !== null
            ? 'the flow worker reported nothing for this view'
            : (failure ?? error?.message ?? `the flow worker ended with ${String(code)} before it reported`);
      resolve(job.views.map((view) => reported.find((r) => r.viewId === view.id) ?? missing(view, ending, message, job.flow.file)));
    };
    child.on('error', (error) => finish(null, error));
    child.on('close', (code) => finish(code));
    child.stdin.end(`${JSON.stringify(job)}\n`);
  });
}

function missing(
  view: RunParams['views'][number],
  ending: ViewStatus | null,
  message: string,
  file: string,
): ViewResult {
  return {
    viewId: view.id,
    label: view.label,
    breakpoint: view.breakpoint,
    engine: view.engine,
    status: ending ?? 'failed',
    durationMs: 0,
    steps: [],
    error: { message, stack: '', step: null, line: null, column: null, file },
    screenshot: null,
  };
}

function isViewResult(value: unknown): value is ViewResult {
  return typeof value === 'object' && value !== null && typeof (value as { viewId?: unknown }).viewId === 'number';
}

function parse(line: string): Record<string, unknown> | null {
  try {
    const value: unknown = JSON.parse(line);
    return typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}
