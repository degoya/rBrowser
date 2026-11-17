// Runs one flow in every view of a job (RB-040-07), inside the flow worker: the flow compiled
// once, then per view a connection to that view's page, the flow with its globals, a timeout,
// and on failure a screenshot into the run's folder. At most `parallel` views run at once (0:
// all). Nothing here knows Playwright: the worker passes `connect` and `expect`.

import { join } from 'node:path';

import { type FlowFunction, FlowSyntaxError, compileFlow, fileColumn } from './compile.ts';
import { createSteps, flowArguments } from './globals.ts';
import {
  CLOSE_TIMEOUT_MS,
  DEFAULT_ACTION_TIMEOUT_MS,
  type ErrorInfo,
  type JobView,
  type RunJob,
  SCREENSHOT_TIMEOUT_MS,
  type ViewResult,
  type ViewStatus,
  locate,
  redact,
  screenshotName,
  secretsOf,
} from './result.ts';

export { SCREENSHOT_TIMEOUT_MS } from './result.ts';

/** A view's page and how to let go of it. */
export interface Connected {
  page: unknown;
  close(): Promise<void>;
}

export interface RunnerDeps {
  connect(view: JobView, timeoutMs: number): Promise<Connected>;
  expect: unknown;
  now(): number;
  writeFile(path: string, data: Buffer): Promise<void>;
  /** A view started (`running`) or ended (its status). */
  onStatus(viewId: number, status: ViewStatus | 'running'): void;
  /** A view's result, as soon as it ends - kept even when the worker is stopped later. */
  onView(result: ViewResult): void;
}

export interface RunOptions {
  /** How long a close may take before the view goes on without it. */
  closeTimeoutMs?: number;
}

class TimedOut extends Error {}

export async function runFlow(job: RunJob, deps: RunnerDeps, options: RunOptions = {}): Promise<ViewResult[]> {
  const closeTimeoutMs = options.closeTimeoutMs ?? CLOSE_TIMEOUT_MS;
  const secrets = secretsOf(job);
  let fn: FlowFunction;
  try {
    fn = compileFlow(job.flow.source, job.flow.file);
  } catch (error) {
    const info = errorInfo(error, job.flow.file, null, secrets);
    if (error instanceof FlowSyntaxError) {
      info.line = error.line;
      info.column = null;
    }
    const failed = job.views.map((view) => base(view, 'failed', 0, info));
    for (const result of failed) {
      deps.onView(result);
    }
    return failed;
  }
  const results = new Array<ViewResult>(job.views.length);
  const limit = job.parallel > 0 ? job.parallel : job.views.length;
  let next = 0;
  const lane = async (): Promise<void> => {
    while (next < job.views.length) {
      const index = next;
      next += 1;
      const view = job.views[index] as JobView;
      deps.onStatus(view.id, 'running');
      const result = await runView(job, view, fn, deps, secrets, closeTimeoutMs);
      results[index] = result;
      deps.onView(result);
      deps.onStatus(view.id, result.status);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, job.views.length) }, lane));
  return results;
}

async function runView(
  job: RunJob,
  view: JobView,
  fn: FlowFunction,
  deps: RunnerDeps,
  secrets: string[],
  closeTimeoutMs: number,
): Promise<ViewResult> {
  const start = deps.now();
  const steps = createSteps(deps.now);
  let connected: Connected | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new TimedOut(`the flow ran longer than ${job.timeoutMs} ms`)), job.timeoutMs);
  });
  let status: ViewStatus = 'passed';
  let error: ErrorInfo | null = null;
  let screenshot: string | null = null;
  try {
    connected = await Promise.race([deps.connect(view, job.timeoutMs), deadline]);
    const page = connected.page;
    limitActions(page, Math.min(job.actionTimeoutMs ?? DEFAULT_ACTION_TIMEOUT_MS, job.timeoutMs), job.timeoutMs);
    await Promise.race([fn(...flowArguments(page, strip(view), deps.expect, job.env, steps)), deadline]);
  } catch (caught) {
    status = caught instanceof TimedOut ? 'timedOut' : 'failed';
    error = errorInfo(caught, job.flow.file, steps.stepOf(caught), secrets);
    if (caught instanceof TimedOut) {
      steps.abort();
    }
    if (connected !== null && job.screenshotOnFailure) {
      screenshot = await takeScreenshot(connected.page, join(job.outDir, screenshotName(view)), deps);
    }
  } finally {
    clearTimeout(timer);
    if (connected !== null) {
      await closeWithin(connected, closeTimeoutMs);
    }
  }
  return { ...base(view, status, deps.now() - start, error), steps: steps.done, screenshot };
}

/**
 * Each action of `page` (click, fill, hover, a wait for an element) fails after `actionMs` with
 * its locator, instead of waiting out the flow's time; a navigation may take `navigationMs`.
 */
function limitActions(page: unknown, actionMs: number, navigationMs: number): void {
  const limits = page as {
    setDefaultTimeout?: (ms: number) => void;
    setDefaultNavigationTimeout?: (ms: number) => void;
  };
  limits.setDefaultTimeout?.(actionMs);
  limits.setDefaultNavigationTimeout?.(navigationMs);
}

/** Lets go of `connected`, waiting at most `ms` for it. */
async function closeWithin(connected: Connected, ms: number): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const limit = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, ms);
  });
  try {
    await Promise.race([connected.close().catch(() => undefined), limit]);
  } finally {
    clearTimeout(timer);
  }
}

async function takeScreenshot(page: unknown, path: string, deps: RunnerDeps): Promise<string | null> {
  const shoot = (page as { screenshot?: (options: object) => Promise<Buffer> }).screenshot;
  if (typeof shoot !== 'function') {
    return null;
  }
  try {
    const data = await shoot.call(page, { timeout: SCREENSHOT_TIMEOUT_MS });
    await deps.writeFile(path, data);
    return path.split(/[\\/]/).at(-1) ?? null;
  } catch {
    return null;
  }
}

function strip(view: JobView) {
  const { endpoint: _endpoint, ...facts } = view;
  return facts;
}

function base(view: JobView, status: ViewStatus, durationMs: number, error: ErrorInfo | null): ViewResult {
  return {
    viewId: view.id,
    label: view.label,
    breakpoint: view.breakpoint,
    engine: view.engine,
    status,
    durationMs,
    steps: [],
    error,
    screenshot: null,
  };
}

function errorInfo(caught: unknown, file: string, step: string | null, secrets: string[]): ErrorInfo {
  const message = redact(caught instanceof Error ? caught.message : String(caught), secrets);
  const stack = redact(caught instanceof Error ? (caught.stack ?? '') : '', secrets);
  const where = caught instanceof TimedOut ? null : locate(stack, file);
  return {
    message,
    stack,
    step,
    line: where?.line ?? null,
    column: where === null ? null : fileColumn(where.line, where.column),
    file,
  };
}
