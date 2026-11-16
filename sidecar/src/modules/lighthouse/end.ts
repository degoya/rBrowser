// The end of a run's browser (RB-080-67): puppeteer's graceful close first - the browser ends
// its own children and lets go of its profile - within `CLOSE_GRACE_MS`; a browser still running
// then is killed with its whole tree (tree.ts), only while Node knows its main process alive
// (review 3, 4). The run's profile folder is the runner's own and removed after the browser
// (`removeProfile`). The end is logged with the pid and, after a kill, the processes ended.

import { mkdtemp, rm } from 'node:fs/promises';
import { join } from 'node:path';

import { CLOSE_GRACE_MS, EXITED_WAIT_MS, PROFILE_RETRIES, PROFILE_RETRY_DELAY_MS } from './budget.ts';

/** Node's record of the browser's main process (puppeteer's `Browser.process()`). */
export interface BrowserProcess {
  pid?: number | undefined;
  exitCode: number | null;
  signalCode: string | null;
}

export interface Closable {
  close(): Promise<void>;
  process(): BrowserProcess | null;
}

export interface EndDeps {
  /** Kills the tree under a pid while `alive()` holds; how many processes ended (tree.ts). */
  killTree(pid: number, alive: () => boolean): Promise<number | null>;
  log(line: string): void;
  /** The grace of the graceful close; `CLOSE_GRACE_MS` unless a test sets it. */
  closeGraceMs?: number;
}

/** Whether `promise` settles within `ms`. */
async function within(promise: Promise<unknown>, ms: number): Promise<boolean> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<false>((resolve) => {
    timer = setTimeout(() => resolve(false), ms);
  });
  try {
    return await Promise.race([promise.then(() => true), late]);
  } finally {
    clearTimeout(timer);
  }
}

/** Ends `launched`: gracefully, else by killing its tree. */
export async function endBrowser(launched: Closable, deps: EndDeps): Promise<void> {
  const proc = launched.process();
  const pid = proc?.pid;
  const alive = () => proc !== null && proc.exitCode === null && proc.signalCode === null;
  const closed = launched.close().catch(() => {});
  if (await within(closed, deps.closeGraceMs ?? CLOSE_GRACE_MS)) {
    deps.log(`lighthouse: browser ended pid=${pid ?? '?'} (closed)`);
    return;
  }
  const killed = pid === undefined ? null : await deps.killTree(pid, alive).catch(() => null);
  await within(closed, EXITED_WAIT_MS);
  deps.log(`lighthouse: browser ended pid=${pid ?? '?'} (killed, processes=${killed ?? '?'})`);
}

/** A fresh profile folder for one run under `root`. */
export function makeProfile(root: string): Promise<string> {
  return mkdtemp(join(root, 'rbrowser-lighthouse-'));
}

/** Removes a run's profile folder; a failure is logged (without the path: it names the user). */
export async function removeProfile(dir: string, log: (line: string) => void): Promise<void> {
  try {
    await rm(dir, { recursive: true, force: true, maxRetries: PROFILE_RETRIES, retryDelay: PROFILE_RETRY_DELAY_MS });
  } catch (error) {
    log(`lighthouse: a browser profile could not be removed: ${(error as NodeJS.ErrnoException).code ?? 'error'}`);
  }
}
