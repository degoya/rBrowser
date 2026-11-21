// The search result preview of the SEO panel (RB-090-95): the `serp` of the tool's answer
// (plugins/seo/src/serp/mod.rs) checked, and the cut as the search makes it - by the text's width
// in pixels, not its characters, at a word end with an ellipsis. The limits come with the answer;
// the font is a fixed system sans-serif, measured with a canvas where the frame has one. Pure
// functions but `canvasMeasure`.

import type { Texts } from './model';
import { isObject } from '../../../../web-common/src/is-object';

/** A text of the result and the tag (or `domain`) it came from. */
export interface SerpText {
  text: string;
  source: string;
}

/** One device's limits: the text's width in pixels, each text's font size and most lines. */
export interface DeviceLimits {
  width: number;
  titleFontPx: number;
  titleLines: number;
  descriptionFontPx: number;
  descriptionLines: number;
}

/** The limits of both devices, how sure they are, when and where they were checked. */
export interface SerpLimits {
  confidence: 'official' | 'code' | 'unverified';
  checked: string;
  source: string | null;
  desktop: DeviceLimits;
  mobile: DeviceLimits;
}

/** The favicon as the component fetched it; `dataUri` while the answer had room. */
export interface Favicon {
  url: string;
  source: string;
  status: number | null;
  error: string | null;
  format: string | null;
  width: number | null;
  height: number | null;
  bytes: number;
  dataUri: string | null;
  tooLarge: boolean;
}

/** The search result of the page. */
export interface Serp {
  title: SerpText | null;
  description: SerpText | null;
  siteName: SerpText;
  url: string;
  breadcrumb: { host: string; segments: string[] };
  noindex: boolean;
  limits: SerpLimits;
  favicon: Favicon | null;
}

/** A device the panel previews. */
export type Device = 'desktop' | 'mobile';

/** The width of `text` in pixels at `fontPx`. */
export type Measure = (text: string, fontPx: number) => number;

/** A text as the result shows it. */
export interface Fitted {
  /** The lines shown, the last one cut with an ellipsis when the text did not fit. */
  lines: string[];
  /** The lines as one text. */
  text: string;
  /** The whole text, its white space collapsed. */
  full: string;
  truncated: boolean;
  /** The characters of the whole text and of what shows (without the ellipsis). */
  chars: number;
  shownChars: number;
  /** The whole text's width on one line, and the room of every line together. */
  widthPx: number;
  limitPx: number;
}

/** The font the result is measured and shown in: a system sans-serif, no web font. */
export const FONT_FAMILY = 'Arial, Helvetica, sans-serif';
/** The font size of the site name and the breadcrumb. */
export const URL_FONT_PX = 12;
/** The separator of the breadcrumb, as the search writes it. */
export const SEPARATOR = ' › ';
const ELLIPSIS = '…';
const DEVICES: readonly Device[] = ['desktop', 'mobile'];
const CONFIDENCES: readonly string[] = ['official', 'code', 'unverified'];

const isText = (value: unknown): value is SerpText => isObject(value) && typeof value.text === 'string' && typeof value.source === 'string';
const isNumber = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);

function isDevice(value: unknown): value is DeviceLimits {
  return (
    isObject(value) &&
    [value.width, value.titleFontPx, value.titleLines, value.descriptionFontPx, value.descriptionLines].every((one) => isNumber(one) && one > 0)
  );
}

function isFavicon(value: unknown): value is Favicon {
  return isObject(value) && typeof value.url === 'string' && typeof value.source === 'string' && typeof value.tooLarge === 'boolean' && isNumber(value.bytes);
}

/** Whether `value` reads as the answer's `serp` (the frame trusts nothing it is sent). */
export function isSerp(value: unknown): value is Serp {
  if (!isObject(value)) return false;
  const { breadcrumb, limits } = value;
  return (
    (value.title === null || isText(value.title)) &&
    (value.description === null || isText(value.description)) &&
    isText(value.siteName) &&
    typeof value.url === 'string' &&
    isObject(breadcrumb) &&
    typeof breadcrumb.host === 'string' &&
    Array.isArray(breadcrumb.segments) &&
    breadcrumb.segments.every((one) => typeof one === 'string') &&
    typeof value.noindex === 'boolean' &&
    isObject(limits) &&
    CONFIDENCES.includes(String(limits.confidence)) &&
    typeof limits.checked === 'string' &&
    (limits.source === null || typeof limits.source === 'string') &&
    isDevice(limits.desktop) &&
    isDevice(limits.mobile) &&
    (value.favicon === null || isFavicon(value.favicon))
  );
}

/** The devices the setting `seo.serpPreview` shows: `both` (the default, and for a value it
 * does not know), `desktop`, `mobile` or `off`. */
export function devicesShown(setting: unknown): Device[] {
  if (setting === 'off') return [];
  if (setting === 'desktop' || setting === 'mobile') return [setting];
  return [...DEVICES];
}

/** Only an inline raster or ICO image may become the favicon's `<img>`: the frame's CSP allows
 * `data:` alone, and an SVG is markup. */
export function safeFavicon(favicon: Favicon | null): string | null {
  const uri = favicon?.dataUri ?? null;
  return uri !== null && /^data:image\/(png|jpeg|gif|webp|x-icon);base64,[A-Za-z0-9+/=]+$/.test(uri) ? uri : null;
}

/** The breadcrumb as the result writes it: `host › segment › segment`. */
export function breadcrumbText(breadcrumb: Serp['breadcrumb']): string {
  return [breadcrumb.host, ...breadcrumb.segments].join(SEPARATOR);
}

/** A width guessed from the characters when there is no canvas: about half the font size each. */
export const estimateMeasure: Measure = (text, fontPx) => [...text].length * fontPx * 0.5;

/** The width of a text in `FONT_FAMILY`, measured by a canvas; the estimate without one. */
export function canvasMeasure(): Measure {
  let context: CanvasRenderingContext2D | null = null;
  try {
    context = document.createElement('canvas').getContext('2d');
  } catch {
    context = null;
  }
  if (!context || typeof context.measureText !== 'function') return estimateMeasure;
  const canvas = context;
  return (text, fontPx) => {
    canvas.font = `${fontPx}px ${FONT_FAMILY}`;
    const width = canvas.measureText(text).width;
    return Number.isFinite(width) && (width > 0 || text === '') ? width : estimateMeasure(text, fontPx);
  };
}

/** `word` broken into pieces of at most `max` px (one character at least each). */
function breakWord(word: string, max: number, px: number, measure: Measure): string[] {
  const pieces: string[] = [];
  let piece = '';
  for (const char of word) {
    if (piece !== '' && measure(piece + char, px) > max) {
      pieces.push(piece);
      piece = char;
    } else {
      piece += char;
    }
  }
  if (piece !== '') pieces.push(piece);
  return pieces;
}

/** `text` wrapped at word ends into lines of at most `max` px, as many as it needs. */
function wrap(text: string, max: number, px: number, measure: Measure): string[] {
  const lines: string[] = [];
  let current = '';
  for (const word of text.split(' ').filter((one) => one !== '')) {
    const joined = current === '' ? word : `${current} ${word}`;
    if (measure(joined, px) <= max) {
      current = joined;
      continue;
    }
    if (current !== '') lines.push(current);
    const pieces = measure(word, px) <= max ? [word] : breakWord(word, max, px, measure);
    lines.push(...pieces.slice(0, -1));
    current = pieces.at(-1) ?? '';
  }
  if (current !== '') lines.push(current);
  return lines;
}

/** `line` with the ellipsis, shortened by whole words (by characters within one word) until it
 * fits in `max` px. */
function ellipsize(line: string, max: number, px: number, measure: Measure): string {
  let rest = line;
  while (rest !== '' && measure(rest + ELLIPSIS, px) > max) {
    const space = rest.lastIndexOf(' ');
    rest = space > 0 ? rest.slice(0, space).trimEnd() : [...rest].slice(0, -1).join('');
  }
  return rest + ELLIPSIS;
}

/** `text` as the result shows it in `lines` lines of `max` px at `px`: wrapped at word ends,
 * the last line cut with an ellipsis when the text does not fit. */
export function fit(text: string, max: number, lines: number, px: number, measure: Measure): Fitted {
  const full = text.split(/\s+/u).filter((one) => one !== '').join(' ');
  const wrapped = wrap(full, max, px, measure);
  const truncated = wrapped.length > lines;
  const shown = wrapped.slice(0, lines);
  if (truncated && shown.length > 0) shown[shown.length - 1] = ellipsize(shown.at(-1) ?? '', max, px, measure);
  const joined = shown.join(' ');
  return {
    lines: shown,
    text: joined,
    full,
    truncated,
    chars: [...full].length,
    shownChars: [...(truncated ? joined.slice(0, -ELLIPSIS.length) : joined)].length,
    widthPx: Math.round(measure(full, px)),
    limitPx: max * lines,
  };
}

/** Why the favicon does not show (its tooltip): none found, the status it answered or the
 * sentence of the failure (`say`), too large, or no image of a format the frame shows; empty
 * when it shows. */
export function faviconNote(t: Texts, favicon: Favicon | null, say: (code: string) => string = (code) => code): string {
  if (safeFavicon(favicon) !== null) return '';
  if (!favicon) return t('panel.serp.favicon_none');
  if (favicon.status !== 200) {
    return t('panel.serp.favicon_failed', { reason: favicon.status !== null ? favicon.status : say(favicon.error ?? '') });
  }
  if (favicon.tooLarge) return t('panel.serp.favicon_large', { bytes: favicon.bytes });
  return t('panel.serp.favicon_format');
}
