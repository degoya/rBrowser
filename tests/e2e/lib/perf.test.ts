import { mkdirSync, mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { newestJson, writePrivate } from './perf.ts';
import { stageProject } from './projects.ts';

describe('newestJson', () => {
  it('reads the last result perf.sh wrote (its names sort by time)', () => {
    const dir = mkdtempSync(join(tmpdir(), 'e2e-perf-'));
    writeFileSync(join(dir, '2026-11-26T10-00-00.json'), '{"n":1}');
    writeFileSync(join(dir, '2026-11-26T11-00-00.json'), '{"n":2}');
    writeFileSync(join(dir, 'notes.txt'), 'x');
    expect(newestJson(dir)).toEqual({ n: 2 });
    expect(newestJson(join(dir, 'none'))).toBeNull();
  });
});

describe('writePrivate', () => {
  it('writes a file only its owner can read (the cdp_endpoint answer holds the token)', () => {
    const file = join(mkdtempSync(join(tmpdir(), 'e2e-secret-')), 'cdp.json');
    writePrivate(file, { url: 'http://127.0.0.1:1' });
    expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual({ url: 'http://127.0.0.1:1' });
    if (process.platform !== 'win32') expect(statSync(file).mode & 0o777).toBe(0o600);
  });
});

describe('stageProject', () => {
  it('copies a fixture project afresh into the work folder', () => {
    const root = mkdtempSync(join(tmpdir(), 'e2e-root-'));
    const work = mkdtempSync(join(tmpdir(), 'e2e-work-'));
    mkdirSync(join(root, 'tests/fixtures/p/.rbrowser'), { recursive: true });
    writeFileSync(join(root, 'tests/fixtures/p/.rbrowser/config.json'), '{"version":1}');
    mkdirSync(join(work, 'p'));
    writeFileSync(join(work, 'p', 'stale.txt'), 'old');
    const copy = stageProject(root, work, 'tests/fixtures/p');
    expect(copy).toBe(join(work, 'p'));
    expect(readFileSync(join(copy, '.rbrowser/config.json'), 'utf8')).toBe('{"version":1}');
    expect(() => statSync(join(copy, 'stale.txt'))).toThrow();
  });
});
