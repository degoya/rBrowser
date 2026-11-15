// The Vue panel's Timeline tab (RB-080-31), pure: the kit's timeline events (the
// `timeline-event-updated` broadcast, `{ layerId, event }` of a plugin's `addTimelineEvent` or
// the kit's own mouse, keyboard, component-event and performance layers), kept newest first
// up to a limit, as rows for the tree pane and as rows of one event for the state pane.

import { formatValue, type Row, type StateRow } from './vue-model';
import { isObject } from '../../../web-common/src/is-object';

/** One event of the timeline. */
export interface TimelineEntry {
  /** The panel's number, newest highest. */
  id: number;
  layerId: string;
  /** Milliseconds since the epoch (the kit's `now()`). */
  time: number;
  title: string;
  subtitle: string;
  logType: string;
  groupId: string | number | undefined;
  data: unknown;
}

/** How many events the log keeps. */
export const DEFAULT_LIMIT = 500;

function text(value: unknown): string | null {
  if (value === undefined) return '';
  return typeof value === 'string' ? value : null;
}

/** The entry of the kit's payload, numbered `id`; null for anything else. */
export function timelineEntry(payload: unknown, id: number): TimelineEntry | null {
  if (!isObject(payload) || typeof payload.layerId !== 'string' || !isObject(payload.event)) return null;
  const event = payload.event;
  if (typeof event.time !== 'number') return null;
  const title = text(event.title);
  const subtitle = text(event.subtitle);
  const logType = text(event.logType);
  if (title === null || subtitle === null || logType === null) return null;
  const groupId = typeof event.groupId === 'string' || typeof event.groupId === 'number' ? event.groupId : undefined;
  return {
    id,
    layerId: payload.layerId,
    time: event.time,
    title,
    subtitle,
    logType: logType === '' ? 'default' : logType,
    groupId,
    data: event.data ?? null,
  };
}

/** The events seen, newest first, at most `limit` of them; by id in constant time. */
export class TimelineLog {
  private readonly limit: number;
  private readonly list: TimelineEntry[] = [];
  private readonly byId = new Map<number, TimelineEntry>();
  private next = 1;

  constructor(limit = DEFAULT_LIMIT) {
    this.limit = limit;
  }

  /** Takes the kit's payload; false when it is no timeline event. */
  push(payload: unknown): boolean {
    const entry = timelineEntry(payload, this.next);
    if (entry === null) return false;
    this.next += 1;
    this.list.unshift(entry);
    this.byId.set(entry.id, entry);
    while (this.list.length > this.limit) {
      const dropped = this.list.pop();
      if (dropped !== undefined) this.byId.delete(dropped.id);
    }
    return true;
  }

  entries(): readonly TimelineEntry[] {
    return this.list;
  }

  get(id: number): TimelineEntry | undefined {
    return this.byId.get(id);
  }

  clear(): void {
    this.list.length = 0;
    this.byId.clear();
  }
}

/** `time` as a clock time with milliseconds, in the viewer's zone. */
export function clockTime(time: number): string {
  const date = new Date(time);
  const two = (n: number): string => String(n).padStart(2, '0');
  return `${two(date.getHours())}:${two(date.getMinutes())}:${two(date.getSeconds())}.${String(date.getMilliseconds()).padStart(3, '0')}`;
}

function labelOf(entry: TimelineEntry): string {
  if (entry.title === '') return entry.layerId;
  return entry.subtitle === '' ? entry.title : `${entry.title}: ${entry.subtitle}`;
}

/** The rows of `entries` whose title, subtitle or layer contains `filter` (case-insensitive). */
export function timelineRows(entries: readonly TimelineEntry[], filter: string): Row[] {
  const needle = filter.trim().toLowerCase();
  return entries
    .filter(
      (entry) =>
        needle === '' ||
        entry.title.toLowerCase().includes(needle) ||
        entry.subtitle.toLowerCase().includes(needle) ||
        entry.layerId.toLowerCase().includes(needle),
    )
    .map((entry) => ({
      id: String(entry.id),
      label: labelOf(entry),
      depth: 0,
      hasChildren: false,
      expanded: false,
      tags: [entry.layerId, clockTime(entry.time)],
    }));
}

/** The rows of one event: what it is, then its data's keys (or the one value). */
export function eventRows(entry: TimelineEntry): StateRow[] {
  const rows: StateRow[] = [
    { section: 'event', key: 'layer', value: entry.layerId },
    { section: 'event', key: 'time', value: clockTime(entry.time) },
  ];
  if (entry.title !== '') rows.push({ section: 'event', key: 'title', value: entry.title });
  if (entry.subtitle !== '') rows.push({ section: 'event', key: 'subtitle', value: entry.subtitle });
  if (entry.groupId !== undefined) rows.push({ section: 'event', key: 'group', value: String(entry.groupId) });
  if (entry.logType !== 'default') rows.push({ section: 'event', key: 'logType', value: entry.logType });
  if (isObject(entry.data)) {
    for (const [key, value] of Object.entries(entry.data)) {
      rows.push({ section: 'data', key, value: formatValue(value) });
    }
  } else if (entry.data !== null && entry.data !== undefined) {
    rows.push({ section: 'data', key: 'value', value: formatValue(entry.data) });
  }
  return rows;
}
