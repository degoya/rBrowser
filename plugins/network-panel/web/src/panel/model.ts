// The network panel's logic without the DOM (RB-070-02, on Nuxt UI since RB-080-84): view
// colours, sizes and times, the arguments of `list`, `mock_add`, `breakpoint_add` and
// `compose`, header lines, keyboard moves, a row's key and status.
import type { HeldEntry, ListRow } from './types';

/**
 * View dot colours: distinct hues with at least 3:1 contrast against the panel's light and dark
 * backgrounds (WCAG 1.4.11); the dot's shape tells the engine, its label the view.
 */
export const PALETTE = ['#1a73e8', '#d93025', '#1e8e3e', '#b06000', '#a142f4', '#12858c', '#e52592', '#7c7c7c'];

/** The resource types the list filters by (CDP's names). */
export const TYPES = ['Document', 'Stylesheet', 'Script', 'Image', 'Font', 'XHR', 'Fetch', 'Media', 'Other'];

/** The status filters of the list. */
export const STATUSES = ['all', 'ok', 'error', 'failed'];

export type Result<T> = { ok: true; args: T } | { ok: false; error: string };

/** The colour of the view at `index` in the answer's view list. */
export function viewColor(index: number): string {
  const at = ((index % PALETTE.length) + PALETTE.length) % PALETTE.length;
  return PALETTE[at] ?? '#5f6368';
}

/** Bytes as the list shows them. */
export function size(bytes: number | null | undefined): string {
  if (typeof bytes !== 'number' || !Number.isFinite(bytes)) return '-';
  if (bytes < 1024) return Math.round(bytes) + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' kB';
  return (bytes / 1024 / 1024).toFixed(1) + ' MB';
}

/** Milliseconds as the list shows them. */
export function duration(ms: number | null | undefined): string {
  if (typeof ms !== 'number' || !Number.isFinite(ms)) return '-';
  return ms < 1000 ? Math.round(ms) + ' ms' : (ms / 1000).toFixed(2) + ' s';
}

/** A URL's last path segment and query, for the list; the whole URL goes into the tooltip. */
export function shortUrl(url: string): string {
  const path = url.replace(/^[a-z]+:\/\/[^/]+/i, '') || '/';
  const tail = path.split('/').filter(Boolean).pop() ?? '';
  return tail || path;
}

/** `url` as a glob matching exactly itself (`*`, `?` and `\` escaped). */
export function escapeGlob(url: string): string {
  return url.replace(/[*?\\]/g, (c) => '\\' + c);
}

export interface Filters {
  search: string;
  type: string;
  status: string;
  differences: boolean;
  /** An endpoint's key (RB-096-06): only its requests; '' for all. */
  endpoint: string;
}

/** The arguments of `list` for the panel's filters. */
export function listArgs(filters: Filters): Record<string, unknown> {
  const args: Record<string, unknown> = { limit: 150 };
  const search = filters.search.trim();
  if (search) args.search = search.slice(0, 200);
  if (filters.type) args.types = [filters.type];
  if (filters.status && filters.status !== 'all') args.status = filters.status;
  if (filters.differences) args.differences = true;
  if (filters.endpoint) args.endpoint = filters.endpoint;
  return args;
}

/** The arguments of `mock_add` from the editor's fields, or the key of the field that is wrong. */
export function mockArgs(form: { pattern: string; method: string; action: string; status: string | number; delay: string | number; body: string }): Result<Record<string, unknown>> {
  const pattern = form.pattern.trim();
  if (!pattern || pattern.length > 2048) return { ok: false, error: 'editor.badPattern' };
  const match: Record<string, unknown> = { urlPattern: pattern };
  const method = form.method.trim().toUpperCase();
  if (method) {
    if (!/^[A-Z]{1,16}$/.test(method)) return { ok: false, error: 'editor.badMethod' };
    match.method = method;
  }
  if (form.action === 'block') return { ok: true, args: { match, block: true } };
  const status = String(form.status).trim() === '' ? Number.NaN : Number(form.status);
  if (!Number.isInteger(status) || status < 100 || status > 599) return { ok: false, error: 'editor.badStatus' };
  const delay = String(form.delay).trim() === '' ? 0 : Number(form.delay);
  if (!Number.isInteger(delay) || delay < 0 || delay > 60000) return { ok: false, error: 'editor.badDelay' };
  const mock: Record<string, unknown> = { status, delayMs: delay };
  if (form.body) mock.body = form.body;
  return { ok: true, args: { match, mock } };
}

/** The breakpoint `breakpoint_add` takes from the form, or the text key naming the wrong field. */
export function breakpointArgs(form: { pattern: string; regex: boolean; method: string; stage: string; views: string }): Result<Record<string, unknown>> {
  const pattern = form.pattern.trim();
  if (!pattern || pattern.length > 2048) return { ok: false, error: 'breakpoints.badPattern' };
  const args: Record<string, unknown> = { urlPattern: pattern, stage: form.stage === 'response' ? 'response' : 'request' };
  if (form.regex) args.regex = true;
  const method = form.method.trim().toUpperCase();
  if (method) {
    if (!/^[A-Z]{1,16}$/.test(method)) return { ok: false, error: 'editor.badMethod' };
    args.method = method;
  }
  const views = form.views.split(/[,\s]+/).filter((part) => part !== '').map(Number);
  if (views.some((id) => !Number.isInteger(id) || id < 0)) return { ok: false, error: 'breakpoints.badViews' };
  if (views.length) args.views = views;
  return { ok: true, args };
}

/** Header lines `name: value` as the pairs the tools take, blank lines left out. */
export function parseHeaderLines(text: string): [string, string][] {
  return text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '')
    .map((line): [string, string] => {
      const at = line.indexOf(':');
      return at < 0 ? [line, ''] : [line.slice(0, at).trim(), line.slice(at + 1).trim()];
    });
}

/** Header pairs as the lines of a text field. */
export function headerLines(headers: [string, string][]): string {
  return headers.map(([name, value]) => name + ': ' + value).join('\n');
}

/** The compose arguments from the form, or the text key naming the wrong field. */
export function composeArgs(form: { method: string; url: string; headers: string; body: string; view: string; times: string | number }): Result<Record<string, unknown>> {
  const url = form.url.trim();
  if (!/^https?:\/\/\S+$/.test(url) || url.length > 2048) return { ok: false, error: 'compose.badUrl' };
  const method = form.method.trim().toUpperCase() || 'GET';
  if (!/^[A-Z]{1,16}$/.test(method)) return { ok: false, error: 'editor.badMethod' };
  const request: Record<string, unknown> = { method, url, headers: parseHeaderLines(form.headers) };
  if (form.body) request.body = form.body;
  const args: Record<string, unknown> = { request };
  if (form.view !== '') args.view = Number(form.view);
  const times = String(form.times).trim() === '' ? 1 : Number(form.times);
  if (Number.isInteger(times) && times > 1) args.times = Math.min(times, 10);
  return { ok: true, args };
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

/** A row's identity across polls: method and URL. */
export const keyOf = (row: ListRow): string => row.method + ' ' + row.url;

/** What a row's status cell says: `blocked`, `failed`, or the status codes its views saw. */
export function statusOf(row: ListRow): 'blocked' | 'failed' | string {
  const seen = Object.values(row.views);
  const failed = seen.find((s) => s.failed);
  if (failed) return /BLOCKED_BY_CLIENT/.test(failed.failed ?? '') ? 'blocked' : 'failed';
  const codes = [...new Set(seen.map((s) => s.status).filter((s): s is number => typeof s === 'number'))];
  return codes.join(' / ') || '-';
}

/** The content type a block of header lines names, or ''. */
export const contentType = (headers: string): string => /^content-type\s*:\s*(.*)$/im.exec(headers)?.[1] ?? '';

/** The fields of the held editor. */
export interface HeldFields {
  url: string;
  status: string | number;
  headers: string;
  body: string;
}

/** The fields the held editor starts with for `held`. */
export function heldFields(held: HeldEntry): HeldFields {
  const response = held.stage === 'response';
  return {
    url: held.url,
    status: String(held.response?.status ?? ''),
    headers: headerLines(response ? (held.response?.headers ?? []) : held.headers),
    body: held.bodyBase64 ? '' : (held.body ?? ''),
  };
}

/** What the person changed in the held editor against `held`: `continue`'s `edits`. */
export function heldEdits(held: HeldEntry, fields: HeldFields): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (held.stage === 'response') {
    const status = Number(fields.status);
    if (status && status !== held.response?.status) out.status = status;
  } else if (fields.url !== held.url) {
    out.url = fields.url;
  }
  const headers = parseHeaderLines(fields.headers);
  const original = held.stage === 'response' ? (held.response?.headers ?? []) : held.headers;
  if (JSON.stringify(headers) !== JSON.stringify(original)) out.headers = headers;
  if (!held.bodyBase64 && fields.body !== (held.body ?? '')) out.body = fields.body;
  return out;
}
