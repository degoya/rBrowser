// The shell's pages allow no popup: a bare `target="_blank"` link does nothing there (RB-080-92,
// the findings' Help). Every external link is `ExternalLink.vue`; this holds `ui/src` to it.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

// `ui/src`: the tests run in ui/ (`npm test`).
const ROOT = join(process.cwd(), 'src');

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      return sources(path);
    }
    return /\.(vue|ts)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
  });
}

describe('external links in the shell', () => {
  it('never use a bare target="_blank"', () => {
    const offenders = sources(ROOT).filter((path) => /target\s*=\s*["']_blank["']|target:\s*["']_blank["']/.test(readFileSync(path, 'utf8')));
    expect(offenders.map((path) => relative(ROOT, path))).toEqual([]);
  });
});
