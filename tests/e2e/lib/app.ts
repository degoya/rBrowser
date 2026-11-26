// The app under test (RB-099-02): the package scripts/e2e-windows.sh staged in a folder of its own
// on Windows (%LOCALAPPDATA%\rbrowser-e2e), run in the portable mode (the file `portable` beside
// rbrowser.exe puts every setting, the trust store and the instance files under `data\`,
// crates/rb-config/src/dirs.rs) so a run never touches the person's own rBrowser and starts from
// known settings. From WSL the Windows programs run through the interop, natively (Git Bash on
// the runner) directly; every spawn hides its console window (`windowsHide`).

import { execFileSync, spawn } from 'node:child_process';
import type { ChildProcess } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import type { Mode } from './args.ts';
import { McpClient } from './mcp.ts';

/** The file scripts/e2e-windows.sh writes into the folder it staged: only there is data wiped. */
export const STAGE_MARKER = '.rbrowser-e2e';

/**
 * The user settings of a run (`data\settings.json`): MCP and the CDP proxy on (the run drives
 * the app as a client would; mcp.allowEvaluate is cdp_endpoint's condition), the timing counters
 * on (K11), and off whatever would reach out or wait for a person (updates, the first-run wizard).
 * Trust is never seeded: a project of the run is untrusted, as K9 needs.
 */
export const SEED_SETTINGS = {
  version: 1,
  mcp: { enabled: true, allowEvaluate: true },
  cdp: { enabled: true },
  performance: { counters: true },
  updates: { enabled: false },
  ui: { firstRunWizard: false },
} as const;

export interface AppInstall {
  mode: Mode;
  dir: string;
  exe: string;
  com: string;
  data: string;
  instances: string;
}

export function installOf(dir: string, mode: Mode): AppInstall {
  return {
    mode,
    dir,
    exe: join(dir, 'rbrowser.exe'),
    com: join(dir, 'rbrowser.com'),
    data: join(dir, 'data'),
    instances: join(dir, 'data', 'local', 'mcp'),
  };
}

/** The portable mode on, `data\` emptied and the seeded settings written. */
export function prepareData(install: AppInstall): void {
  if (!existsSync(join(install.dir, STAGE_MARKER))) {
    throw new Error(`${install.dir} was not staged by scripts/e2e-windows.sh (no ${STAGE_MARKER}): its data stays`);
  }
  writeFileSync(join(install.dir, 'portable'), '');
  rmSync(install.data, { recursive: true, force: true });
  mkdirSync(install.data, { recursive: true });
  writeFileSync(join(install.data, 'settings.json'), `${JSON.stringify(SEED_SETTINGS, null, 2)}\n`);
}

/**
 * A program of System32: by name on Windows and when WSL appends the Windows PATH, else at its
 * place under /mnt/c (WSL without `appendWindowsPath`, as scripts/run-windows.sh allows).
 */
export function system32(name: string): string {
  const mounted = `/mnt/c/Windows/System32/${name}`;
  return process.platform !== 'win32' && existsSync(mounted) ? mounted : name;
}

/** `path` as Windows sees it: `wslpath -w` from WSL, unchanged natively. */
export function windowsPath(path: string, mode: Mode): string {
  if (mode === 'native') return path;
  return execFileSync('wslpath', ['-w', path], { encoding: 'utf8' }).trim();
}

export interface Ran {
  code: number | null;
  stdout: string;
  stderr: string;
}

/** Runs `rbrowser.com <args>` to its end (`init`, `--version`); fails after `timeoutMs`. */
export function runCom(install: AppInstall, args: readonly string[], timeoutMs = 120_000): Promise<Ran> {
  return new Promise((resolve, reject) => {
    const child = spawn(install.com, args, { cwd: install.dir, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk: Buffer) => (stdout += chunk.toString('utf8')));
    child.stderr.on('data', (chunk: Buffer) => (stderr += chunk.toString('utf8')));
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`rbrowser.com ${args[0] ?? ''}: no end in ${timeoutMs} ms`));
    }, timeoutMs);
    child.on('error', (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.on('close', (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
  });
}

/**
 * Starts rBrowser on the project `projectWin` (a Windows path), detached: from WSL through
 * `cmd.exe /c start` from C: (as scripts/run-windows.sh does), natively as a detached child.
 * Every bundled plugin is installed at this start (the SEO check of K8, the panels of M7).
 */
export function startApp(install: AppInstall, projectWin: string): void {
  const args = ['--project', projectWin, '--install-all-bundled-plugins'];
  if (install.mode === 'wsl') {
    const exe = windowsPath(install.exe, 'wsl');
    // `start ""`: the empty title, as an empty argument (the interop quotes it).
    spawn(system32('cmd.exe'), ['/d', '/c', 'start', '', exe, ...args], {
      cwd: install.dir,
      detached: true,
      stdio: 'ignore',
      windowsHide: true,
    }).unref();
    return;
  }
  spawn(install.exe, args, { cwd: install.dir, detached: true, stdio: 'ignore', windowsHide: true }).unref();
}

/** `rbrowser.com mcp --project <projectWin>` and a client on its stdio (not yet initialized). */
export function spawnMcp(install: AppInstall, projectWin: string): { client: McpClient; child: ChildProcess } {
  const child = spawn(install.com, ['mcp', '--project', projectWin], {
    cwd: install.dir,
    windowsHide: true,
    stdio: ['pipe', 'pipe', 'ignore'],
  });
  return { client: new McpClient(child.stdin, child.stdout), child };
}

/** Ends the process tree of `pid` (the app the run started, by the pid of its instance file). */
export function stopPid(pid: number): void {
  try {
    execFileSync(system32('taskkill.exe'), ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true });
  } catch {
    // Gone already.
  }
}

/** The rows of `tasklist /FO CSV /NH`: one per process. */
export function taskCount(csv: string): number {
  return csv.split(/\r?\n/).filter((line) => line.startsWith('"')).length;
}

/** How many rbrowser.exe processes run on Windows (scripts/perf.sh sums their memory). */
export function rbrowserProcesses(): number {
  const csv = execFileSync(system32('tasklist.exe'), ['/FI', 'IMAGENAME eq rbrowser.exe', '/FO', 'CSV', '/NH'], {
    encoding: 'utf8',
    windowsHide: true,
  });
  return taskCount(csv);
}
