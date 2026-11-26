import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import { shellMessages } from './index';
import { PAGES_CATALOGUE } from './pages';
import { PAGES_ONLY_PREFIXES, splitCatalogue } from './split';
import type { Catalogue } from './messages';

function flatten(catalogue: Catalogue, prefix = ''): Map<string, string> {
  const flat = new Map<string, string>();
  for (const [segment, value] of Object.entries(catalogue)) {
    const key = prefix === '' ? segment : `${prefix}.${segment}`;
    if (typeof value === 'string') {
      flat.set(key, value);
    } else {
      for (const [inner, text] of flatten(value, key)) {
        flat.set(inner, text);
      }
    }
  }
  return flat;
}

const SRC = join(process.cwd(), 'src');
/** The pages window's own modules: they run with the pages catalogue beside the shell's. */
const PAGES_MODULES = ['settings/'];
/** Keys the shell only hands to the core, which renders them itself (a dialog title). */
const SENT_TO_CORE = ['ipc/contractShellMessages.ts'];
const NAMED = /(?<![\w$])\$?t\(\s*(['"])([a-z][a-z0-9]*(?:\.[A-Za-z0-9_-]+)+)\1/g;

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      return sources(path);
    }
    return /\.(ts|vue)$/.test(entry.name) && !/\.test\.ts$/.test(entry.name) ? [path] : [];
  });
}

describe('the catalogue split (RB-099-20)', () => {
  it('moves the settings keys and option names out of the shared catalogue', () => {
    const { shell, pages } = splitCatalogue({
      settings: {
        title: 'Settings',
        key: { sync: { scroll: { label: 'Scroll' } } },
        option: { theme: { dark: 'Dark' } },
      },
      shell: { toolbar: { back: 'Back' } },
    });
    expect(shell).toEqual({ settings: { title: 'Settings' }, shell: { toolbar: { back: 'Back' } } });
    expect(pages).toEqual({
      settings: { key: { sync: { scroll: { label: 'Scroll' } } }, option: { theme: { dark: 'Dark' } } },
    });
  });

  it('leaves a catalogue without those families as it is, and never changes its input', () => {
    const input: Catalogue = { shell: { toolbar: { back: 'Back' } }, settings: 'not a family' };
    const before = JSON.stringify(input);
    const { shell, pages } = splitCatalogue(input);
    expect(shell).toEqual(input);
    expect(pages).toEqual({});
    expect(JSON.stringify(input)).toBe(before);
  });

  it('keeps every key of the catalogue in exactly one half', () => {
    const shell = flatten(shellMessages());
    const pages = flatten(PAGES_CATALOGUE as Catalogue);
    expect(pages.size).toBeGreaterThan(0);
    for (const key of pages.keys()) {
      expect(PAGES_ONLY_PREFIXES.some((prefix) => key.startsWith(`${prefix}.`))).toBe(true);
      expect(shell.has(key)).toBe(false);
    }
    expect(shell.get('shell.toolbar.reload')).toBe('Reload all views');
    expect(pages.has('settings.key.sync.scroll.label')).toBe(true);
  });

  it('holds every key the shell names outside the pages window in the shared catalogue', () => {
    const shell = flatten(shellMessages());
    const missing: string[] = [];
    const misplaced: string[] = [];
    let named = 0;
    for (const path of sources(SRC)) {
      const file = relative(SRC, path).replaceAll('\\', '/');
      const pagesModule = PAGES_MODULES.some((dir) => file.startsWith(dir)) || SENT_TO_CORE.includes(file);
      for (const match of readFileSync(path, 'utf8').matchAll(NAMED)) {
        const key = match[2] ?? '';
        named += 1;
        const pagesOnly = PAGES_ONLY_PREFIXES.some((prefix) => key.startsWith(`${prefix}.`));
        if (pagesOnly && !pagesModule) {
          misplaced.push(`${file}: ${key}`);
        } else if (!pagesOnly && !shell.has(key)) {
          missing.push(`${file}: ${key}`);
        }
      }
    }
    expect(named).toBeGreaterThan(1000);
    expect(misplaced).toEqual([]);
    expect(missing).toEqual([]);
  });
});
