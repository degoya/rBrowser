// The overlay's colours in the panel (RB-090-58): the six elements, in the order the panel shows
// them, and a colour's value - `#rrggbb`, or `#rrggbbaa` with the opacity in the last two digits.
// The picker sets the hue, the slider the opacity in per cent; fully opaque is six digits.
// (`ui/src/settings/colorValue.ts` is the settings window's twin; a panel imports nothing of it.)

export const COLOR_KEYS = ['ruler', 'ticks', 'guide', 'columnFill', 'columnEdge', 'baseline'] as const;
export type ColorKey = (typeof COLOR_KEYS)[number];

const HEX = /^#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?$/;

/** Whether `value` is `#rrggbb` or `#rrggbbaa`. */
export const isColor = (value: unknown): value is string => typeof value === 'string' && HEX.test(value);

function parts(value: string): { rgb: string; alpha: number } | null {
  const match = HEX.exec(value);
  if (!match) return null;
  return { rgb: `#${match[1]?.toLowerCase()}`, alpha: match[2] === undefined ? 255 : parseInt(match[2], 16) };
}

const format = (rgb: string, alpha: number): string =>
  alpha >= 255 ? rgb : `${rgb}${alpha.toString(16).padStart(2, '0')}`;

/** The hue of `value`, `#rrggbb`; black where it is no colour. */
export const hueOf = (value: string): string => parts(value)?.rgb ?? '#000000';

/** `current` with the picker's hue `rgb`, its opacity kept. */
export function withRgb(current: string, rgb: string): string {
  const picked = parts(rgb);
  return picked ? format(picked.rgb, parts(current)?.alpha ?? 255) : current;
}

/** The opacity of `value` in per cent. */
export const opacityOf = (value: string): number => Math.round(((parts(value)?.alpha ?? 255) / 255) * 100);

/** `current` at `percent` opacity (0 to 100). */
export function withOpacity(current: string, percent: number): string {
  const clamped = Math.min(100, Math.max(0, percent));
  return format(hueOf(current), Math.round((clamped / 100) * 255));
}

/** One colour as `set` takes it. */
export const colorArg = (key: ColorKey, value: string): { colors: Partial<Record<ColorKey, string>> } => ({
  colors: { [key]: value },
});
