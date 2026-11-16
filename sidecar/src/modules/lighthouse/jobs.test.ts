import { describe, expect, it } from 'vitest';

import { RpcError } from '../../rpc.ts';
import { type RunOutput, type Runner, Jobs, LighthouseError, MAX_QUEUED } from './jobs.ts';
import { readStart } from './params.ts';

const REQUEST = readStart({
  url: 'http://localhost:5173/',
  label: 'mobile',
  settings: {
    formFactor: 'mobile',
    screenEmulation: { width: 375, height: 812, deviceScaleFactor: 3, mobile: true },
    throttlingMethod: 'simulate',
  },
});

const OUTPUT: RunOutput = {
  json: '{"lighthouseVersion":"13.6.0"}',
  html: '<html>report</html>',
  finalUrl: 'http://localhost:5173/',
  lighthouseVersion: '13.6.0',
};

/** A runner whose runs end when the test says so. */
function manual() {
  const runs: { url: string; signal: AbortSignal; finish: (output: RunOutput) => void; fail: (error: Error) => void }[] = [];
  const runner: Runner = (request, signal) =>
    new Promise<RunOutput>((resolve, reject) => {
      runs.push({ url: request.url, signal, finish: resolve, fail: reject });
      signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true });
    });
  return { runs, runner };
}

function jobs(runner: Runner, now = () => 0) {
  let next = 0;
  return new Jobs({ runner, now, newId: () => `lh-${++next}` });
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
const signal = new AbortController().signal;

describe('Jobs', () => {
  it('runs one job at a time, in order', async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const first = queue.start(REQUEST);
    const second = queue.start({ ...REQUEST, url: 'http://localhost:5173/b' });
    await tick();
    expect(runs.map((run) => run.url)).toEqual(['http://localhost:5173/']);
    expect(await queue.poll(second, 0, signal)).toMatchObject({ job: second, state: 'queued', position: 1 });
    runs[0]?.finish(OUTPUT);
    expect(await queue.poll(first, 1000, signal)).toMatchObject({
      job: first,
      state: 'done',
      result: { finalUrl: 'http://localhost:5173/', lighthouseVersion: '13.6.0', jsonBytes: 30, htmlBytes: 19 },
    });
    await tick();
    expect(runs.map((run) => run.url)).toEqual(['http://localhost:5173/', 'http://localhost:5173/b']);
    expect(queue.status()).toEqual({ running: { job: second, label: 'mobile', url: 'http://localhost:5173/b' }, queued: 0, busy: true });
  });

  it('answers a poll as soon as the job ends, before its wait is over', async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const job = queue.start(REQUEST);
    await tick();
    const answer = queue.poll(job, 60_000, signal);
    runs[0]?.finish(OUTPUT);
    expect((await answer).state).toBe('done');
  });

  it('reports a failed run with its code and message', async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const job = queue.start(REQUEST);
    await tick();
    runs[0]?.fail(new LighthouseError(-32010, 'no browser'));
    expect(await queue.poll(job, 1000, signal)).toMatchObject({ state: 'failed', error: { code: -32010, message: 'no browser' } });
  });

  it('cancels the running and the queued jobs', async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const first = queue.start(REQUEST);
    const second = queue.start(REQUEST);
    await tick();
    expect(queue.cancel()).toBe(2);
    expect(runs[0]?.signal.aborted).toBe(true);
    expect((await queue.poll(first, 1000, signal)).state).toBe('cancelled');
    expect((await queue.poll(second, 1000, signal)).state).toBe('cancelled');
    await tick();
    expect(runs).toHaveLength(1);
  });

  it('hands out a report in slices and refuses an unknown job', async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const job = queue.start(REQUEST);
    await tick();
    const html = `<html>${'é'.repeat(50)}</html>`;
    runs[0]?.finish({ ...OUTPUT, html });
    await queue.poll(job, 1000, signal);
    const slices: string[] = [];
    let offset: number | null = 0;
    while (offset !== null) {
      const part: { text: string; total: number; next: number | null } = queue.report(job, 'html', offset, 16);
      expect(part.total).toBe(html.length);
      slices.push(part.text);
      offset = part.next;
    }
    expect(slices.join('')).toBe(html);
    expect(queue.report(job, 'json', 0).text).toBe(OUTPUT.json);
    expect(() => queue.report('lh-9', 'json', 0)).toThrow(RpcError);
    expect(queue.release(job)).toBe(true);
    expect(() => queue.report(job, 'json', 0)).toThrow(RpcError);
  });

  it('never splits a surrogate pair between two slices', async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const job = queue.start(REQUEST);
    await tick();
    runs[0]?.finish({ ...OUTPUT, json: `a${'😀'.repeat(10)}` });
    await queue.poll(job, 1000, signal);
    const part = queue.report(job, 'json', 0, 2);
    expect(part.text).toBe('a');
    expect(part.next).toBe(1);
  });

  it('refuses a report of a job that has not finished', async () => {
    const { runner } = manual();
    const queue = jobs(runner);
    const job = queue.start(REQUEST);
    expect(() => queue.report(job, 'json', 0)).toThrow(RpcError);
  });

  it('refuses more queued jobs than it holds', () => {
    const { runner } = manual();
    const queue = jobs(runner);
    for (let i = 0; i < MAX_QUEUED; i += 1) {
      queue.start(REQUEST);
    }
    expect(() => queue.start(REQUEST)).toThrow(RpcError);
  });

  it('forgets finished jobs after their time', async () => {
    const { runs, runner } = manual();
    let now = 0;
    const queue = jobs(runner, () => now);
    const job = queue.start(REQUEST);
    await tick();
    runs[0]?.finish(OUTPUT);
    await queue.poll(job, 1000, signal);
    now = 11 * 60 * 1000;
    queue.status();
    await expect(queue.poll(job, 0, signal)).rejects.toThrow(RpcError);
  });

  // RB-080-67: the sidecar's end cancelled only the jobs of the caller '' - never a plugin's run,
  // whose browser was then left to whatever ended Node.
  it("cancels every caller's jobs when the sidecar ends and waits for the running one", async () => {
    const { runs, runner } = manual();
    const queue = jobs(runner);
    const first = queue.start(REQUEST, 'lighthouse');
    const second = queue.start(REQUEST, 'other');
    await tick();
    let ended = false;
    const shutdown = queue.shutdown().then(() => {
      ended = true;
    });
    expect(runs[0]?.signal.aborted).toBe(true);
    await shutdown;
    expect(ended).toBe(true);
    expect((await queue.poll(first, 0, signal, 'lighthouse')).state).toBe('cancelled');
    expect((await queue.poll(second, 0, signal, 'other')).state).toBe('cancelled');
    await tick();
    expect(runs).toHaveLength(1);
  });
});
