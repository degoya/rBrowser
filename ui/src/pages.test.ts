// Every page of the shell (`ui/*.html`) is built and loaded by some view (RB-090-27): a page no
// view loads is dead code whose components look tested while no one sees them - as the cell
// label page was. A page is loaded when the core names its `rbrowser://ui/<page>.html` URL in a
// line of code; it is built when it is an input of `vite.config.ts`.

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// The repository root: the tests run in ui/ (`npm test`).
const ROOT = join(process.cwd(), '..');
const UI = join(ROOT, 'ui');

/** The shell's pages, `index.html` included. */
function pages(): string[] {
  return readdirSync(UI)
    .filter((name) => name.endsWith('.html'))
    .sort();
}

/** Every Rust source of the app, its tests aside. */
function rustSources(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...rustSources(path));
    else if (entry.name.endsWith('.rs') && !entry.name.endsWith('_tests.rs')) out.push(path);
  }
  return out;
}

/** The code of the app's Rust sources, comment lines left out. */
function rustCode(): string {
  return rustSources(join(ROOT, 'crates/rbrowser/src'))
    .map((path) =>
      readFileSync(path, 'utf-8')
        .split('\n')
        .filter((line) => !line.trimStart().startsWith('//'))
        .join('\n'),
    )
    .join('\n');
}

describe('the shell pages', () => {
  it('finds the pages', () => {
    expect(pages()).toContain('index.html');
  });

  it('loads every page in some view', () => {
    const code = rustCode();
    const unloaded = pages().filter((page) => !code.includes(`"rbrowser://ui/${page}"`));
    expect(unloaded).toEqual([]);
  });

  it('builds every page and nothing else', () => {
    const config = readFileSync(join(UI, 'vite.config.ts'), 'utf-8');
    const inputs = [...config.matchAll(/new URL\('\.\/([\w-]+\.html)'/g)].map((match) => match[1]).sort();
    expect(inputs).toEqual(pages());
  });
});
