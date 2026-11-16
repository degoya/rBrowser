// The colour picker panel's pure part (RB-070-10, on Nuxt UI since RB-080-86): the formats in
// the person's order, the shapes of the tools' answers, and which text a failure code has.

export const FORMATS = ['hex', 'rgb', 'hsl', 'oklch'] as const;
export type Format = (typeof FORMATS)[number];
export type Formats = Record<Format, string>;
export type Slot = 'foreground' | 'background';
export const SLOTS: readonly Slot[] = ['foreground', 'background'];

export interface Levels {
  aa?: boolean;
  aaa?: boolean;
}

/** The tool `contrast`'s answer. */
export interface Rating {
  ratio: number;
  normal?: Levels;
  large?: Levels;
  foreground?: Formats;
  background?: Formats;
}

/** One colour of the tool `palette`'s answer. */
export interface PaletteGroup {
  hex: string;
  count: number;
  members?: string[];
}

/** One view of the tool `measure_text`'s answer. */
export interface MeasureView {
  view: number;
  label?: string;
  lines?: number;
  average?: number;
  max?: number;
  over?: number;
  rating?: string;
  error?: string;
}

export interface Measured {
  selector: string;
  optimal?: { min?: number; max?: number };
  warnAbove?: number;
  views: MeasureView[];
}

/** The formats with the default format (setting `color-picker.defaultFormat`) first. */
export function order(first: unknown): Format[] {
  const wanted = String(first ?? 'hex');
  return [...FORMATS].sort((a, b) => Number(b === wanted) - Number(a === wanted));
}

/** A complete contrast answer, else null (a failed or partial one changes nothing). */
export function asRating(answer: unknown): Rating | null {
  const rating = answer as Rating | null;
  return rating && typeof rating === 'object' && rating.foreground && rating.background ? rating : null;
}

/** A text measure result, else null. */
export function asMeasured(result: unknown): Measured | null {
  if (!result || typeof result !== 'object' || !Array.isArray((result as Measured).views)) return null;
  return result as Measured;
}

/** The full key of a failure code's text - the code itself or below the plugin's prefix - or
 * null when the catalogue has none (the panel then says its own sentence, never the code). */
export function failureKey(code: string, strings: Record<string, string>, prefix: string): string | null {
  if (code in strings) return code;
  if (prefix + code in strings) return prefix + code;
  return null;
}

/** Nuxt UI's colour of a text measure rating; a level passed or failed is said in words too. */
export const RATING_COLOR: Record<string, 'success' | 'error' | 'neutral'> = {
  optimal: 'success',
  short: 'error',
  long: 'error',
  none: 'neutral',
};
