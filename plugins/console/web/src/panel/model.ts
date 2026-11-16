// The console panel's logic without the DOM (RB-060-02, on Nuxt UI since RB-080-84): the
// levels, view colours, sources, the arguments of `grouped`, keyboard moves, and the shapes of
// the plugin's answers `grouped` and `exec` (plugins/console/src/lib.rs).

/** Every level, in the toolbar's order. */
export const LEVELS = ['error', 'warning', 'info', 'log', 'debug', 'network'] as const;
export type Level = (typeof LEVELS)[number];

/** The levels shown without a setting: all but debug. */
export const DEFAULT_LEVELS: Level[] = ['error', 'warning', 'info', 'log', 'network'];

/** View dot colours: distinct hues with at least 3:1 contrast against the panel's light and
 * dark backgrounds (WCAG 1.4.11); the dot's shape tells the engine, its label the view. */
export const PALETTE = ['#1a73e8', '#d93025', '#1e8e3e', '#b06000', '#a142f4', '#12858c', '#e52592', '#7c7c7c'];

/** The newest entries the panel asks for. */
export const LIMIT = 200;

export interface ViewEntry {
  id: number;
  label: string;
  breakpoint: string;
  engine: string;
  width: number;
}

export interface GroupEntry {
  id: string;
  level: string;
  kind: string;
  text: string;
  count: number;
  views: number[];
  perView: Record<string, number>;
  source?: { url: string; line?: number | null; column?: number | null };
  stack?: string;
  first: number;
  last: number;
}

export interface GroupedAnswer {
  captured: boolean;
  total: number;
  shown: number;
  truncated: boolean;
  levels: Record<string, number>;
  entries: GroupEntry[];
  views: ViewEntry[];
}

export interface ExecResult {
  ok: boolean;
  type: string;
  text: string;
  views: number[];
}

export interface ExecRecord {
  id: number;
  expression: string;
  results: ExecResult[];
  error?: string;
}

export interface Filters {
  levels: Set<string>;
  views: Set<number>;
  search: string;
}

/** The colour of the view at `index` in the answer's view list. */
export function viewColor(index: number): string {
  const at = ((index % PALETTE.length) + PALETTE.length) % PALETTE.length;
  return PALETTE[at] ?? '#5f6368';
}

/** A source as the list shows it - the file and line - and in full for its tooltip. */
export function shortSource(source: { url?: string; line?: number | null } | undefined): { short: string; full: string } | null {
  if (!source || !source.url) return null;
  const line = typeof source.line === 'number' ? ':' + source.line : '';
  const path = source.url.split(/[?#]/)[0] ?? source.url;
  const file = path.replace(/\/+$/, '').split('/').pop() || path;
  return { short: file + line, full: source.url + line };
}

/** A row's source as the list shows it and its tooltip: rBrowser and the plugin for what
 * rBrowser's plugins logged (kind `rbrowser`, RB-080-66), else the file and line. `t` is the
 * panel's text below the plugin's prefix. */
export function sourceLabel(
  t: (key: string, params?: Record<string, unknown>) => string,
  entry: { kind?: string; source?: { url?: string; line?: number | null } },
): { text: string; title: string } {
  const source = shortSource(entry.source);
  if (entry.kind === 'rbrowser') return { text: t('panel.rbrowserSource', { plugin: source ? source.short : '' }), title: entry.source?.url ?? '' };
  return source ? { text: source.short, title: source.full } : { text: t('panel.unknownSource'), title: '' };
}

/** The levels the panel starts with: the setting `console.levels` when it reads. */
export function startLevels(settings: Record<string, unknown>): string[] {
  const levels = settings['console.levels'];
  if (!Array.isArray(levels)) return [...DEFAULT_LEVELS];
  return LEVELS.filter((level) => levels.includes(level));
}

/** The arguments of `grouped` for the panel's filters. */
export function groupedArgs(filters: Filters): { levels: string[]; views?: number[]; search?: string; limit: number } {
  const args: { levels: string[]; views?: number[]; search?: string; limit: number } = { levels: LEVELS.filter((level) => filters.levels.has(level)), limit: LIMIT };
  if (filters.views.size > 0) args.views = [...filters.views].sort((a, b) => a - b);
  const search = filters.search.trim();
  if (search) args.search = search.slice(0, 200);
  return args;
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

/** Whether a failure is a code of rBrowser or a plugin (`plugins.error.busy reason=...`) - said
 * as a sentence - rather than the page's own words (`TypeError: x is not a function`). */
export function isCode(text: string): boolean {
  return /^[a-z][\w-]*(?:\.[\w-]+)+(?:\s|$)/.test(text);
}
