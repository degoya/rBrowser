// The screenshot editor's controls (RB-080-63, on Nuxt UI since RB-080-86): the options beside
// the tools belong to the chosen tool only - colour, line width, fill, text size, redaction mode
// and block size -, read as drawing options within their ranges, with the plugin settings'
// starting values. Also the zoom steps and a failure's reason in words.
import type { ShotOptions } from './types';

const SETTING = 'screenshot-editor.';
export const DEFAULT_COLOR = '#e53935';

/** Each option's range and the tools it belongs to. */
export const RANGES = { width: [1, 64], size: [6, 256], block: [8, 128] } as const;
export type OptionName = 'color' | 'width' | 'size' | 'fill' | 'mode' | 'block' | 'crop';
const OPTION_TOOLS: Record<OptionName, readonly string[]> = {
  color: ['rect', 'arrow', 'line', 'freehand', 'text', 'marker', 'highlight'],
  width: ['rect', 'arrow', 'line', 'freehand'],
  size: ['text', 'marker'],
  fill: ['rect'],
  mode: ['redact'],
  block: ['redact'],
  crop: ['crop'],
};

/** The options `tool` shows, in their order; the block size only for a mosaic or a blur. */
export function shownOptions(tool: string, mode: string): OptionName[] {
  return (Object.keys(OPTION_TOOLS) as OptionName[]).filter((name) => OPTION_TOOLS[name].includes(tool) && !(name === 'block' && mode === 'black'));
}

/** The redaction modes, as the component names them. */
export const MODES = ['pixelate', 'blur', 'black'] as const;

/** The colours offered as swatches (the picker takes any other). */
export const SWATCHES = [
  ['red', '#e53935'],
  ['orange', '#fb8c00'],
  ['yellow', '#fdd835'],
  ['green', '#43a047'],
  ['blue', '#1e88e5'],
  ['purple', '#8e24aa'],
  ['black', '#000000'],
  ['white', '#ffffff'],
] as const;

const whole = (value: unknown, [low, high]: readonly [number, number], fallback: number): number => {
  const number = Math.round(Number(value));
  return Number.isFinite(number) ? Math.min(Math.max(number, low), high) : fallback;
};

/** The drawing options from the controls' values, each within its range. */
export function options(values: { color: string; width: unknown; size: unknown; mode: string; block: unknown; fill: boolean }): ShotOptions {
  return {
    color: /^#[0-9a-fA-F]{6}$/.test(values.color) ? values.color.toLowerCase() : DEFAULT_COLOR,
    width: whole(values.width, RANGES.width, 4),
    size: whole(values.size, RANGES.size, 20),
    mode: (MODES as readonly string[]).includes(values.mode) ? values.mode : 'pixelate',
    block: whole(values.block, RANGES.block, 12),
    fill: values.fill,
  };
}

/** The plugin settings' starting values for the controls that have one. */
export function fromSettings(settings: Record<string, unknown>): Partial<{ color: string; width: number; size: number; block: number }> {
  const out: Partial<{ color: string; width: number; size: number; block: number }> = {};
  const value = (key: string): unknown => settings[SETTING + key];
  const color = value('color');
  if (typeof color === 'string' && /^#[0-9a-fA-F]{6}$/.test(color)) out.color = color.toLowerCase();
  for (const [name, key] of [['width', 'strokeWidth'], ['size', 'fontSize'], ['block', 'redactBlock']] as const) {
    const given = value(key);
    if (given !== undefined && given !== null && Number.isFinite(Number(given))) out[name] = Number(given);
  }
  return out;
}

/** The zoom steps of + and -. */
export const ZOOMS = [0.125, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.5, 2, 3, 4];

/** The step after `zoom` in `direction`. */
export function stepped(zoom: number, direction: number): number {
  const index = ZOOMS.findIndex((value) => value >= zoom - 1e-6);
  const at = index < 0 ? ZOOMS.length - 1 : index;
  const target = direction > 0 ? ZOOMS[Math.min(at + (ZOOMS[at] === zoom ? 1 : 0), ZOOMS.length - 1)] : ZOOMS[Math.max(at - 1, 0)];
  return target ?? 1;
}

/**
 * The plugin's own words for a host failure `plugins.error.<x>` (RB-080-63: never the raw key),
 * or undefined where it has none and the shared error sentence says it.
 */
export function hostReason(code: string, own: (key: string) => string | undefined): string | undefined {
  const host = /^plugins\.error\.([a-z_]+)$/.exec(code)?.[1];
  return host ? own(`panel.host_error.${host}`) : undefined;
}
