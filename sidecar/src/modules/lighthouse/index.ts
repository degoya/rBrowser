// `lighthouse.*` (RB-060-07): Lighthouse for the `lighthouse` plugin, which reaches these methods
// through the `sidecar` capability with exactly this list in its manifest.
//
// - `lighthouse.start {url, settings, label?}` -> `{job}`: queues a run (params.ts holds what a
//   caller may set); runs go one at a time (jobs.ts).
// - `lighthouse.poll {job, waitMs?}` -> the job's state, waiting at most 20 s for it to end.
// - `lighthouse.report {job, format, offset?}` -> one slice of the JSON result or the HTML report.
// - `lighthouse.release {job}`, `lighthouse.cancel {}`, `lighthouse.status {}`.
//
// rBrowser adds `$caller` (the calling plugin's id) to every call; each caller reaches only its own
// jobs (jobs.ts).

import { randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';

import type { Router } from '../../router.ts';
import { lighthouseDangling } from './dangling.ts';
import { Jobs, type JobsDeps, SLICE_CHARS } from './jobs.ts';
import { readCaller, readJob, readReport, readStart, readWait } from './params.ts';
import { loadLibraries, realRunner, sidecarBrowser, stderr } from './real.ts';
import { killTree, machineTree } from './tree.ts';

/** The real dependencies: this machine's browser and the pinned Lighthouse. */
export function realLighthouseDeps(): JobsDeps {
  return {
    runner: realRunner({
      findBrowser: sidecarBrowser,
      load: loadLibraries,
      killTree: (pid, alive) => killTree(pid, machineTree(), alive),
      log: stderr,
      profiles: { root: tmpdir() },
      dangling: lighthouseDangling,
    }),
    now: () => Date.now(),
    newId: () => `lh-${randomUUID()}`,
  };
}

export function registerLighthouse(router: Router, deps: JobsDeps): Jobs {
  const jobs = new Jobs(deps);
  router.register('lighthouse.start', (params) => ({ job: jobs.start(readStart(params), readCaller(params)) }));
  router.register('lighthouse.poll', (params, ctx) =>
    jobs.poll(readJob(params), readWait(params), ctx.signal, readCaller(params)),
  );
  router.register('lighthouse.report', (params) => {
    const { job, format, offset } = readReport(params);
    return jobs.report(job, format, offset, SLICE_CHARS, readCaller(params));
  });
  router.register('lighthouse.release', (params) => ({ released: jobs.release(readJob(params), readCaller(params)) }));
  router.register('lighthouse.cancel', (params) => ({ cancelled: jobs.cancel(readCaller(params)) }));
  router.register('lighthouse.status', (params) => jobs.status(readCaller(params)));
  return jobs;
}
