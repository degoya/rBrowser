// The reference the e2e suite holds the browser to: the drawing's arithmetic, done here by hand.
// Colour matrices work in linear RGB (feColorMatrix with color-interpolation-filters linearRGB);
// the CSS filter functions in sRGB (Filter Effects 1); a blur leaves a flat swatch's centre as it
// is. Colours are 0-255 sRGB.

export type Rgb = [number, number, number];

export type Step =
  | { op: 'matrix'; values: number[] }
  | { op: 'blur'; px: number }
  | { op: 'contrast' | 'brightness' | 'saturate'; amount: number };

export interface Spec {
  filter: Step[];
  overlay: { kind: string; [key: string]: unknown } | null;
}

const clamp = (value: number): number => Math.min(1, Math.max(0, value));
const toLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toSrgb = (c: number): number => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);

function multiply(m: readonly number[], [r, g, b]: [number, number, number]): [number, number, number] {
  const row = (i: number): number => clamp((m[i * 5] ?? 0) * r + (m[i * 5 + 1] ?? 0) * g + (m[i * 5 + 2] ?? 0) * b + (m[i * 5 + 4] ?? 0));
  return [row(0), row(1), row(2)];
}

function saturate(s: number): number[] {
  return [
    0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s, 0, 0,
    0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s, 0, 0,
    0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s, 0, 0,
  ];
}

/** `color` through `steps`, in order. */
export function filtered(color: Rgb, steps: readonly Step[]): Rgb {
  let c = color.map((value) => value / 255) as [number, number, number];
  for (const step of steps) {
    switch (step.op) {
      case 'matrix':
        c = multiply(step.values, c.map(toLinear) as [number, number, number]).map(toSrgb) as [number, number, number];
        break;
      case 'blur':
        break;
      case 'brightness':
        c = c.map((value) => clamp(value * step.amount)) as [number, number, number];
        break;
      case 'contrast':
        c = c.map((value) => clamp((value - 0.5) * step.amount + 0.5)) as [number, number, number];
        break;
      case 'saturate':
        c = multiply(saturate(step.amount), c);
        break;
    }
  }
  return c.map((value) => Math.round(value * 255)) as Rgb;
}

/** `top` at `alpha` over `bottom`. */
export function over(bottom: Rgb, top: Rgb, alpha: number): Rgb {
  return bottom.map((value, index) => Math.round(value * (1 - alpha) + (top[index] ?? 0) * alpha)) as Rgb;
}

export function hex(value: string): Rgb {
  const n = parseInt(value.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** The largest difference of one channel. */
export const distance = (a: Rgb, b: Rgb): number => Math.max(...a.map((value, index) => Math.abs(value - (b[index] ?? 0))));
