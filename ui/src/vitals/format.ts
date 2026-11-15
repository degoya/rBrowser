// The numbers and shapes of the Web Vitals badges (RB-060-10). A rating is never colour alone:
// each has its own shape (Lighthouse's: circle good, square needs improvement, triangle poor)
// and its word in the badge's accessible name.

import type { Rating, VitalMetric } from '../ipc/protocol';

/** The shape of each rating, in front of the value. */
export const SHAPES: Record<Rating, string> = {
  good: '●',
  'needs-improvement': '■',
  poor: '▲',
};

/** The colour of each rating (beside the shape and the word). */
export const TONES: Record<Rating, string> = {
  good: 'text-success',
  'needs-improvement': 'text-warning',
  poor: 'text-error',
};

/** A value as the badge shows it: ms below a second, seconds with two decimals above, CLS
 *  with two decimals. */
export function formatVital(metric: VitalMetric, value: number): string {
  if (metric === 'cls') {
    return value > 0 && value < 0.005 ? '<0.01' : value.toFixed(2);
  }
  return value < 1000 ? `${Math.round(value)} ms` : `${(value / 1000).toFixed(2)} s`;
}

/** A CPU percentage or MB as a whole number. */
export function formatResource(value: number): string {
  return String(Math.round(value));
}
