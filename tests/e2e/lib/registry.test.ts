import { readdirSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { CASES } from './registry.ts';

describe('the registry', () => {
  it('has K1 to K12, each id once', () => {
    const ids = CASES.map((entry) => entry.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (let k = 1; k <= 12; k += 1) expect(ids).toContain(`K${k}`);
  });

  it('holds every case file of tests/e2e', () => {
    const files = readdirSync(join(import.meta.dirname, '..')).filter((name) => /^(k\d\d|m\d\d|sync)-.*\.ts$/.test(name));
    expect(files.length).toBe(CASES.length);
  });

  it('names a threshold and the jobs of each case', () => {
    for (const entry of CASES) {
      expect(entry.threshold.length, entry.id).toBeGreaterThan(0);
      expect(entry.jobs.every((job) => /^RB-\d{3}-\d{2}$/.test(job)), entry.id).toBe(true);
    }
  });

  it('runs K11 last among the e2e project cases (it switches engines)', () => {
    const e2e = CASES.filter((entry) => entry.project === 'e2e');
    expect(e2e[e2e.length - 1]?.id).toBe('K11');
  });
});
