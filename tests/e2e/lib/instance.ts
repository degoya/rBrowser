// The MCP instance file of a running rBrowser (crates/rb-mcp/src/instance.rs):
// `<local dir>/mcp/<project key>.json` with `{version, pid, port, token, project}`. The harness
// reads it for the pid (to stop the app it started) and to know that the app is up; it never
// keeps the token - MCP goes through `rbrowser.com mcp`, which reads the file itself.

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/** The rb-mcp file version this reads. */
const FILE_VERSION = 1;
const TOKEN = /^[0-9a-f]{64}$/i;
/** The most bytes rb-mcp reads of a file. */
const MAX_FILE = 4 * 1024;

export interface Instance {
  version: number;
  pid: number;
  port: number;
  project: string;
}

/** The instance in `text` when it is one; the token is checked, not kept. */
export function parseInstance(text: string): Instance | undefined {
  if (text.length > MAX_FILE) return undefined;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return undefined;
  }
  if (typeof value !== 'object' || value === null) return undefined;
  const fields = value as Record<string, unknown>;
  const { version, pid, port, token, project } = fields;
  if (
    version !== FILE_VERSION ||
    typeof pid !== 'number' ||
    typeof port !== 'number' ||
    port <= 0 ||
    typeof token !== 'string' ||
    !TOKEN.test(token) ||
    typeof project !== 'string'
  ) {
    return undefined;
  }
  return { version, pid, port, project };
}

function normal(path: string): string {
  return path
    .replaceAll('/', '\\')
    .replace(/^\\\\wsl\$\\/i, '\\\\wsl.localhost\\')
    .replace(/\\+$/, '')
    .toLowerCase();
}

/** Whether two Windows paths name the same project folder. */
export function sameProject(a: string, b: string): boolean {
  return normal(a) === normal(b);
}

/** The instance of `project` among the files of `dir` (`<local dir>/mcp`). */
export function findInstance(dir: string, project: string): Instance | undefined {
  let names: string[];
  try {
    names = readdirSync(dir).filter((name) => name.endsWith('.json'));
  } catch {
    return undefined;
  }
  for (const name of names) {
    let text: string;
    try {
      text = readFileSync(join(dir, name), 'utf8');
    } catch {
      continue;
    }
    const instance = parseInstance(text);
    if (instance !== undefined && sameProject(instance.project, project)) return instance;
  }
  return undefined;
}
