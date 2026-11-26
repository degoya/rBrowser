// The Vite plugin behind the catalogue's two halves (RB-099-20, `split.ts`): it reads
// `locales/en.json` at build time and serves `virtual:rb-catalogue/shell`, which every page
// imports, and `virtual:rb-catalogue/pages`, which only the pages window's entry imports. Each is
// one `JSON.parse` of a string: V8 reads a JSON string much faster than the same object literal.
// The catalogue's format stays as it is (docs/i18n.md); only the bundle is cut.

import { readFileSync } from 'node:fs';
import type { Plugin } from 'vite';
import { isObject } from '../../../plugins/web-common/src/is-object.ts';
import type { Catalogue } from './messages.ts';
import { splitCatalogue } from './split.ts';

export const SHELL_CATALOGUE_ID = 'virtual:rb-catalogue/shell';
export const PAGES_CATALOGUE_ID = 'virtual:rb-catalogue/pages';
const RESOLVED = '\0';

/** The source of a module whose default export is `catalogue`, parsed from a JSON string (the
 * minifier picks the string's quotes). */
export function catalogueModule(catalogue: Catalogue): string {
  return `export default JSON.parse(${JSON.stringify(JSON.stringify(catalogue))});\n`;
}

/** The plugin over the catalogue at `file` (`locales/en.json`). */
export function cataloguePlugin(file: string): Plugin {
  const halves = (): ReturnType<typeof splitCatalogue> => {
    const parsed: unknown = JSON.parse(readFileSync(file, 'utf8'));
    if (!isObject(parsed)) {
      throw new Error(`${file}: the catalogue is not a JSON object`);
    }
    return splitCatalogue(parsed as Catalogue);
  };
  return {
    name: 'rb-catalogue',
    resolveId(id) {
      return id === SHELL_CATALOGUE_ID || id === PAGES_CATALOGUE_ID ? RESOLVED + id : null;
    },
    load(id) {
      if (id !== RESOLVED + SHELL_CATALOGUE_ID && id !== RESOLVED + PAGES_CATALOGUE_ID) {
        return null;
      }
      this.addWatchFile(file);
      const { shell, pages } = halves();
      return catalogueModule(id === RESOLVED + SHELL_CATALOGUE_ID ? shell : pages);
    },
  };
}
