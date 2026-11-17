// The storage panel's logic without the DOM (RB-070-01, on Nuxt UI since RB-080-84): the shapes
// of the command `panel`'s answers (plugins/storage-panel/src/panel.rs), the columns grouped by
// session, which cells differ, keyboard movement in the grid and the arguments of the calls.

export interface StorageView {
  id: number;
  label: string;
  breakpoint: string;
  engine: string;
  session: string;
  url: string;
  origin: string | null;
  status: 'ok' | 'unsupported' | 'error';
  error?: string | null;
  storageError?: string | null;
  documentOrigin?: string | null;
  notRead?: number;
}

export interface StorageCell {
  value?: string;
  length: number;
  truncated?: boolean;
  domain?: string;
  path?: string;
  expires?: number | null;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: string | null;
  partitioned?: boolean;
}

export interface StorageRow {
  id: string;
  scope: string;
  name: string;
  path?: string;
  partition?: string;
  state: 'same' | 'differs' | 'missing';
  pinned: boolean;
  cells: Record<string, StorageCell | null>;
}

export interface StorageList {
  kind: string;
  views: StorageView[];
  rows: StorageRow[];
  total: number;
  shown: number;
  truncated: boolean;
}

export interface StoragePin {
  id: string;
  kind: string;
  origin: string;
  key: string;
  value: string;
  length?: number;
  truncated?: boolean;
  session: string | null;
  enabled: boolean;
  cookie?: { domain?: string | null; path?: string } | null;
}

export type CellState = 'value' | 'missing' | 'na' | 'unsupported' | 'error';

/** The tabs, as the plugin's `kind` names them. */
export const KINDS = ['cookies', 'local', 'session'] as const;
export type Kind = (typeof KINDS)[number];

/** The views ordered by session (each session where its first view stands), and the header
 * groups: one per session with its number of columns. */
export function arrange(views: StorageView[]): { columns: StorageView[]; groups: { session: string; span: number }[] } {
  const bySession = new Map<string, StorageView[]>();
  for (const view of views) {
    const list = bySession.get(view.session) ?? [];
    list.push(view);
    bySession.set(view.session, list);
  }
  const groups = [...bySession].map(([session, list]) => ({ session, span: list.length }));
  return { columns: [...bySession.values()].flat(), groups };
}

/** What decides whether two cells agree: the value and, for a cookie, its flags. */
function cellKey(cell: StorageCell): string {
  return [cell.value ?? '', cell.length, cell.httpOnly ?? '', cell.secure ?? '', cell.sameSite ?? ''].join('\u0000');
}

/** The views of a row whose cell differs from the largest group of agreeing views. */
export function differing(row: StorageRow): Set<number> {
  const groups = new Map<string, number[]>();
  for (const [view, cell] of Object.entries(row.cells)) {
    if (!cell) continue;
    const key = cellKey(cell);
    groups.set(key, [...(groups.get(key) ?? []), Number(view)]);
  }
  if (groups.size < 2) return new Set();
  const largest = [...groups.values()].reduce((a, b) => (b.length > a.length ? b : a));
  return new Set([...groups.values()].filter((g) => g !== largest).flat());
}

/** What a cell of `row` in `view` shows: `value`, `missing` (the entry applies there but is
 * absent), `na` (another page), `unsupported` (another engine) or `error`. */
export function cellState(row: StorageRow, view: StorageView): CellState {
  if (view.status === 'unsupported') return 'unsupported';
  if (view.status !== 'ok') return 'error';
  const id = String(view.id);
  if (!(id in row.cells)) return 'na';
  return row.cells[id] ? 'value' : 'missing';
}

/** Where a key moves the focus in a grid of `rows` x `cols` cells (column 0 the names), or
 * `null` for another key. */
export function moveCell(row: number, col: number, key: string, rows: number, cols: number, ctrl = false): [number, number] | null {
  if (rows === 0 || cols === 0) return null;
  switch (key) {
    case 'ArrowDown':
      return [Math.min(row + 1, rows - 1), col];
    case 'ArrowUp':
      return [Math.max(row - 1, 0), col];
    case 'ArrowRight':
      return [row, Math.min(col + 1, cols - 1)];
    case 'ArrowLeft':
      return [row, Math.max(col - 1, 0)];
    case 'Home':
      return ctrl ? [0, 0] : [row, 0];
    case 'End':
      return ctrl ? [rows - 1, cols - 1] : [row, cols - 1];
    default:
      return null;
  }
}

/** The arguments of `list` for a tab and the filters. */
export function listArgs(kind: string, filters: { origin: string; search: string }): Record<string, string> {
  const args: Record<string, string> = { op: 'list', kind };
  if (filters.origin.trim()) args.origin = filters.origin.trim().slice(0, 300);
  if (filters.search.trim()) args.search = filters.search.trim().slice(0, 200);
  return args;
}

/** The entry of a row in one view, as `value` and `pin` name it. */
export function entryArgs(kind: string, row: StorageRow, view: number): Record<string, string | number> {
  return { kind, view, scope: row.scope, name: row.name, path: row.path ?? '', partition: row.partition ?? '' };
}

/** The pins' kind names of the tabs. */
const KIND_NAMES: Record<string, string> = { cookies: 'cookies', local: 'localStorage', session: 'sessionStorage' };

/** The pin holding a row of tab `kind`, if any. */
export function pinOf(pins: StoragePin[], kind: string, row: StorageRow): StoragePin | undefined {
  const bare = (domain: string): string => domain.replace(/^\./, '').toLowerCase();
  const host = (origin: string): string => {
    try {
      return new URL(origin).hostname;
    } catch {
      return '';
    }
  };
  return pins.find((pin) => {
    if (pin.kind !== KIND_NAMES[kind] || pin.key !== row.name) return false;
    if (kind !== 'cookies') return pin.origin === row.scope;
    const domain = pin.cookie?.domain ?? host(pin.origin);
    return bare(domain) === bare(row.scope) && (pin.cookie?.path ?? '/') === (row.path ?? '/');
  });
}

/** A cookie's expiry as text: the date, or `session` for a session cookie. */
export function expiresText(seconds: number | null | undefined, session: string): string {
  if (typeof seconds !== 'number' || !Number.isFinite(seconds) || seconds <= 0) return session;
  return new Date(seconds * 1000).toISOString().replace('.000Z', 'Z');
}

export interface CookieForm {
  domain: string;
  path: string;
  expires: string;
  httpOnly: boolean;
  secure: boolean;
  sameSite: string;
}

/** The cookie attributes of the editor's fields, as `set` takes them. */
export function cookieArgs(form: CookieForm): Record<string, unknown> {
  const cookie: Record<string, unknown> = { path: form.path.trim() || '/', httpOnly: form.httpOnly, secure: form.secure };
  if (form.domain.trim()) cookie.domain = form.domain.trim();
  if (form.sameSite) cookie.sameSite = form.sameSite;
  const when = form.expires.trim() ? Date.parse(form.expires.trim()) : NaN;
  if (Number.isFinite(when) && when > 0) cookie.expires = Math.floor(when / 1000);
  return cookie;
}

/** `json` for a value that is JSON, or the start of it when cut (RB-080-65); else null. */
export function jsonOf(value: string, detect: (text: string) => string | null): 'json' | null {
  return /^\s*[[{]/.test(value) || detect(value) === 'json' ? 'json' : null;
}
