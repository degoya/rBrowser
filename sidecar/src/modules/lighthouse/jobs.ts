// The Lighthouse jobs (RB-060-07): one run at a time - Lighthouse measures performance, and two
// runs side by side would slow each other down - in the order they were started. A run takes
// longer than one sidecar request may (`engines.nodeRpcTimeoutMs`, 30 s by default), so a caller
// starts a job, polls it (each poll waits at most `MAX_WAIT_MS`), fetches its reports in slices
// and releases it. A finished job is forgotten after `KEEP_MS` or when more than `MAX_KEPT` have
// finished; `cancel` ends the caller's running and queued jobs, `shutdown` everyone's.
//
// Every job belongs to the caller that started it (the plugin rBrowser names in `$caller`,
// params.ts): another caller can neither poll, read, release nor cancel it, and `status` shows it
// only as `busy` (review finding L4 of Charge 7).

import { ErrorCode, RpcError } from '../../rpc.ts';
import type { StartRequest } from './params.ts';

/** The module's own error codes (next to rpc.ts's `-3200x`). */
export const LighthouseCode = {
  /** No Chromium-based browser to run Lighthouse in (browser.ts). */
  NoBrowser: -32010,
  /** No job has this id (never started, released or forgotten). */
  UnknownJob: -32011,
  /** Too many jobs wait already. */
  Busy: -32012,
  /** The job has no report (yet). */
  NotFinished: -32013,
  /** Lighthouse failed. */
  RunFailed: -32014,
  /** The browser set in `engines.lighthouseBrowserPath` (`CHROME_PATH`) does not exist. */
  BrowserMissing: -32015,
} as const;

export class LighthouseError extends RpcError {}

/** What a finished run hands over. */
export interface RunOutput {
  json: string;
  html: string;
  finalUrl: string;
  lighthouseVersion: string;
}

/** Runs Lighthouse once; rejects when `signal` aborts. */
export type Runner = (request: StartRequest, signal: AbortSignal) => Promise<RunOutput>;

export interface JobsDeps {
  runner: Runner;
  now(): number;
  newId(): string;
}

export type JobState = 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

export interface PollAnswer {
  job: string;
  state: JobState;
  label: string;
  url: string;
  /** For a queued job: how many run or wait before it, the running one included. */
  position?: number;
  error?: { code: number; message: string };
  result?: { finalUrl: string; lighthouseVersion: string; jsonBytes: number; htmlBytes: number };
}

/** The most unfinished jobs. */
export const MAX_QUEUED = 32;
/** The most finished jobs kept for their reports. */
export const MAX_KEPT = 16;
/** How long a finished job is kept. */
export const KEEP_MS = 10 * 60 * 1000;
/** The most characters of one report slice: well below the 4 MiB of a sidecar answer even when
 * every character needs a JSON escape. */
export const SLICE_CHARS = 512 * 1024;
const MAX_MESSAGE = 500;

interface Job {
  id: string;
  owner: string;
  request: StartRequest;
  state: JobState;
  controller: AbortController;
  finished: Promise<void>;
  finish: () => void;
  finishedAt: number | null;
  error?: { code: number; message: string };
  output?: RunOutput;
}

function errorOf(error: unknown): { code: number; message: string } {
  const message = (error instanceof Error ? error.message : String(error)).slice(0, MAX_MESSAGE);
  return { code: error instanceof RpcError ? error.code : LighthouseCode.RunFailed, message };
}

/** The same answer for a job that does not exist and another caller's: nothing is told. */
function unknownJob(id: string): LighthouseError {
  return new LighthouseError(LighthouseCode.UnknownJob, `no Lighthouse job ${id}`);
}

function isHighSurrogate(code: number): boolean {
  return code >= 0xd800 && code <= 0xdbff;
}

export class Jobs {
  readonly #deps: JobsDeps;
  readonly #jobs = new Map<string, Job>();
  #running: Job | null = null;

  constructor(deps: JobsDeps) {
    this.#deps = deps;
  }

  /** Queues a run of `owner`; answers its job id. */
  start(request: StartRequest, owner = ''): string {
    this.#prune();
    const unfinished = [...this.#jobs.values()].filter((job) => job.finishedAt === null).length;
    if (unfinished >= MAX_QUEUED) {
      throw new LighthouseError(LighthouseCode.Busy, `${MAX_QUEUED} Lighthouse runs wait already`);
    }
    let finish = () => {};
    const finished = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const job: Job = {
      id: this.#deps.newId(),
      owner,
      request,
      state: 'queued',
      controller: new AbortController(),
      finished,
      finish,
      finishedAt: null,
    };
    this.#jobs.set(job.id, job);
    queueMicrotask(() => this.#pump());
    return job.id;
  }

  /** The job's state, once it has ended or after `waitMs`, whichever comes first. */
  async poll(id: string, waitMs: number, signal: AbortSignal, owner = ''): Promise<PollAnswer> {
    const job = this.#get(id, owner);
    if (job.finishedAt === null && waitMs > 0) {
      let timer: ReturnType<typeof setTimeout> | undefined;
      let onAbort = () => {};
      const waited = new Promise<void>((resolve) => {
        timer = setTimeout(resolve, waitMs);
        onAbort = () => resolve();
        signal.addEventListener('abort', onAbort, { once: true });
      });
      await Promise.race([job.finished, waited]);
      clearTimeout(timer);
      signal.removeEventListener('abort', onAbort);
    }
    return this.#answer(job);
  }

  /** A slice of a finished job's report, from `offset`; `next` is null after the last. */
  report(
    id: string,
    format: 'json' | 'html',
    offset: number,
    slice = SLICE_CHARS,
    owner = '',
  ): { text: string; offset: number; total: number; next: number | null } {
    const job = this.#get(id, owner);
    if (job.output === undefined) {
      throw new LighthouseError(LighthouseCode.NotFinished, `job ${id} has no report`);
    }
    const text = format === 'json' ? job.output.json : job.output.html;
    if (offset > text.length) {
      throw new RpcError(ErrorCode.InvalidParams, 'offset is past the end of the report');
    }
    let end = Math.min(text.length, offset + slice);
    if (end < text.length && end > offset + 1 && isHighSurrogate(text.charCodeAt(end - 1))) {
      end -= 1;
    }
    return { text: text.slice(offset, end), offset, total: text.length, next: end < text.length ? end : null };
  }

  /** Forgets a job of `owner` (cancelling it first if it has not ended). */
  release(id: string, owner = ''): boolean {
    const job = this.#jobs.get(id);
    if (job === undefined) {
      return false;
    }
    if (job.owner !== owner) {
      throw unknownJob(id);
    }
    if (job.finishedAt === null) {
      this.#cancelJob(job);
    }
    this.#jobs.delete(id);
    return true;
  }

  /** Cancels `owner`'s running job and its queued ones; answers how many. */
  cancel(owner = ''): number {
    const open = [...this.#jobs.values()].filter((job) => job.finishedAt === null && job.owner === owner);
    open.forEach((job) => this.#cancelJob(job));
    return open.length;
  }

  /**
   * The sidecar ends: cancels every caller's jobs and resolves once the running one has ended,
   * its browser with it (RB-080-67: `cancel()` reached only the caller '').
   */
  async shutdown(): Promise<void> {
    const running = this.#running;
    [...this.#jobs.values()].filter((job) => job.finishedAt === null).forEach((job) => this.#cancelJob(job));
    await running?.finished;
  }

  /** `owner`'s running job and how many of its jobs wait; `busy` while any job runs. */
  status(owner = ''): { running: { job: string; label: string; url: string } | null; queued: number; busy: boolean } {
    this.#prune();
    const running = this.#running;
    const own = running !== null && running.owner === owner ? running : null;
    return {
      running: own === null ? null : { job: own.id, label: own.request.label, url: own.request.url },
      queued: [...this.#jobs.values()].filter((job) => job.state === 'queued' && job.owner === owner).length,
      busy: running !== null,
    };
  }

  #get(id: string, owner: string): Job {
    this.#prune();
    const job = this.#jobs.get(id);
    if (job === undefined || job.owner !== owner) {
      throw unknownJob(id);
    }
    return job;
  }

  #answer(job: Job): PollAnswer {
    const answer: PollAnswer = { job: job.id, state: job.state, label: job.request.label, url: job.request.url };
    if (job.state === 'queued') {
      const queued = [...this.#jobs.values()].filter((other) => other.state === 'queued');
      answer.position = queued.indexOf(job) + (this.#running === null ? 0 : 1);
    }
    if (job.error !== undefined) {
      answer.error = job.error;
    }
    if (job.output !== undefined) {
      answer.result = {
        finalUrl: job.output.finalUrl,
        lighthouseVersion: job.output.lighthouseVersion,
        jsonBytes: Buffer.byteLength(job.output.json),
        htmlBytes: Buffer.byteLength(job.output.html),
      };
    }
    return answer;
  }

  #end(job: Job, state: JobState): void {
    job.state = state;
    job.finishedAt = this.#deps.now();
    job.finish();
  }

  #cancelJob(job: Job): void {
    if (job.state === 'running') {
      // The run's rejection ends it as cancelled (#run).
      job.controller.abort();
    } else if (job.state === 'queued') {
      this.#end(job, 'cancelled');
    }
  }

  #pump(): void {
    if (this.#running !== null) {
      return;
    }
    const next = [...this.#jobs.values()].find((job) => job.state === 'queued');
    if (next !== undefined) {
      void this.#run(next);
    }
  }

  async #run(job: Job): Promise<void> {
    this.#running = job;
    job.state = 'running';
    try {
      job.output = await this.#deps.runner(job.request, job.controller.signal);
      this.#end(job, job.controller.signal.aborted ? 'cancelled' : 'done');
    } catch (error) {
      if (job.controller.signal.aborted) {
        this.#end(job, 'cancelled');
      } else {
        job.error = errorOf(error);
        this.#end(job, 'failed');
      }
    } finally {
      this.#running = null;
      this.#pump();
    }
  }

  #prune(): void {
    const now = this.#deps.now();
    const finished = [...this.#jobs.values()]
      .filter((job): job is Job & { finishedAt: number } => job.finishedAt !== null)
      .sort((a, b) => a.finishedAt - b.finishedAt);
    finished.forEach((job, index) => {
      if (now - job.finishedAt > KEEP_MS || finished.length - index > MAX_KEPT) {
        this.#jobs.delete(job.id);
      }
    });
  }
}
