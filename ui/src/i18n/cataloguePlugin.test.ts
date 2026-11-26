import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { catalogueModule, cataloguePlugin, PAGES_CATALOGUE_ID, SHELL_CATALOGUE_ID } from './cataloguePlugin';

type Hook = (this: { addWatchFile: (file: string) => void }, id: string) => unknown;

/** The module's default export: the source must be exactly one `JSON.parse` of a string literal. */
function evaluate(source: string): unknown {
  const literal = /^export default JSON\.parse\(("(?:[^"\\]|\\.)*")\);\n$/s.exec(source)?.[1];
  expect(literal).toBeDefined();
  return JSON.parse(JSON.parse(literal ?? '""') as string) as unknown;
}

function catalogueFile(text: string): string {
  const file = join(mkdtempSync(join(tmpdir(), 'rb-catalogue-')), 'en.json');
  writeFileSync(file, text);
  return file;
}

describe('the catalogue plugin (RB-099-20)', () => {
  it('writes a catalogue as one JSON.parse of a string, quotes and all', () => {
    const catalogue = { shell: { quote: 'Say "{name}" \\ `now` ${x}', line: 'a\nb c' } };
    const source = catalogueModule(catalogue);
    expect(source.startsWith('export default JSON.parse("')).toBe(true);
    expect(evaluate(source)).toEqual(catalogue);
  });

  it('serves the shared half and the pages half of the catalogue file', () => {
    const file = catalogueFile(
      JSON.stringify({ settings: { title: 'Settings', key: { a: { label: 'A' } } }, shell: { back: 'Back' } }),
    );
    const plugin = cataloguePlugin(file);
    const resolve = plugin.resolveId as unknown as (id: string) => string | null;
    const load = plugin.load as unknown as Hook;
    const watched: string[] = [];
    const context = { addWatchFile: (path: string) => watched.push(path) };
    const shellId = resolve(SHELL_CATALOGUE_ID);
    const pagesId = resolve(PAGES_CATALOGUE_ID);
    expect(shellId).not.toBeNull();
    expect(resolve('vue')).toBeNull();
    expect(evaluate(load.call(context, shellId ?? '') as string)).toEqual({
      settings: { title: 'Settings' },
      shell: { back: 'Back' },
    });
    expect(evaluate(load.call(context, pagesId ?? '') as string)).toEqual({ settings: { key: { a: { label: 'A' } } } });
    expect(load.call(context, 'other.ts')).toBeNull();
    expect(watched).toEqual([file, file]);
  });

  it('refuses a catalogue that is not an object', () => {
    const plugin = cataloguePlugin(catalogueFile('[]'));
    const load = plugin.load as unknown as Hook;
    expect(() => load.call({ addWatchFile: () => undefined }, `\0${SHELL_CATALOGUE_ID}`)).toThrow(/not a JSON object/);
  });
});
