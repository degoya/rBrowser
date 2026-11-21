// The pseudo text of the pseudo-localisation (RB-092-02): a page text made longer by a factor,
// its letters accented and its edges marked with `[` and `]`, so a cut-off text, a font without
// accents and a string built from pieces show at a glance - optionally running right to left.

import type { PseudoLocale } from '../protocol';

/** Unicode RIGHT-TO-LEFT OVERRIDE: what follows runs right to left. */
export const RLO = '\u202E';
/** Unicode POP DIRECTIONAL FORMATTING: ends the override. */
export const PDF = '\u202C';
/** What pads a text to its length. */
const PAD = '~';
/** The factor's range and the default for one that is no number (`rb_config`'s). */
const FACTOR_RANGE: [number, number] = [1, 3];
const DEFAULT_FACTOR = 1.4;

const ACCENTED: Record<string, string> = {
  a: '\u00e1', b: 'ƀ', c: '\u00e7', d: '\u00f0', e: '\u00e9', f: 'ƒ', g: 'ĝ', h: 'ĥ', i: '\u00ee', j: 'ĵ', k: 'ķ', l: 'ļ',
  m: 'ɱ', n: '\u00f1', o: '\u00f6', p: '\u00fe', q: 'ǫ', r: 'ŕ', s: 'š', t: 'ţ', u: '\u00fb', v: 'ṽ', w: 'ŵ', x: 'ẋ',
  y: '\u00fd', z: 'ž',
  A: '\u00c5', B: 'Ɓ', C: '\u00c7', D: '\u00d0', E: '\u00c9', F: 'Ƒ', G: 'Ĝ', H: 'Ĥ', I: '\u00ce', J: 'Ĵ', K: 'Ķ', L: 'Ļ',
  M: 'Ṁ', N: '\u00d1', O: '\u00d6', P: '\u00de', Q: 'Ǫ', R: 'Ŕ', S: 'Š', T: 'Ţ', U: '\u00db', V: 'Ṽ', W: 'Ŵ', X: 'Ẋ',
  Y: '\u00dd', Z: 'Ž',
};

/** `factor` within its range; one that is no number is the default. */
export function clampFactor(factor: number): number {
  if (!Number.isFinite(factor)) return DEFAULT_FACTOR;
  return Math.min(FACTOR_RANGE[1], Math.max(FACTOR_RANGE[0], factor));
}

/** The pseudo text of `text`; white space alone stays as it is, and so does the white space
 *  around a text (the layout's). */
export function pseudoText(text: string, options: PseudoLocale): string {
  const match = /^(\s*)([\s\S]*?)(\s*)$/.exec(text);
  const core = match?.[2] ?? '';
  if (!match || core.length === 0) return text;
  const letters = Array.from(core);
  const body = options.accents ? letters.map((letter) => ACCENTED[letter] ?? letter).join('') : core;
  const target = Math.ceil(letters.length * clampFactor(options.factor));
  const padded = body + PAD.repeat(Math.max(0, target - letters.length));
  const inner = options.rtl ? `${RLO}${padded}${PDF}` : padded;
  return `${match[1]}[${inner}]${match[3]}`;
}
