// The flow worker (RB-040-07): a Node process of its own per run, started by the sidecar's
// `flows.run` under Node's permission model (`spawn.ts`). It reads one job line on stdin,
// connects to each view's endpoint with Playwright's `connectOverCDP`, runs the flow with
// `expect` of @playwright/test and writes one JSON line per event on stdout:
// `{"type":"status","viewId":1,"status":"running"}`, `{"type":"view","view":{...}}` as each view
// ends (kept by the sidecar if this worker is stopped), and finally `{"type":"result","views":[...]}`.
// Console output of the flow goes to stderr, which ends in rBrowser's log.

import { writeFile } from 'node:fs/promises';

import { connectOptions } from './connect.ts';
import type { RunnerDeps } from './runner.ts';
import { runFlow } from './runner.ts';
import { type RunJob, redact, secretsOf } from './result.ts';


function emit(line: Record<string, unknown>): Promise<void> {
  return new Promise((resolve) => {
    process.stdout.write(`${JSON.stringify(line)}\n`, () => resolve());
  });
}

async function readJob(): Promise<RunJob> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk as Buffer);
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8')) as RunJob;
}

async function deps(): Promise<RunnerDeps> {
  const { chromium } = await import('playwright-core');
  const { expect } = await import('@playwright/test');
  return {
    expect,
    now: () => Date.now(),
    async connect(view, timeoutMs) {
      const browser = await chromium.connectOverCDP(view.endpoint.url, connectOptions(view.endpoint, timeoutMs));
      const page = browser.contexts()[0]?.pages()[0];
      if (page === undefined) {
        await browser.close().catch(() => undefined);
        throw new Error(`view ${view.id} shows no page`);
      }
      return { page, close: () => browser.close() };
    },
    writeFile: (path, data) => writeFile(path, data),
    onStatus: (viewId, status) => void emit({ type: 'status', viewId, status }),
    onView: (view) => void emit({ type: 'view', view }),
  };
}

async function main(): Promise<void> {
  const toStderr = (...args: unknown[]) => console.error(...args);
  console.log = toStderr;
  console.info = toStderr;
  console.debug = toStderr;
  const job = await readJob();
  try {
    await emit({ type: 'result', views: await runFlow(job, await deps()) });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    // Where it failed goes to rBrowser's log (stderr), masked like the message.
    if (error instanceof Error && error.stack !== undefined) {
      process.stderr.write(`${redact(error.stack, secretsOf(job))}\n`);
    }
    await emit({ type: 'error', message: redact(message, secretsOf(job)) });
  }
}

await main();
// Every line is flushed; pages a timed-out flow still waits on must not keep the process alive.
process.exit(0);
