// A colour setting's value (RB-090-58): `#rrggbb`, or `#rrggbbaa` where the schema's `pattern`
// takes eight digits - the last two the opacity. The colour field's picker sets the hue, its
// slider the opacity in per cent; fully opaque is written with six digits.

import type { JsonSchema } from './schema';

export interface ParsedColor {
  /** `#rrggbb`, lower case. */
  rgb: string;
  /** 0 (transparent) to 255 (opaque). */
  alpha: number;
}

const HEX = /^#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?$/;

/** The colour `value` names; `null` for anything but `#rrggbb` or `#rrggbbaa`. */
export function parseColor(value: unknown): ParsedColor | null {
  if (typeof value !== 'string') return null;
  const match = HEX.exec(value);
  if (!match) return null;
  return { rgb: `#${match[1]?.toLowerCase()}`, alpha: match[2] === undefined ? 255 : parseInt(match[2], 16) };
}

function format(rgb: string, alpha: number): string {
  return alpha >= 255 ? rgb : `${rgb}${alpha.toString(16).padStart(2, '0')}`;
}

/** `current` with the hue `rgb` (the picker's `#RRGGBB`) and its opacity, where allowed. */
export function withRgb(current: string, rgb: string, alpha: boolean): string {
  const picked = parseColor(rgb);
  if (!picked) return current;
  return format(picked.rgb, alpha ? (parseColor(current)?.alpha ?? 255) : 255);
}

/** The opacity of `value` in per cent; 100 where it is no colour. */
export function opacityOf(value: string): number {
  return Math.round(((parseColor(value)?.alpha ?? 255) / 255) * 100);
}

/** `current` at `percent` opacity (0 to 100); black where it is no colour yet. */
export function withOpacity(current: string, percent: number): string {
  const rgb = parseColor(current)?.rgb ?? '#000000';
  const clamped = Math.min(100, Math.max(0, percent));
  return format(rgb, Math.round((clamped / 100) * 255));
}

/** Whether the setting takes an opacity: its `pattern` accepts eight hex digits. */
export function allowsAlpha(schema: JsonSchema): boolean {
  const pattern = schema.pattern;
  if (typeof pattern !== 'string') return false;
  try {
    return new RegExp(pattern).test('#00000000');
  } catch {
    return false;
  }
}
