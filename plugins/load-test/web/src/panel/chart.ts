// The time chart of a run (RB-096-07): the requests per second and the p95 of each second as two
// lines in one box, each scaled to its own top, so both read at a glance; pure, so it is tested
// without a DOM.

export interface Line {
  points: string;
  top: number;
}

/** The top a line is scaled to: its largest value, at least 1. */
export function topOf(values: (number | null)[]): number {
  return values.reduce<number>((top, value) => (value !== null && Number.isFinite(value) && value > top ? value : top), 1);
}

/** `values`, one per second, as an SVG polyline in a box `width` by `height`; a second without
 * a value lies on the floor. */
export function line(values: (number | null)[], width: number, height: number): Line {
  const top = topOf(values);
  const steps = Math.max(values.length - 1, 1);
  const points = values
    .map((value, index) => {
      const x = (index / steps) * width;
      const share = value !== null && Number.isFinite(value) ? Math.min(Math.max(value / top, 0), 1) : 0;
      return `${x.toFixed(1)},${(height - share * height).toFixed(1)}`;
    })
    .join(' ');
  return { points, top };
}
