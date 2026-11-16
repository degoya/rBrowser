// The page's web fonts (RB-070-13): every `@font-face` rule of the stylesheets the page may read
// (inside `@media`, `@supports`, `@layer` and `@import` too), joined with `document.fonts`, which
// knows the faces of the sheets it may not read (Google Fonts' CSS is cross-origin) and whether a
// face was ever needed (`unloaded`), loaded or failed.

import type { Face, Src } from './types';

/** The most faces kept. */
export const MAX_FACES = 200;
/** The longest family, weight, style or range kept, in characters. */
const MAX_SHORT = 120;
/** The longest URL kept, in characters. */
export const MAX_URL = 2048;

/** A `document.fonts` entry, as the page part reads it. */
export interface FaceLike {
  family: string;
  weight: string;
  style: string;
  display: string;
  unicodeRange: string;
  status: string;
}

const FORMATS: Record<string, string> = {
  woff2: 'woff2',
  woff: 'woff',
  ttf: 'truetype',
  otf: 'opentype',
  eot: 'embedded-opentype',
  svg: 'svg',
};

export function cut(text: string, max: number): string {
  return text.length > max ? text.slice(0, max) : text;
}

/** `"Inter ASCII"` -> `Inter ASCII`. */
export function unquote(text: string): string {
  const trimmed = text.trim();
  const quoted = /^(["'])(.*)\1$/.exec(trimmed);
  return quoted ? (quoted[2] ?? '') : trimmed;
}

/** The first family of a `font-family` stack, unquoted. */
export function firstFamily(stack: string): string {
  return unquote(splitList(stack)[0] ?? '');
}

/** The weight as a number: `normal` 400, `bold` 700; a range's lower end. */
export function weightOf(value: string): number {
  const text = value.trim().toLowerCase();
  if (text === '' || text === 'normal') return 400;
  if (text === 'bold') return 700;
  const number = Number.parseFloat(text);
  return Number.isFinite(number) ? Math.round(number) : 400;
}

/** A weight as text: `normal` `400`, `bold` `700`, a range kept (`100 900`). */
function weightText(value: string): string {
  const text = value.trim().toLowerCase();
  if (text === '' || text === 'normal') return '400';
  if (text === 'bold') return '700';
  return cut(text, MAX_SHORT);
}

function styleText(value: string): string {
  return cut(value.trim().toLowerCase() || 'normal', MAX_SHORT);
}

/** The comma-separated entries of a CSS value, commas inside quotes and parentheses kept. */
export function splitList(value: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let quote = '';
  let current = '';
  for (const char of value) {
    if (quote) {
      if (char === quote) quote = '';
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === '(') {
      depth += 1;
    } else if (char === ')') {
      depth = Math.max(0, depth - 1);
    } else if (char === ',' && depth === 0) {
      parts.push(current.trim());
      current = '';
      continue;
    }
    current += char;
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

function absolute(url: string, base: string): string {
  try {
    return cut(new URL(url, base).href, MAX_URL);
  } catch {
    return cut(url, MAX_URL);
  }
}

/** The format of a URL by its extension, when no `format()` names it. */
export function formatOf(url: string): string {
  const path = url.split(/[?#]/)[0] ?? '';
  const extension = /\.([a-z0-9]+)$/i.exec(path)?.[1]?.toLowerCase() ?? '';
  return FORMATS[extension] ?? '';
}

/** A `src` descriptor as a list of sources, the URLs made absolute against `base`. */
export function parseSrc(value: string, base: string): Src[] {
  return splitList(value).flatMap((entry): Src[] => {
    const local = /^local\(\s*(.*?)\s*\)/i.exec(entry);
    if (local) return [{ url: cut(unquote(local[1] ?? ''), MAX_SHORT), format: '', local: true }];
    const url = /^url\(\s*(.*?)\s*\)/i.exec(entry);
    if (!url) return [];
    const href = absolute(unquote(url[1] ?? ''), base);
    const hint = /format\(\s*(.*?)\s*\)/i.exec(entry)?.[1];
    const format = hint ? unquote(hint).toLowerCase() : formatOf(href);
    return [{ url: href, format: cut(format, 40), local: false }];
  });
}

interface RuleFace {
  face: Face;
  matched: boolean;
}

function readRule(rule: CSSFontFaceRule, sheet: string, base: string): Face {
  const style = rule.style;
  const weight = weightText(style.getPropertyValue('font-weight'));
  const src = parseSrc(style.getPropertyValue('src'), base);
  const variations = src.some((source) => source.format.includes('variations'));
  return {
    family: cut(unquote(style.getPropertyValue('font-family')), MAX_SHORT),
    weight,
    style: styleText(style.getPropertyValue('font-style')),
    display: cut(style.getPropertyValue('font-display').trim().toLowerCase() || 'auto', 20),
    unicodeRange: cut(style.getPropertyValue('unicode-range').trim(), MAX_SHORT),
    status: 'unknown',
    src,
    sheet,
    readable: true,
    variable: variations || weight.includes(' '),
  };
}

/** Every `@font-face` rule below `rules`, with the stylesheets it could not read. */
function walkRules(rules: CSSRuleList, sheet: string, base: string, faces: RuleFace[], unreadable: string[], depth: number): void {
  for (const rule of Array.from(rules)) {
    if (faces.length >= MAX_FACES) return;
    if (rule.constructor.name === 'CSSFontFaceRule' || rule.type === 5) {
      faces.push({ face: readRule(rule as CSSFontFaceRule, sheet, base), matched: false });
    } else if ('styleSheet' in rule && (rule as CSSImportRule).styleSheet && depth < 8) {
      readSheet((rule as CSSImportRule).styleSheet as CSSStyleSheet, faces, unreadable, depth + 1);
    } else if ('cssRules' in rule && depth < 8) {
      walkRules((rule as CSSGroupingRule).cssRules, sheet, base, faces, unreadable, depth + 1);
    }
  }
}

function readSheet(sheet: CSSStyleSheet, faces: RuleFace[], unreadable: string[], depth: number): void {
  const name = sheet.href ? cut(sheet.href, MAX_URL) : 'inline';
  let rules: CSSRuleList;
  try {
    rules = sheet.cssRules;
  } catch {
    if (sheet.href) unreadable.push(name);
    return;
  }
  walkRules(rules, name, sheet.href ?? document.baseURI, faces, unreadable, depth);
}

function sameFace(face: Face, entry: FaceLike): boolean {
  return (
    face.family.toLowerCase() === unquote(entry.family).toLowerCase() &&
    face.weight === weightText(entry.weight) &&
    face.style === styleText(entry.style)
  );
}

/** The sheet a face only `document.fonts` knows comes from: the one sheet the page may not
 * read, or the one naming its family in a Google Fonts URL; empty when that cannot tell. */
function sheetOf(family: string, unreadable: string[]): string {
  if (unreadable.length === 1) return unreadable[0] ?? '';
  const name = encodeURIComponent(family).replace(/%20/g, '+').toLowerCase();
  return unreadable.find((href) => href.toLowerCase().includes(`family=${name}`)) ?? '';
}

/** The page's faces: its readable rules, each with its `document.fonts` status, then the
 * entries no rule explains. `extraUnreadable` names sheets the caller knows are unreadable. */
export function readFaces(entries: FaceLike[], extraUnreadable: string[] = []): { faces: Face[]; unreadable: string[] } {
  const ruleFaces: RuleFace[] = [];
  const unreadable: string[] = [...extraUnreadable];
  for (const sheet of Array.from(document.styleSheets)) {
    readSheet(sheet as CSSStyleSheet, ruleFaces, unreadable, 0);
  }
  const extra: Face[] = [];
  for (const entry of entries) {
    const rule = ruleFaces.find((candidate) => !candidate.matched && sameFace(candidate.face, entry));
    if (rule) {
      rule.matched = true;
      rule.face.status = cut(entry.status, 20);
      continue;
    }
    const family = cut(unquote(entry.family), MAX_SHORT);
    const weight = weightText(entry.weight);
    extra.push({
      family,
      weight,
      style: styleText(entry.style),
      display: cut(entry.display.trim().toLowerCase() || 'auto', 20),
      unicodeRange: cut(entry.unicodeRange, MAX_SHORT),
      status: cut(entry.status, 20),
      src: [],
      sheet: sheetOf(family, unreadable),
      readable: false,
      variable: weight.includes(' '),
    });
  }
  const faces = [...ruleFaces.map((rule) => rule.face), ...extra].slice(0, MAX_FACES);
  return { faces, unreadable: unreadable.slice(0, 50) };
}
