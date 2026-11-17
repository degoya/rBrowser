// The arithmetic of the theme contrast test (RB-090-22, docs/design.md section 7): Tailwind's
// `oklch()` colours as sRGB, WCAG's relative luminance and contrast ratio, a translucent colour
// laid over its ground as the browser does it, and the theme tokens a stylesheet sets. Used by
// `contrast.test.ts` only; nothing here runs in the shell.

/** An sRGB colour, each channel gamma-encoded in 0..1. */
export type Rgb = readonly [number, number, number];

export const WHITE: Rgb = [1, 1, 1];

/** `oklch(L% C H)` (Tailwind's palette) as sRGB, clipped to the gamut. */
export function oklch(value: string): Rgb {
  const match = /^oklch\(([\d.]+)% ([\d.]+) ([\d.]+)\)$/.exec(value.trim());
  if (!match) {
    throw new Error(`not an oklch() colour: ${value}`);
  }
  const lightness = Number(match[1]) / 100;
  const chroma = Number(match[2]);
  const hue = (Number(match[3]) * Math.PI) / 180;
  const a = chroma * Math.cos(hue);
  const b = chroma * Math.sin(hue);
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const linear = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  const [red, green, blue] = linear.map((channel) => encode(Math.min(1, Math.max(0, channel))));
  return [red ?? 0, green ?? 0, blue ?? 0];
}

/** `#rgb` or `#rrggbb` as sRGB. */
export function hex(value: string): Rgb {
  const digits = value.replace('#', '');
  const full = digits.length === 3 ? [...digits].map((digit) => digit + digit).join('') : digits;
  const channel = (at: number): number => parseInt(full.slice(at, at + 2), 16) / 255;
  return [channel(0), channel(2), channel(4)];
}

function encode(linear: number): number {
  return linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
}

function decode(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

/** WCAG 2's relative luminance. */
export function luminance([red, green, blue]: Rgb): number {
  return 0.2126 * decode(red) + 0.7152 * decode(green) + 0.0722 * decode(blue);
}

/** WCAG 2's contrast ratio of two colours, 1 to 21. */
export function ratio(one: Rgb, other: Rgb): number {
  const [light, dark] = [luminance(one), luminance(other)].sort((x, y) => y - x);
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}

/** `colour` at `alpha` laid over the opaque `ground`, mixed in sRGB as the browser composites. */
export function over(colour: Rgb, alpha: number, ground: Rgb): Rgb {
  const mix = (at: 0 | 1 | 2): number => colour[at] * alpha + ground[at] * (1 - alpha);
  return [mix(0), mix(1), mix(2)];
}

/** The custom properties `--ui-*` the first rule for `selector` in `css` sets, by name. */
export function tokens(css: string, selector: string): Record<string, string> {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const block = new RegExp(`(?:^|[}\\s;])${escaped}\\s*\\{([^}]*)\\}`).exec(css);
  if (!block) {
    return {};
  }
  const found: Record<string, string> = {};
  for (const [, name, value] of (block[1] ?? '').matchAll(/(--ui-[\w-]+)\s*:\s*([^;]+);?/g)) {
    if (name && value) {
      found[name] = value.trim();
    }
  }
  return found;
}
