// The report history's logic without the DOM (RB-091-02): the shapes of the tool `reports`
// (`list`, `compare`), run labels, the breakpoints and runs to pick from, the default pair to
// compare, and how a metric and its change read.

export interface HistoryBreakpoint {
  breakpoint: string;
  scores: Record<string, number | null>;
  /** The report below the plugin's out folder, `<run>/<file>.html`. */
  report: string;
}

export interface HistoryRun {
  runId: string;
  startedAt: number | null;
  url: string | null;
  cancelled: boolean;
  breakpoints: HistoryBreakpoint[];
}

export interface CompareRow {
  before: number | null;
  after: number | null;
  delta: number | null;
}

export interface Comparison {
  breakpoint: string;
  before: string;
  after: string;
  scores: (CompareRow & { category: string })[];
  metrics: (CompareRow & { metric: string; unit: string })[];
}

/** `YYYYMMDD-HHMMSS` as `YYYY-MM-DD HH:MM:SS UTC`; anything else as it is. */
export function runLabel(runId: string): string {
  const parts = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/.exec(runId);
  if (!parts) return runId;
  const [, year, month, day, hour, minute, second] = parts;
  return `${year}-${month}-${day} ${hour}:${minute}:${second} UTC`;
}

/** Every breakpoint of `runs` once, in the order they first appear (newest run first). */
export function breakpointsOf(runs: HistoryRun[]): string[] {
  const seen: string[] = [];
  for (const run of runs) for (const entry of run.breakpoints) if (!seen.includes(entry.breakpoint)) seen.push(entry.breakpoint);
  return seen;
}

/** The runs that measured `breakpoint`, newest first. */
export const runsWith = (runs: HistoryRun[], breakpoint: string): HistoryRun[] =>
  runs.filter((run) => run.breakpoints.some((entry) => entry.breakpoint === breakpoint));

/** The two newest runs of `breakpoint`, the older first; null with fewer. */
export function defaultPair(runs: HistoryRun[], breakpoint: string): [string, string] | null {
  const [after, before] = runsWith(runs, breakpoint);
  return after && before ? [before.runId, after.runId] : null;
}

const MS = new Intl.NumberFormat('en', { maximumFractionDigits: 0 });
const PLAIN = new Intl.NumberFormat('en', { minimumFractionDigits: 3, maximumFractionDigits: 3 });

/** A metric in its unit: milliseconds whole, a unitless one (CLS) with three decimals. */
export function formatMetric(value: number | null | undefined, unit: string): string {
  if (typeof value !== 'number') return '-';
  return unit ? `${MS.format(value)} ${unit}` : PLAIN.format(value);
}

/** A change with its sign (`+250 ms`, `-0.010`); empty without one. */
export function formatDelta(delta: number | null | undefined, unit: string): string {
  if (typeof delta !== 'number') return '';
  const text = formatMetric(Math.abs(delta), unit);
  if (delta > 0) return `+${text}`;
  if (delta < 0) return `-${text}`;
  return text;
}
