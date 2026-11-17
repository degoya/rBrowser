// Kills a Lighthouse browser with every process it started (RB-080-67), when its graceful close
// did not end it in time (end.ts): on Windows with `taskkill /PID <pid> /T /F` (one stdout line
// per process ended), elsewhere by killing the process group puppeteer gives the browser
// (`detached` off Windows). Only while the browser's main process is known to live (`alive`,
// Node's own record of its child): the id of an exited browser, and its group, may belong to
// somebody else by now (review 4). rb-sidecar's job object (rb-process) is the second line:
// whatever Node started ends with the sidecar.

import { execFile } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { win32 } from 'node:path';

import { TASKKILL_TIMEOUT_MS } from './budget.ts';

export interface TreeDeps {
  platform: NodeJS.Platform;
  /** `%SystemRoot%`, for taskkill's full path (the sidecar's PATH comes from an allowlist). */
  systemRoot: string | undefined;
  /** Runs a program; its stdout, also after a non-zero exit; null when it does not start. */
  exec(file: string, args: string[]): Promise<{ stdout: string } | null>;
  /** The members of process group `pgid`; null where the platform does not list them. */
  group(pgid: number): number[] | null;
  /** Sends SIGKILL to process group `pgid`; throws when there is none. */
  killGroup(pgid: number): void;
}

/**
 * Ends the process tree under `pid` at once while `alive()` - asked right before the kill -
 * says its main process still runs; answers how many processes ended (null: unknown).
 */
export async function killTree(pid: number, deps: TreeDeps, alive: () => boolean): Promise<number | null> {
  if (!Number.isSafeInteger(pid) || pid <= 0 || !alive()) {
    return 0;
  }
  if (deps.platform === 'win32') {
    const taskkill = deps.systemRoot === undefined ? 'taskkill.exe' : win32.join(deps.systemRoot, 'System32', 'taskkill.exe');
    const result = await deps.exec(taskkill, ['/PID', String(pid), '/T', '/F']);
    if (result === null) {
      return null;
    }
    return result.stdout.split(/\r?\n/).filter((line) => line.trim() !== '').length;
  }
  const members = deps.group(pid);
  if (!alive()) {
    return 0;
  }
  try {
    deps.killGroup(pid);
  } catch {
    return 0;
  }
  return members === null ? null : members.length;
}

/** The members of a process group from `/proc` (Linux); null elsewhere. */
function procGroup(pgid: number): number[] | null {
  let names: string[];
  try {
    names = readdirSync('/proc');
  } catch {
    return null;
  }
  const members: number[] = [];
  for (const name of names) {
    if (!/^\d+$/.test(name)) {
      continue;
    }
    try {
      const stat = readFileSync(`/proc/${name}/stat`, 'utf8');
      // pid (comm) state ppid pgrp ...: the command may hold spaces and parentheses.
      const fields = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
      if (Number(fields[2]) === pgid) {
        members.push(Number(name));
      }
    } catch {
      // The process ended between the listing and the read.
    }
  }
  return members;
}

/** This machine. */
export function machineTree(): TreeDeps {
  return {
    platform: process.platform,
    systemRoot: process.env.SystemRoot,
    exec: (file, args) =>
      new Promise((resolve) => {
        execFile(file, args, { windowsHide: true, timeout: TASKKILL_TIMEOUT_MS }, (error, stdout) => {
          if (error !== null && typeof error.code === 'string') {
            // ENOENT and the like: the program did not run.
            resolve(null);
            return;
          }
          resolve({ stdout: String(stdout) });
        });
      }),
    group: (pgid) => (process.platform === 'linux' ? procGroup(pgid) : null),
    killGroup: (pgid) => {
      process.kill(-pgid, 'SIGKILL');
    },
  };
}
