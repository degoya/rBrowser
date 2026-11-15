// The real worker process under the permission flags `flows.run` gives it: Playwright and
// @playwright/test load, a flow that does not parse fails every view with its line, a view that
// cannot be reached fails with a network error (not a permission error), and a flow cannot read
// outside the project or start processes.

import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterAll, describe, expect, it } from 'vitest';

import { nodeArgs, workerEnv } from './spawn.ts';
import type { RunJob, ViewResult } from './result.ts';

const HERE = dirname(fileURLToPath(import.meta.url));
const SIDECAR = join(HERE, '..', '..', '..');
const WORKER = join(HERE, 'worker.ts');
const ROOT = mkdtempSync(join(tmpdir(), 'rb-flow-project-'));
const OUT = join(ROOT, '.rbrowser', 'out', 'flows', 'run-1');
const SECRET = mkdtempSync(join(tmpdir(), 'rb-flow-outside-'));
const NET = process.allowedNodeEnvironmentFlags.has('--allow-net');

mkdirSync(join(OUT, '.tmp'), { recursive: true });
writeFileSync(join(ROOT, 'fixture.txt'), 'inside');
writeFileSync(join(SECRET, 'secret.txt'), 'outside');

afterAll(() => {
  rmSync(ROOT, { recursive: true, force: true });
  rmSync(SECRET, { recursive: true, force: true });
});

function run(args: string[], input: string): Promise<{ code: number | null; stdout: string; stderr: string }> {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, args, {
      env: workerEnv(process.env, join(OUT, '.tmp')),
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk: Buffer) => (stdout += chunk.toString()));
    child.stderr.on('data', (chunk: Buffer) => (stderr += chunk.toString()));
    child.on('close', (code) => resolve({ code, stdout, stderr }));
    child.stdin.end(input);
  });
}

function job(source: string): RunJob {
  return {
    runId: 'run-1',
    flow: { name: 'probe', source, file: join(ROOT, '.rbrowser', 'flows', 'probe.ts') },
    views: [
      {
        id: 1,
        label: 'Mobile',
        width: 375,
        height: 812,
        engine: 'chromium',
        breakpoint: 'mobile',
        endpoint: { url: 'http://127.0.0.1:1', headers: { Authorization: `Bearer ${'ab'.repeat(32)}` } },
      },
    ],
    env: null,
    timeoutMs: 10_000,
    parallel: 0,
    screenshotOnFailure: true,
    outDir: OUT,
  };
}

async function results(source: string): Promise<ViewResult[]> {
  const args = nodeArgs({ sidecarDir: SIDECAR, workerScript: WORKER, netFlag: NET }, ROOT, OUT);
  const { stdout, stderr } = await run(args, `${JSON.stringify(job(source))}\n`);
  const last = stdout.trim().split('\n').at(-1) ?? '';
  const event = JSON.parse(last) as { type: string; views?: ViewResult[] };
  expect(event.type, stderr).toBe('result');
  return event.views ?? [];
}

describe('the flow worker', () => {
  it('fails every view of a flow that does not parse, with its line', async () => {
    const [view] = await results('const a = 1;\nconst = ;\n');
    expect(view).toMatchObject({ viewId: 1, status: 'failed' });
    expect(view?.error?.line).toBe(2);
  });

  it('loads Playwright under the permission model and reaches the network', async () => {
    const [view] = await results('await page.goto("about:blank");\n');
    expect(view?.status).toBe('failed');
    expect(view?.error?.message).not.toContain('ERR_ACCESS_DENIED');
    expect(view?.error?.message).toMatch(/ECONNREFUSED|connect/i);
  }, 30_000);
});

describe('the permission flags', () => {
  const probe = (code: string) => {
    const args = nodeArgs({ sidecarDir: SIDECAR, workerScript: '--input-type=module', netFlag: NET }, ROOT, OUT);
    return run([...args.slice(0, -1), '--input-type=module', '-e', code], '');
  };

  it('let a flow read the project and write the run folder only', async () => {
    const fs = "const fs = process.getBuiltinModule('node:fs');";
    const inside = await probe(`${fs} console.log(fs.readFileSync(${JSON.stringify(join(ROOT, 'fixture.txt'))}, 'utf8'));`);
    expect(inside.stdout.trim()).toBe('inside');
    const outside = await probe(`${fs} fs.readFileSync(${JSON.stringify(join(SECRET, 'secret.txt'))});`);
    expect(outside.code).not.toBe(0);
    expect(outside.stderr).toContain('ERR_ACCESS_DENIED');
    const write = await probe(`${fs} fs.writeFileSync(${JSON.stringify(join(ROOT, 'x.txt'))}, 'x');`);
    expect(write.stderr).toContain('ERR_ACCESS_DENIED');
    const out = await probe(`${fs} fs.writeFileSync(${JSON.stringify(join(OUT, 'shot.png'))}, 'x');`);
    expect(out.code).toBe(0);
  });

  it('refuse child processes', async () => {
    const spawned = await probe("process.getBuiltinModule('node:child_process').execFileSync(process.execPath, ['-v']);");
    expect(spawned.stderr).toContain('ERR_ACCESS_DENIED');
  });
});
