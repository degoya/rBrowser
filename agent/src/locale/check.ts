// The check of the core's `locale` command (RB-092-02): what reaches the page is a known
// direction, a string as language (`apply.ts` takes only a BCP 47 tag of it) and a
// pseudo-localisation of numbers and switches.

import { isObject } from '../../../plugins/web-common/src/is-object';
import type { PageLocale } from '../protocol';

const DIRECTIONS = ['ltr', 'rtl', 'auto'];

/** Whether `value` reads as a `PageLocale`. */
export function isPageLocale(value: unknown): value is PageLocale {
  if (!isObject(value)) return false;
  const { dir, lang, pseudo } = value;
  if (dir !== undefined && !(typeof dir === 'string' && DIRECTIONS.includes(dir))) return false;
  if (lang !== undefined && typeof lang !== 'string') return false;
  if (pseudo === undefined) return true;
  return (
    isObject(pseudo) &&
    typeof pseudo.factor === 'number' &&
    Number.isFinite(pseudo.factor) &&
    typeof pseudo.accents === 'boolean' &&
    typeof pseudo.rtl === 'boolean'
  );
}
