// One running app on one project (RB-099-02): started detached, found by its MCP instance file,
// driven over `rbrowser.com mcp` like any MCP client, stopped by the pid of that file.

import type { ChildProcess } from 'node:child_process';

import { spawnMcp, startApp, stopPid, windowsPath } from './app.ts';
import type { AppInstall } from './app.ts';
import { endpointOf } from './cdp.ts';
import type { Endpoint } from './cdp.ts';
import { findInstance } from './instance.ts';
import type { McpClient } from './mcp.ts';
import { waitFor } from './measure.ts';

/** How long a cold start may take before the run gives up on it (CEF, plugins, the views). */
const START_TIMEOUT_MS = 120_000;
/** How long a page may take to load in every view. */
const LOAD_TIMEOUT_MS = 30_000;

/** A view as `views_list` answers it (the fields the run reads). */
export interface View {
  id: number;
  label?: string;
  breakpoint: string | null;
  engine: string;
  width: number;
  height: number;
  dpr?: number;
  url: string;
  loading?: boolean;
  desync: { reason?: string; selector?: string } | null;
  unsupported?: string[];
}

/** One view's value of `evaluate`. */
export interface Evaluated<T> {
  view: number;
  breakpoint?: string;
  value: T;
}

export interface Session {
  projectWin: string;
  pid: number;
  mcp: McpClient;
  views(): Promise<View[]>;
  /** `navigate` with `target: all`, then every view's page loaded; answers the navigate answer. */
  navigateAll(url: string): Promise<{ settled?: boolean; views?: { url?: string }[] }>;
  evaluateAll<T>(expression: string): Promise<Evaluated<T>[]>;
  /** The `cdp_endpoint` answer (it holds the token: never log it). */
  endpointAnswer(): Promise<unknown>;
  endpoint(): Promise<Endpoint>;
  close(keep: boolean): void;
}

export async function openSession(
  install: AppInstall,
  projectLocal: string,
  log: (line: string) => void,
): Promise<Session> {
  const projectWin = windowsPath(projectLocal, install.mode);
  log(`starting rBrowser on ${projectWin}`);
  startApp(install, projectWin);
  const instance = await waitFor(async () => findInstance(install.instances, projectWin), {
    timeoutMs: START_TIMEOUT_MS,
    stepMs: 500,
    what: `the MCP instance file of ${projectWin}`,
  });
  log(`rBrowser runs (pid ${instance.pid}, MCP on its port)`);
  const { client, child } = spawnMcp(install, projectWin);
  await client.initialize();
  const session = makeSession(projectWin, instance.pid, client, child, install.mode === 'wsl');
  await waitFor(async () => ((await session.views()).length > 0 ? true : undefined), {
    timeoutMs: START_TIMEOUT_MS,
    stepMs: 500,
    what: 'the first view',
  });
  return session;
}

function makeSession(
  projectWin: string,
  pid: number,
  mcp: McpClient,
  child: ChildProcess,
  inWsl: boolean,
): Session {
  const views = () => mcp.value<View[]>('views_list');
  const evaluateAll = <T>(expression: string) =>
    mcp.value<Evaluated<T>[]>('evaluate', { expression, target: 'all', awaitPromise: true });
  return {
    projectWin,
    pid,
    mcp,
    views,
    evaluateAll,
    async navigateAll(url) {
      const answer = await mcp.value<{ settled?: boolean; views?: { url?: string }[] }>('navigate', { url, target: 'all' });
      await waitFor(
        async () => {
          // The new document, not the one before it, which is `complete` too.
          const states = await evaluateAll<boolean>(
            `location.href === new URL(${JSON.stringify(url)}, location.href).href && document.readyState === 'complete'`,
          );
          return states.length > 0 && states.every((state) => state.value === true) ? true : undefined;
        },
        { timeoutMs: LOAD_TIMEOUT_MS, stepMs: 250, what: `${url} loaded in every view` },
      );
      return answer;
    },
    endpointAnswer: () => mcp.value('cdp_endpoint'),
    async endpoint() {
      return endpointOf(await mcp.value('cdp_endpoint'), inWsl);
    },
    close(keep) {
      child.stdin?.end();
      child.kill();
      if (!keep) stopPid(pid);
    },
  };
}
