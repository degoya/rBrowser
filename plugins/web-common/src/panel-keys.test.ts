import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// RB-080-59: a panel never shows a raw key. Every key a plugin's panel names - in a `data-text`
// (or `data-label`, `data-title`, `data-placeholder`) attribute, as a quoted string or as the fixed
// start of a template string - is in the plugin's English locale. A key is relative to the
// plugin's `plugin.<id>.` prefix or written out in full. Only names whose first part is a
// section of the plugin's own texts count (`panel.css` is a file, `window.parent` code).

const PLUGINS = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const ATTRIBUTE = /data-(?:text|label|title|placeholder)="([^"]+)"/g;
const QUOTED = /['"`]([a-z][a-z0-9_-]*(?:\.[a-z0-9_-]+)+)['"`]/g;
const TEMPLATE = /`([a-z][a-z0-9_-]*(?:\.[a-z0-9_-]+)*\.[a-z0-9_-]*)\$\{/g;
const FILE = /\.(?:js|mjs|css|html|json|png|svg|ttf|txt)$/;

function flatten(value: unknown, prefix: string, into: Set<string>): Set<string> {
  if (typeof value === 'string') {
    into.add(prefix);
  } else if (value !== null && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      flatten(child, prefix === '' ? key : `${prefix}.${key}`, into);
    }
  }
  return into;
}

function filesUnder(folder: string): string[] {
  return readdirSync(folder).flatMap((name) => {
    const path = join(folder, name);
    if (statSync(path).isDirectory()) return filesUnder(path);
    return /\.(?:js|html)$/.test(name) ? [path] : [];
  });
}

const all = (pattern: RegExp, text: string): string[] => [...text.matchAll(pattern)].map((match) => match[1] ?? '');

interface Panel {
  id: string;
  keys: Set<string>;
  sources: { path: string; text: string }[];
}

/** Every plugin with a `panel/` folder and an English locale. */
function panels(): Panel[] {
  return readdirSync(PLUGINS)
    .filter((id) => existsSync(join(PLUGINS, id, 'panel')) && existsSync(join(PLUGINS, id, 'locales', 'en.json')))
    .map((id) => ({
      id,
      keys: flatten(JSON.parse(readFileSync(join(PLUGINS, id, 'locales', 'en.json'), 'utf8')), '', new Set()),
      sources: filesUnder(join(PLUGINS, id, 'panel')).map((path) => ({ path, text: readFileSync(path, 'utf8') })),
    }));
}

/** The keys `panel` names and those of them its locale lacks. */
function check(panel: Panel): { named: number; missing: string[] } {
  const prefix = `plugin.${panel.id}.`;
  const own = [...panel.keys].filter((key) => key.startsWith(prefix)).map((key) => key.slice(prefix.length));
  const sections = new Set(own.map((key) => key.split('.')[0]));
  const relative = (name: string): string | null => {
    if (name.startsWith(prefix)) return name.slice(prefix.length);
    if (name.startsWith('plugin.')) return null;
    return sections.has(name.split('.')[0]) && !FILE.test(name) ? name : null;
  };
  let named = 0;
  const missing = new Set<string>();
  for (const { path, text } of panel.sources) {
    const where = path.slice(PLUGINS.length + 1);
    for (const name of [...all(ATTRIBUTE, text), ...all(QUOTED, text)]) {
      const key = relative(name);
      if (key === null) continue;
      named += 1;
      if (!own.includes(key)) missing.add(`${where}: ${key}`);
    }
    for (const start of all(TEMPLATE, text)) {
      const key = relative(start);
      if (key === null) continue;
      named += 1;
      if (!own.some((candidate) => candidate.startsWith(key))) missing.add(`${where}: ${key}\${...}`);
    }
  }
  return { named, missing: [...missing].sort() };
}

describe('the plugin panels\' texts', () => {
  const found = panels();

  it('finds the panels of the first-party plugins', () => {
    expect(found.map((panel) => panel.id)).toEqual(
      expect.arrayContaining(['rulers-guides', 'screenshot-editor', 'simulators', 'storage-panel']),
    );
  });

  it.each(found.map((panel) => [panel.id, panel] as const))('every key the %s panel names is in its locale', (_, panel) => {
    const { named, missing } = check(panel);
    expect(named, 'the scan reads the panel').toBeGreaterThan(0);
    expect(missing).toEqual([]);
  });

  it('notices a key the locale lacks', () => {
    const panel: Panel = {
      id: 'demo',
      keys: new Set(['plugin.demo.panel.title', 'plugin.demo.panel.tool.rect']),
      sources: [
        {
          path: join(PLUGINS, 'demo', 'panel', 'index.html'),
          text: '<h1 data-text="panel.title"></h1><p data-text="panel.gone"></p><link href="panel.css">',
        },
        {
          path: join(PLUGINS, 'demo', 'panel', 'panel.js'),
          text: "text('panel.missing'); text(`panel.tool.${x}`); text(`panel.shape.${x}`); text('plugin.demo.panel.title');",
        },
      ],
    };
    expect(check(panel).missing).toEqual([
      'demo/panel/index.html: panel.gone',
      'demo/panel/panel.js: panel.missing',
      'demo/panel/panel.js: panel.shape.${...}',
    ]);
  });
});
