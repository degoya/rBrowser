// The Lighthouse panel's logic without the DOM (RB-060-07, on Nuxt UI since RB-080-83): ratings,
// the score cards' rows with the change since the previous run, keyboard moves through the tiles,
// and the page link the panel may open.

/** Lighthouse's categories, in its order. */
export const CATEGORIES = ['performance', 'accessibility', 'best-practices', 'seo'] as const;

export type Rating = 'good' | 'average' | 'poor';

export interface ScoreCell {
  category: string;
  score: number | null;
  rating: Rating | null;
  delta: number | null;
}

export interface ScoreRow {
  breakpoint: string;
  report: string;
  findings: number;
  cells: ScoreCell[];
}

export interface BreakpointResult {
  breakpoint: string;
  view: number;
  width: number;
  height: number;
  formFactor: string;
  url: string;
  scores: Record<string, number | null>;
  report: string;
  json: string;
  findings: number;
}

export interface LighthouseSummary {
  runId: string;
  url: string | null;
  categories?: string[];
  breakpoints: BreakpointResult[];
  skipped?: { breakpoint: string; engine: string | null; reason: string }[];
  failures?: { breakpoint: string; reason: string }[];
  cancelled?: boolean;
  previous?: { runId: string; breakpoints: { breakpoint: string; scores: Record<string, number | null> }[] } | null;
}

export interface LighthouseStatus {
  sidecar: {
    available: boolean;
    running?: { job: string; label: string; url: string } | null;
    queued?: number;
    error?: string | null;
    reason?: string | null;
  };
  latest: LighthouseSummary | null;
}

/** Lighthouse's rating of a score 0 to 100: good from 90, average from 50, else poor. */
export function rating(score: number | null | undefined): Rating | null {
  if (typeof score !== 'number') return null;
  if (score >= 90) return 'good';
  if (score >= 50) return 'average';
  return 'poor';
}

/** One card per measured breakpoint, a cell per category of the run, each with its rating and its
 * change since the previous run of the same breakpoint. */
export function rows(latest: LighthouseSummary | null): { categories: string[]; rows: ScoreRow[] } {
  if (!latest || !Array.isArray(latest.breakpoints)) return { categories: [], rows: [] };
  const wanted: readonly string[] = latest.categories ?? CATEGORIES;
  const categories = CATEGORIES.filter((category) => wanted.includes(category));
  const before = new Map<string, Record<string, number | null>>();
  for (const entry of latest.previous?.breakpoints ?? []) before.set(entry.breakpoint, entry.scores ?? {});
  return {
    categories,
    rows: latest.breakpoints.map((entry) => ({
      breakpoint: entry.breakpoint,
      report: entry.report,
      findings: entry.findings ?? 0,
      cells: categories.map((category) => {
        const score = entry.scores?.[category] ?? null;
        const old = before.get(entry.breakpoint)?.[category];
        return {
          category,
          score,
          rating: rating(score),
          delta: typeof score === 'number' && typeof old === 'number' && score !== old ? score - old : null,
        };
      }),
    })),
  };
}

/** The tile `key` moves to among `rowCount` x `columnCount` tiles; null for other keys. */
export function moveCell(row: number, column: number, key: string, rowCount: number, columnCount: number): [number, number] | null {
  const clamp = (value: number, count: number): number => Math.max(0, Math.min(count - 1, value));
  switch (key) {
    case 'ArrowUp':
      return [clamp(row - 1, rowCount), column];
    case 'ArrowDown':
      return [clamp(row + 1, rowCount), column];
    case 'ArrowLeft':
      return [row, clamp(column - 1, columnCount)];
    case 'ArrowRight':
      return [row, clamp(column + 1, columnCount)];
    case 'Home':
      return [row, 0];
    case 'End':
      return [row, columnCount - 1];
    default:
      return null;
  }
}

/** The measured page when the panel may open it: only `http:` and `https:` (the host's rule for
 * `open_link`, RB-080-92). */
export function pageLink(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.href : null;
  } catch {
    return null;
  }
}

/** The sign of a change: `+3`, `-4`. */
export const signed = (delta: number): string => (delta > 0 ? `+${delta}` : String(delta));
