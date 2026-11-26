// The pages window's half of the catalogue (RB-099-20, `split.ts`): the settings keys' labels,
// descriptions, help texts and option names. Only the pages window's entry (`settings-main.ts`)
// imports this module, so no other page parses them.

import catalogue from 'virtual:rb-catalogue/pages';
import { createShellI18n } from './index';

export const PAGES_CATALOGUE: unknown = catalogue;

/** The pages window's i18n: the shared catalogue with the pages-only half. */
export function createPagesI18n(): ReturnType<typeof createShellI18n> {
  return createShellI18n([PAGES_CATALOGUE]);
}
