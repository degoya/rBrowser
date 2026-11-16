// The error report panel's logic without the DOM (RB-080-09, on Nuxt UI since RB-080-84): the
// shapes of the plugin's `get` answer (plugins/error-report/src/report.rs), view colours, sources,
// the entries by category, keyboard moves.

export interface ReportSource {
  url: string;
  line?: number | null;
  column?: number | null;
}

export interface ReportRequest {
  method?: string;
  status?: number | null;
  resourceType?: string | null;
  mimeType?: string | null;
}

export interface ReportView {
  id: number;
  label: string;
  breakpoint: string;
  engine: string;
  width: number;
  url: string;
  captures: boolean;
}

export interface ReportEntry {
  id: string;
  rule: string;
  severity: string;
  text: string;
  url?: string;
  source?: ReportSource;
  stack?: string;
  request?: ReportRequest;
  detail?: string;
  onlyIn?: string[];
  views: number[];
  perView: Record<string, number>;
  count: number;
  first: number;
  last: number;
}

export interface ReportAnswer {
  captured: boolean;
  total: number;
  truncated: boolean;
  summary: Record<string, number>;
  severities: Record<string, number>;
  views: ReportView[];
  entries: ReportEntry[];
  published?: unknown;
}

/** Every category (rule id), in the report's order. */
export const CATEGORIES = [
  'js-exception',
  'http-not-found',
  'http-server-error',
  'mixed-content-blocked',
  'mixed-content-passive',
  'csp-violation',
  'deprecation',
] as const;

/** View dot colours, as the console's: distinct hues with at least 3:1 contrast against the
 * panel's light and dark backgrounds (WCAG 1.4.11); the dot's shape tells the engine. */
export const PALETTE = ['#1a73e8', '#d93025', '#1e8e3e', '#b06000', '#a142f4', '#12858c', '#e52592', '#7c7c7c'];

/** The colour of the view at `index` in the answer's view list. */
export function viewColor(index: number): string {
  const at = ((index % PALETTE.length) + PALETTE.length) % PALETTE.length;
  return PALETTE[at] ?? '#5f6368';
}

/** A source as the list shows it - the file, line and column - and in full for its tooltip. */
export function shortSource(source: ReportSource | undefined): { short: string; full: string } | null {
  if (!source || !source.url) return null;
  let place = typeof source.line === 'number' ? ':' + source.line : '';
  if (place && typeof source.column === 'number') place += ':' + source.column;
  const path = source.url.split(/[?#]/)[0] ?? source.url;
  const file = path.replace(/\/+$/, '').split('/').pop() || path;
  return { short: file + place, full: source.url + place };
}

/** A request as one line: method, status, resource type. */
export function requestText(request: ReportRequest | undefined): string {
  if (!request) return '';
  return [request.method, request.status, request.resourceType].filter((part) => part !== undefined && part !== null && part !== '').join(' ');
}

/** The entries grouped by category, in the report's order; empty categories left out. */
export function byCategory<E extends { rule: string }>(entries: E[]): { category: string; entries: E[] }[] {
  return CATEGORIES.map((category) => ({ category: category as string, entries: entries.filter((entry) => entry.rule === category) })).filter(
    (group) => group.entries.length > 0,
  );
}

/** The i18n name of a rule id (`js-exception` -> `js_exception`). */
export function keyName(rule: string): string {
  return rule.replace(/-/g, '_');
}

/** The row a key moves the focus to in a list of `length` rows, or `null` for another key. */
export function moveFocus(index: number, key: string, length: number): number | null {
  if (length === 0) return null;
  switch (key) {
    case 'ArrowDown':
      return Math.min(index + 1, length - 1);
    case 'ArrowUp':
      return Math.max(index - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return length - 1;
    default:
      return null;
  }
}

/** The Nuxt UI colour and the Lucide icon of a severity; its word is the text `severities.*`. */
export const SEVERITY = {
  error: { color: 'error', icon: 'i-lucide-circle-x' },
  warning: { color: 'warning', icon: 'i-lucide-triangle-alert' },
  info: { color: 'info', icon: 'i-lucide-info' },
} as const;

export function severityOf(severity: string): (typeof SEVERITY)[keyof typeof SEVERITY] {
  return SEVERITY[severity as keyof typeof SEVERITY] ?? SEVERITY.info;
}
