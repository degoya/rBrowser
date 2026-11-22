// The real dependencies of `flows.run`: the worker beside the sidecar's entry point
// (`dist/flow-worker.js` in the package, `src/modules/flows/worker.ts` when run from source),
// started with the sidecar's own Node, without a console window.

import { spawn } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { cp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { VIEW_OVERHEAD_MS } from './result.ts';
import type { FlowsDeps } from './spawn.ts';

/** How long past its budget a worker may take to report (loading Playwright included). */
const GRACE_MS = 15_000;

/** The deps for the sidecar whose entry point is `mainUrl` (`import.meta.url` of main). */
export function realFlowsDeps(mainUrl: string): FlowsDeps {
  const main = fileURLToPath(mainUrl);
  const bundled = main.endsWith('.js');
  const sidecarDir = dirname(dirname(main));
  return {
    nodePath: process.execPath,
    workerScript: bundled ? join(dirname(main), 'flow-worker.js') : join(dirname(main), 'modules', 'flows', 'worker.ts'),
    sidecarDir,
    netFlag: process.allowedNodeEnvironmentFlags.has('--allow-net'),
    baseEnv: process.env,
    graceMs: GRACE_MS,
    viewOverheadMs: VIEW_OVERHEAD_MS,
    spawn: (args, env) =>
      spawn(process.execPath, args, { env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true }),
    makeDir: async (path) => {
      await mkdir(path, { recursive: true });
    },
    removeDir: (path) => rm(path, { recursive: true, force: true }),
    // The sidecar runs without the permission model: the copy onto a WSL share works here.
    copyDir: (from, to) => cp(from, to, { recursive: true, force: true }),
    platform: process.platform,
    systemTmp: tmpdir(),
    // The worker's folders exist when it starts (its temp folder is made first).
    realPath: (path) => realpathSync.native(path),
  };
}
