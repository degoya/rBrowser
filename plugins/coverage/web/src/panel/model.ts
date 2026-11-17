// The Coverage panel's logic without the DOM (RB-080-07, on Nuxt UI since RB-080-84): from the
// answer of `coverage_get` one row per file with one value - the unused bytes and share, the
// smallest over the measured views (what is unused in every view that loads it), with the span
// when the views differ -, sorted by unused bytes (the largest lever first); the views measured
// and those that could not be (named once); the totals; the type of a file by its content - a
// style sheet a dev server hands over as a script module (`main.scss`, a Vue file's style) is a
// style sheet -; the used parts of a file over the views and its source cut into used and unused
// pieces.

export type Kind = 'css' | 'js';
export type ViewStatus = 'measured' | 'unavailable' | 'failed';

export interface CoverageCell {
  file: number;
  total: number;
  used: number;
  unusedPercent: number;
}

export interface CoverageViewLine {
  view: number;
  label: string;
  breakpoint: string;
  width: number;
  engine: string;
  url: string;
  status: ViewStatus;
  error?: string;
  files: CoverageCell[];
}

export interface CoverageSummary {
  mode: 'load' | 'interaction';
  running: boolean;
  started: number;
  finished: number;
  files: { id: number; url: string; kind: Kind; inline: boolean }[];
  views: CoverageViewLine[];
  truncated: boolean;
  findingCount: number;
  findings: unknown[];
}

export interface CoverageDetail {
  url: string;
  kind: Kind;
  inline: boolean;
  total: number;
  offset: number;
  length: number;
  next: number | null;
  views: { view: number; label: string; breakpoint: string; total: number; used: number; unusedPercent: number; segments: [number, number][] }[];
  text: string | null;
  textTotal?: number;
  rules?: { prelude: string; start: number; end: number; usedIn: number[] }[];
  functions?: { name: string; start: number; end: number; executedIn: number[] }[];
}

/** A view's value of one file. */
export interface ViewValue {
  view: number;
  name: string;
  total: number;
  unused: number;
  percent: number;
}

/** One file: its value over the views, and each view's. */
export interface FileRow {
  id: number;
  url: string;
  name: string;
  /** What the file holds: a style sheet or a script. */
  kind: Kind;
  /** As the measurement saw it: a style sheet served as a script module is `js` here. */
  measured: Kind;
  inline: boolean;
  total: number;
  /** Unused bytes and share in every view that loads it (the smallest). */
  unused: number;
  percent: number;
  /** The largest share; above `percent` when the views differ. */
  maxPercent: number;
  views: ViewValue[];
}

/** A view without a measurement. */
export interface Unmeasured {
  view: number;
  name: string;
  engine: string;
  status: Exclude<ViewStatus, 'measured'>;
  error: string | null;
}

/** The share, rounded to whole percent, at which two views count as the same. */
const SAME = 1;

/** The name of a view: its breakpoint, else its label. */
export const viewName = (view: { breakpoint: string; label: string }): string => view.breakpoint || view.label;

/** The last part of a URL's path (with the fragment of an inline file), for one line. */
export function shortName(url: string): string {
  const [path = '', fragment] = url.split('#', 2);
  const name = path.replace(/[?].*$/, '').split('/').filter(Boolean).pop() ?? path;
  return fragment ? name + '#' + fragment : name;
}

const STYLE_EXT = /\.(css|scss|sass|less|styl|stylus|pcss|postcss)$/i;

/** What a file holds: a script whose address names a style sheet - `main.scss`, `x.css?direct`,
 * a Vue file's `?vue&type=style` - is a style sheet a dev server (Vite) delivers as a module. */
export function contentKind(url: string, kind: Kind): Kind {
  if (kind === 'css') return 'css';
  const [beforeHash = ''] = url.split('#', 1);
  const [path = '', query = ''] = beforeHash.split('?', 2);
  if (STYLE_EXT.test(path)) return 'css';
  if (/(^|&)type=style(&|$)/.test(query) || /(^|&)lang\.(css|scss|sass|less|styl|stylus|pcss|postcss)(&|$)/i.test(query)) return 'css';
  return 'js';
}

/** The measured views' values per file, one row per file loaded by any, by unused bytes. */
export function rows(summary: CoverageSummary | null): FileRow[] {
  if (!summary || !Array.isArray(summary.views) || !Array.isArray(summary.files)) return [];
  const measured = summary.views.filter((view) => view.status === 'measured');
  const out: FileRow[] = [];
  for (const file of summary.files) {
    const views: ViewValue[] = [];
    for (const view of measured) {
      const cell = (view.files ?? []).find((entry) => entry.file === file.id);
      if (cell) views.push({ view: view.view, name: viewName(view), total: cell.total, unused: Math.max(0, cell.total - cell.used), percent: cell.unusedPercent });
    }
    if (views.length === 0) continue;
    const least = views.reduce((a, b) => (b.percent < a.percent ? b : a));
    const most = views.reduce((a, b) => (b.percent > a.percent ? b : a));
    out.push({
      id: file.id,
      url: file.url,
      name: shortName(file.url),
      kind: contentKind(file.url, file.kind),
      measured: file.kind,
      inline: Boolean(file.inline),
      total: Math.max(...views.map((view) => view.total)),
      unused: least.unused,
      percent: least.percent,
      maxPercent: most.percent,
      views,
    });
  }
  return out.sort((a, b) => b.unused - a.unused || b.percent - a.percent || a.name.localeCompare(b.name));
}

/** Whether the views' shares differ (by more than a rounding). */
export const differs = (row: FileRow): boolean => Math.round(row.maxPercent) - Math.round(row.percent) >= SAME;

/** The views without a measurement, each once. */
export function unmeasured(summary: CoverageSummary | null): Unmeasured[] {
  return (summary?.views ?? [])
    .filter((view): view is CoverageViewLine & { status: Unmeasured['status'] } => view.status !== 'measured')
    .map((view) => ({ view: view.view, name: viewName(view), engine: view.engine, status: view.status, error: view.error ?? null }));
}

/** The measured views. */
export const measuredCount = (summary: CoverageSummary | null): number => (summary?.views ?? []).filter((view) => view.status === 'measured').length;

/** The rows of `kind` (or all) whose address contains `query`, case-insensitive. */
export function filterRows(all: FileRow[], kind: Kind | 'all', query: string): FileRow[] {
  const needle = query.trim().toLowerCase();
  return all.filter((row) => (kind === 'all' || row.kind === kind) && (needle === '' || row.url.toLowerCase().includes(needle)));
}

/** The sum of `rows`: unused bytes, total bytes and the share. */
export function totals(list: FileRow[]): { unused: number; total: number; percent: number } {
  const unused = list.reduce((sum, row) => sum + row.unused, 0);
  const total = list.reduce((sum, row) => sum + row.total, 0);
  return { unused, total, percent: total > 0 ? (unused / total) * 100 : 0 };
}

/** Bytes for a reader: `512 B`, `12.3 KB`, `1.4 MB` (1024-based, as DevTools). */
export function bytes(value: number): string {
  if (value < 1024) return `${Math.round(value)} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

/** A share for a reader: whole percent, one decimal below 10. */
export function percent(value: number): string {
  return value < 10 && value > 0 ? value.toFixed(1) : String(Math.round(value));
}

/** The segments used in any of `views`, merged and sorted. */
export function usedAnywhere(views: { segments: [number, number][] }[]): [number, number][] {
  const all: [number, number][] = views.flatMap((view) => view.segments ?? []).map(([start, end]) => [start, end]);
  all.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const merged: [number, number][] = [];
  for (const [start, end] of all) {
    const last = merged[merged.length - 1];
    if (last && start <= last[1]) last[1] = Math.max(last[1], end);
    else if (start < end) merged.push([start, end]);
  }
  return merged;
}

/** `used` (absolute offsets) within the window of `length` units at `offset`, in the window's
 * own offsets. */
export function windowRanges(offset: number, length: number, used: [number, number][]): [number, number][] {
  const out: [number, number][] = [];
  for (const [start, end] of used) {
    const from = Math.max(0, start - offset);
    const to = Math.min(length, end - offset);
    if (to > from) out.push([from, to]);
  }
  return out;
}

/** `source` (the window starting at `offset`) cut into used and unused pieces. Offsets are
 * UTF-16 units, as JavaScript strings count. */
export function pieces(source: string, offset: number, used: [number, number][]): { text: string; used: boolean }[] {
  const out: { text: string; used: boolean }[] = [];
  let at = 0;
  for (const [from, to] of windowRanges(offset, source.length, used)) {
    if (to <= at) continue;
    if (from > at) out.push({ text: source.slice(at, from), used: false });
    out.push({ text: source.slice(Math.max(from, at), to), used: true });
    at = to;
  }
  if (at < source.length) out.push({ text: source.slice(at), used: false });
  return out.filter((piece) => piece.text.length > 0);
}
