// The licence list against what it lists (RB-010-18): every crate of the lock files and every
// npm package of the shell is either listed with a licence or known not to ship, so a package
// new to a lock file fails here until `scripts/licenses.sh` runs again.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { allComponents, BUNDLED, filterComponents, THIRD_PARTY } from './licences';

// The repository root: the tests run in ui/ (`npm test`).
const ROOT = join(process.cwd(), '..');
const REGENERATE = 'run scripts/licenses.sh';

function read(path: string): string {
  return readFileSync(join(ROOT, path), 'utf-8');
}

/** `name@version` of every registry or git crate of a `Cargo.lock`. */
function lockedCrates(text: string): string[] {
  const crates: string[] = [];
  for (const block of text.split('[[package]]').slice(1)) {
    const name = /^name = "([^"]+)"/m.exec(block)?.[1];
    const version = /^version = "([^"]+)"/m.exec(block)?.[1];
    if (name && version && /^source = /m.test(block)) {
      crates.push(`${name}@${version}`);
    }
  }
  return crates;
}

/** The workspace's lock file and every plugin crate's (a folder of plugins/ with a Cargo.lock;
 * plugins/node_modules, the plugins' npm workspace install, has none). */
function cargoLocks(): string[] {
  // Every plugin crate; `plugins/web-common` is an npm package without one.
  const plugins = readdirSync(join(ROOT, 'plugins'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => `plugins/${entry.name}/Cargo.lock`)
    .filter((path) => existsSync(join(ROOT, path)));
  return ['Cargo.lock', ...plugins];
}

/** The npm lock files `scripts/lib/licenses.py` reads (`NPM_LOCKS`): the shell, the sync agent
 * and the sidecar. */
const NPM_LOCKS = ['ui', 'agent', 'sidecar'].map((dir) => `${dir}/package-lock.json`);

function npmPackages(): string[] {
  return NPM_LOCKS.filter((path) => existsSync(join(ROOT, path))).flatMap((path) => {
    const lock = JSON.parse(read(path)) as { packages: Record<string, { version?: string }> };
    return Object.entries(lock.packages)
      .filter(([entry, info]) => entry !== '' && info.version !== undefined)
      .map(([entry, info]) => `${entry.split('node_modules/').pop()}@${info.version}`);
  });
}

const key = (row: { name: string; version: string }) => `${row.name}@${row.version}`;

describe('the licence list', () => {
  it('accounts for every crate of the lock files', () => {
    const locked = new Set(cargoLocks().flatMap((path) => lockedCrates(read(path))));
    const listed = new Set([...THIRD_PARTY.rust.map(key), ...THIRD_PARTY.rustNotShipped]);
    expect([...locked].filter((crate) => !listed.has(crate)), REGENERATE).toEqual([]);
    expect([...listed].filter((crate) => !locked.has(crate)), REGENERATE).toEqual([]);
  });

  it('accounts for every npm package of the shell, the sync agent and the sidecar', () => {
    const locked = new Set(npmPackages());
    const listed = new Set([...THIRD_PARTY.npm.map(key), ...THIRD_PARTY.npmNotShipped]);
    expect([...locked].filter((name) => !listed.has(name)), REGENERATE).toEqual([]);
    expect([...listed].filter((name) => !locked.has(name)), REGENERATE).toEqual([]);
  });

  it('gives every listed component a licence', () => {
    const without = allComponents().filter((component) => component.license.trim() === '');
    expect(without.map((component) => component.name)).toEqual([]);
    expect(THIRD_PARTY.rust.length).toBeGreaterThan(0);
    expect(THIRD_PARTY.npm.length).toBeGreaterThan(0);
  });

  it('lists Vue and Nuxt UI among the npm packages', () => {
    const names = THIRD_PARTY.npm.map((row) => row.name);
    expect(names).toContain('vue');
    expect(names).toContain('@nuxt/ui');
  });
});

describe('the bundled binaries', () => {
  const pins = read('scripts/lib/tool-versions.sh');
  const byId = (id: string) => BUNDLED.find((row) => row.id === id);

  it('carry the pinned CEF, Chromium and Node versions', () => {
    const archive = /^RB_CEF_ARCHIVE="cef_binary_([^"]+)_windows64/m.exec(pins)?.[1] ?? '';
    const [cef, chromium] = archive.split('+chromium-');
    expect(byId('cef')?.version).toBe(cef);
    expect(byId('chromium')?.version).toBe(chromium);
    expect(byId('node')?.version).toBe(read('.nvmrc').trim());
  });

  it('carry the bundled icon set at its locked version', () => {
    const lock = JSON.parse(read('ui/package-lock.json')) as { packages: Record<string, { version?: string }> };
    expect(byId('lucide')?.version).toBe(lock.packages['node_modules/@iconify-json/lucide']?.version);
  });

  it("carry the sidecar's Lighthouse and puppeteer-core at their locked versions", () => {
    const lock = JSON.parse(read('sidecar/package-lock.json')) as {
      packages: Record<string, { version?: string; license?: string }>;
    };
    for (const id of ['lighthouse', 'puppeteer-core']) {
      const locked = lock.packages[`node_modules/${id}`];
      expect(byId(id)?.version, id).toBe(locked?.version);
      expect(byId(id)?.license, id).toBe(locked?.license);
      expect(byId(id)?.status, id).toBe('bundled');
    }
  });
});

describe('the filter', () => {
  it('finds axe-core under MPL', () => {
    const names = filterComponents(allComponents(), 'mpl').map((component) => component.name);
    expect(names).toContain('axe-core');
    expect(names.every((name) => name !== 'vue')).toBe(true);
  });

  it('matches names ignoring case and keeps everything for a blank query', () => {
    const all = allComponents();
    expect(filterComponents(all, '  ')).toHaveLength(all.length);
    expect(filterComponents(all, 'SERDE_JSON').map((component) => component.name)).toContain('serde_json');
  });
});
