// The catalogue's split (RB-099-20). The toolbar, the status bar, the canvas board and the pages
// window share one chunk, so every text it holds is parsed by every page. The labels,
// descriptions and help texts of the settings keys and the names of their options - over 40 % of
// `locales/en.json` - are looked up only by the pages window (the settings page and the palette),
// through the schema the core sends it; they go into the pages window's own entry instead
// (`pages.ts`). The core renders none of them for another page, so nothing else needs them; the
// test beside this file holds the shell's named keys to the shared half.

import type { Catalogue } from './messages';

/** The key families only the pages window looks up. */
export const PAGES_ONLY_PREFIXES: readonly string[] = ['settings.key', 'settings.option'];

export interface SplitCatalogue {
  /** Every text but the pages-only families: what every page loads. */
  shell: Catalogue;
  /** The pages-only families, nested as in the catalogue. */
  pages: Catalogue;
}

function copy(catalogue: Catalogue): Catalogue {
  return JSON.parse(JSON.stringify(catalogue)) as Catalogue;
}

/** `catalogue` in its two halves; the input stays as it is. */
export function splitCatalogue(catalogue: Catalogue): SplitCatalogue {
  const shell = copy(catalogue);
  const pages: Catalogue = {};
  for (const prefix of PAGES_ONLY_PREFIXES) {
    const segments = prefix.split('.');
    const last = segments.pop() ?? '';
    let from: Catalogue | undefined = shell;
    let to: Catalogue = pages;
    for (const segment of segments) {
      const next: string | Catalogue | undefined = from?.[segment];
      from = typeof next === 'object' ? next : undefined;
      const existing = to[segment];
      const child: Catalogue = typeof existing === 'object' ? existing : {};
      to[segment] = child;
      to = child;
    }
    const family = from?.[last];
    if (from !== undefined && typeof family === 'object') {
      to[last] = family;
      delete from[last];
    }
  }
  return { shell, pages: prune(pages) };
}

/** `catalogue` without the objects that ended up empty (a family the catalogue lacks). */
function prune(catalogue: Catalogue): Catalogue {
  const kept: Catalogue = {};
  for (const [segment, value] of Object.entries(catalogue)) {
    if (typeof value === 'string') {
      kept[segment] = value;
    } else {
      const inner = prune(value);
      if (Object.keys(inner).length > 0) {
        kept[segment] = inner;
      }
    }
  }
  return kept;
}
