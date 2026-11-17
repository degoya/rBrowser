// The one line an entry of a settings list shows while it is closed (RB-080-13): its name, then
// what tells it apart - a breakpoint `mobile, 375 x 667, DPR 2, Chromium, Touch`, a cell
// `mobile, column 1, row 2`, a layout preset `review, grid, 3 x 1 grid, 2 cells` (joined by a
// middle dot). Read from
// the fields the entry has, not from a table per key, so a list added later sums up by itself.

import type { JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

export type Translate = (key: string, args?: Record<string, unknown>) => string;

type JsonObject = { [key: string]: JsonValue };

const SEPARATOR = ' \u00b7 ';
const NAME_FIELDS = ['label', 'id', 'name', 'breakpoint'] as const;
/** Fields shown as they are: a device's kind, a preset's mode and focus breakpoint. */
const PLAIN_FIELDS = ['kind', 'mode', 'focus'] as const;


function num(value: JsonValue | undefined): number | null {
  return typeof value === 'number' ? value : null;
}

function text(value: JsonValue | undefined): string | null {
  return typeof value === 'string' && value.trim() !== '' ? value : null;
}

/** The name of an entry: its label, id, name or breakpoint, the first one set. */
export function nameOf(value: JsonValue): string | null {
  if (!isObject(value)) {
    return null;
  }
  for (const field of NAME_FIELDS) {
    const name = text(value[field]);
    if (name !== null) {
      return name;
    }
  }
  return null;
}

function sizeParts(width: number | null, height: number | null, t: Translate): string[] {
  if (width !== null && height !== null) {
    return [t('settings.summary.size', { width: String(width), height: String(height) })];
  }
  if (width !== null) {
    return [t('settings.summary.width', { width: String(width) })];
  }
  return height === null ? [] : [t('settings.summary.height', { height: String(height) })];
}

function gridParts(layout: JsonObject, t: Translate): string[] {
  const parts: string[] = [];
  const columns = num(layout.columns);
  const rows = num(layout.rows);
  if (columns !== null && rows !== null) {
    parts.push(t('settings.summary.grid', { columns: String(columns), rows: String(rows) }));
  }
  if (Array.isArray(layout.cells) && layout.cells.length > 0) {
    parts.push(t('settings.summary.cells', { n: layout.cells.length }));
  }
  return parts;
}

function placeParts(value: JsonObject, t: Translate): string[] {
  const parts: string[] = [];
  const col = num(value.col);
  const row = num(value.row);
  if (col !== null && row !== null) {
    parts.push(t('settings.summary.position', { col: String(col), row: String(row) }));
    const colSpan = num(value.colSpan) ?? 1;
    const rowSpan = num(value.rowSpan) ?? 1;
    if (colSpan > 1 || rowSpan > 1) {
      parts.push(t('settings.summary.span', { columns: String(colSpan), rows: String(rowSpan) }));
    }
  }
  const zoom = num(value.zoom);
  if (zoom !== null) {
    parts.push(t('settings.summary.zoom', { zoom: String(zoom) }));
  }
  const x = num(value.x);
  const y = num(value.y);
  if (x !== null && y !== null) {
    parts.push(t('settings.summary.at', { x: String(x), y: String(y) }));
  }
  return parts;
}

/** What tells an entry apart, without its name. */
export function detailsOf(value: JsonValue, t: Translate): string[] {
  if (!isObject(value)) {
    return [];
  }
  const parts: string[] = [];
  for (const field of PLAIN_FIELDS) {
    const plain = text(value[field]);
    if (plain !== null) {
      parts.push(plain);
    }
  }
  const device = text(value.device);
  if (device !== null) {
    parts.push(t('settings.summary.device', { id: device }));
  }
  const viewport = isObject(value.viewport) ? value.viewport : value;
  parts.push(...sizeParts(num(viewport.width), num(viewport.height), t));
  parts.push(...gridParts(isObject(value.layout) ? value.layout : value, t));
  parts.push(...placeParts(value, t));
  const dpr = num(value.dpr);
  if (dpr !== null) {
    parts.push(t('settings.summary.dpr', { dpr: String(dpr) }));
  }
  const engine = text(value.engine);
  if (engine !== null) {
    parts.push(t(`engines.name.${engine}`));
  }
  if (value.touch === true) {
    parts.push(t('settings.summary.touch'));
  }
  const url = text(value.url);
  if (url !== null) {
    parts.push(url);
  }
  return parts;
}

/** The line of a list entry: its name (or "Unnamed entry") and its details. */
export function summaryOf(value: JsonValue, t: Translate): string {
  if (!isObject(value)) {
    return String(value);
  }
  return [nameOf(value) ?? t('settings.summary.unnamed'), ...detailsOf(value, t)].join(SEPARATOR);
}

/** The line of a map entry (a layout preset): the key it is stored under and its details. */
export function entrySummary(name: string, value: JsonValue, t: Translate): string {
  return [name, ...detailsOf(value, t)].join(SEPARATOR);
}
