// K1, K3 and K11 are measured by scripts/perf.sh (RB-020-09, RB-090-32, docs/perf-budget.md);
// the run calls it with the session's CDP endpoint and reads the JSON it writes, rather than
// measuring a second way.

import { spawn } from 'node:child_process';
import { appendFileSync, chmodSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import type { Context } from './context.ts';
import { redact } from './report.ts';

/** The result perf.sh wrote last into `dir` (its file names sort by time), or null. */
export function newestJson(dir: string): unknown {
  let names: string[];
  try {
    names = readdirSync(dir).filter((name) => name.endsWith('.json')).sort();
  } catch {
    return null;
  }
  const last = names[names.length - 1];
  return last === undefined ? null : (JSON.parse(readFileSync(join(dir, last), 'utf8')) as unknown);
}

/** Writes `value` as JSON readable by its owner only. */
export function writePrivate(file: string, value: unknown): void {
  writeFileSync(file, `${JSON.stringify(value)}\n`, { mode: 0o600 });
  chmodSync(file, 0o600);
}

export interface PerfRun {
  code: number | null;
  /** perf.sh's result JSON, or null when it wrote none. */
  result: Record<string, unknown> | null;
}

/**
 * Runs `scripts/perf.sh <args> --out <work>/<name>` with the session's endpoint in a private file
 * (removed afterwards); its output goes to the case log, redacted. Exit 0 met, 1 missed, 2 failed.
 */
export async function runPerf(ctx: Context, name: string, args: readonly string[], endpoint: unknown): Promise<PerfRun> {
  const out = join(ctx.work, name);
  rmSync(out, { recursive: true, force: true });
  const secret = join(ctx.work, `${name}-cdp-endpoint.json`);
  writePrivate(secret, endpoint);
  try {
    const code = await new Promise<number | null>((resolve, reject) => {
      const child = spawn('bash', [join(ctx.root, 'scripts/perf.sh'), ...args, '--out', out], {
        cwd: ctx.root,
        env: { ...process.env, RBROWSER_CDP_ENDPOINT: secret },
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
      });
      const keep = (chunk: Buffer) => ctx.log(redact(chunk.toString('utf8')).trimEnd());
      child.stdout.on('data', keep);
      child.stderr.on('data', keep);
      child.on('error', reject);
      child.on('close', resolve);
    });
    const result = newestJson(out);
    return { code, result: result !== null && typeof result === 'object' ? (result as Record<string, unknown>) : null };
  } finally {
    rmSync(secret, { force: true });
  }
}

/** Appends `text` to `file` redacted (the per-case logs). */
export function appendLog(file: string, text: string): void {
  appendFileSync(file, `${redact(text)}\n`);
}
