// WCAG 1.4.11 for the view dots (RB-080-09, on Nuxt UI since RB-080-84): every dot colour stands
// out at least 3:1 from the panel's background, light and dark (Nuxt UI's `bg-default`: white,
// and slate 900 in dark). The severity colours and badges are Nuxt UI's theme.
import { describe, expect, it } from 'vitest';
import { PALETTE } from './panel/model';

function luminance(hex: string): number {
  const channels = [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255);
  const [r, g, b] = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return ((hi ?? 0) + 0.05) / ((lo ?? 0) + 0.05);
}

describe.each([
  ['light', '#ffffff'],
  ['dark', '#0f172b'],
])('the %s theme', (_, background) => {
  it('every view dot stands out from the background', () => {
    for (const dot of PALETTE) expect(contrast(dot, background), dot).toBeGreaterThanOrEqual(3);
  });
});
