// The flow worker's own folder (RB-090-72): its temp folder (TEMP/TMP/TMPDIR, so Playwright's
// `playwright-artifacts-*` lands there) and the screenshots it takes, on a local drive - never on
// the project's side. A project in WSL is a share (`\\wsl.localhost\...`), and Node's permission
// model refused the worker's removal of Playwright's folder there in every form granted (diag18),
// which ended the run before it reported. rBrowser names the folder (`tmpDir`, its local folder's
// `flows-tmp\<runId>`); without one the worker takes `%LOCALAPPDATA%\rBrowser\flows-tmp\<runId>`
// on Windows, else the system temp folder's `rbrowser-flows-tmp/<runId>`. The sidecar copies the
// screenshots into the run folder afterwards and removes this folder.

import { posix, win32 } from 'node:path';

import { grantForms } from './grants.ts';

/** The fallback's folder under `%LOCALAPPDATA%\rBrowser` (the core's is the same name). */
export const FLOWS_TMP_DIR = 'flows-tmp';
/** The fallback's folder under the system temp folder. */
export const SYSTEM_TMP_DIR = 'rbrowser-flows-tmp';
/** The screenshots' folder inside the worker's folder. */
export const SHOTS_DIR = 'out';

/** The worker's folder of run `runId`: `given` (rBrowser's) or the fallback. */
export function workerTmpDir(
  given: string | null,
  runId: string,
  env: Record<string, string | undefined>,
  platform: NodeJS.Platform,
  systemTmp: string,
): string {
  if (platform !== 'win32') {
    return given ?? posix.join(systemTmp, SYSTEM_TMP_DIR, runId);
  }
  if (given !== null) {
    return grantForms(given, platform)[0] ?? given;
  }
  const local = env.LOCALAPPDATA;
  return local !== undefined && local !== ''
    ? win32.join(local, 'rBrowser', FLOWS_TMP_DIR, runId)
    : win32.join(systemTmp, SYSTEM_TMP_DIR, runId);
}

/** The screenshots' folder inside the worker's folder `tmp`, with the separator of `platform`. */
export function shotsDirOf(tmp: string, platform: NodeJS.Platform): string {
  return platform === 'win32' ? win32.join(tmp, SHOTS_DIR) : posix.join(tmp, SHOTS_DIR);
}
