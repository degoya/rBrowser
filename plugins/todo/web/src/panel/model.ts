// What the todo panel reads from the component (RB-110-13): the files of `list` and of every
// change's answer (in pages, `load.ts`), each item with its line, depth, kind, state, a Markdown
// heading's level (RB-110-15), text and tags; how a heading or project stands out; what it shows of
// them under the settings and the tag filter; and the answers that mean "the file changed
// meanwhile", after which it reads the files again instead of writing.

import { searchItems, searching, type Search } from './search';
import { isObject } from '../../../../web-common/src/is-object';

export type State = 'open' | 'done' | 'cancelled';

export interface Tag {
  name: string;
  value: string | null;
}

export interface Item {
  line: number;
  depth: number;
  kind: 'project' | 'heading' | 'task' | 'note';
  state: State | null;
  /** A Markdown heading's level, 1 to 6; `null` for every other kind. */
  level: number | null;
  text: string;
  raw: string;
  tags: Tag[];
  project: string;
  archived: boolean;
  /** A line too long for an answer: its texts and tags are shortened, so it is not edited. */
  cut: boolean;
}

export interface TodoFile {
  path: string;
  rev: string;
  items: Item[];
  counts: { open: number; done: number; cancelled: number };
  /** A file that does not read: its failure's code and parameters. */
  error: { code: string; params: Record<string, string> } | null;
}

export interface Listing {
  files: TodoFile[];
  truncated: boolean;
}

/** What the answer of a change hands back to the card that asked for it. */
export type Done = (answer: Record<string, unknown>) => void;

/** The panel's own settings (`todo.*`). */
export interface PanelSettings {
  hideDone: boolean;
  hideArchive: boolean;
  /** 0: read again only on request. */
  refreshSeconds: number;
}

/** The project Todo+ archives into. */
export const ARCHIVE = 'Archive';
/** Tags the plugin writes itself: no filter offers them. */
const OWN_TAGS = new Set(['done', 'cancelled', 'project', 'lasted', 'wasted']);
/** The codes of a change refused because the file changed since it was shown. */
export const CONFLICTS: readonly string[] = ['plugin.todo.errors.changed', 'plugins.error.write_conflict'];

const text = (value: unknown): string => (typeof value === 'string' ? value : '');
const count = (value: unknown): number => (typeof value === 'number' && Number.isFinite(value) ? value : 0);

function readItem(value: unknown): Item | null {
  if (!isObject(value) || typeof value.line !== 'number') return null;
  const kind = value.kind === 'project' || value.kind === 'heading' || value.kind === 'task' ? value.kind : 'note';
  const state = value.state === 'open' || value.state === 'done' || value.state === 'cancelled' ? value.state : null;
  const tags = Array.isArray(value.tags)
    ? value.tags.filter(isObject).map((tag) => ({ name: text(tag.name), value: typeof tag.value === 'string' ? tag.value : null }))
    : [];
  return {
    line: value.line,
    depth: count(value.depth),
    kind,
    state: kind === 'task' ? (state ?? 'open') : null,
    level: kind === 'heading' ? Math.min(6, Math.max(1, Math.floor(count(value.level)))) : null,
    text: text(value.text),
    raw: text(value.raw),
    tags,
    project: text(value.project),
    archived: value.archived === true,
    cut: value.cut === true,
  };
}

/** One file of an answer; `null` for anything else. */
export function readFile(value: unknown): TodoFile | null {
  if (!isObject(value) || typeof value.path !== 'string') return null;
  const counts = isObject(value.counts) ? value.counts : {};
  const params = isObject(value.params) ? value.params : {};
  return {
    path: value.path,
    rev: text(value.rev),
    items: Array.isArray(value.items) ? value.items.map(readItem).filter((item): item is Item => item !== null) : [],
    counts: { open: count(counts.open), done: count(counts.done), cancelled: count(counts.cancelled) },
    error:
      typeof value.error === 'string'
        ? { code: value.error, params: Object.fromEntries(Object.entries(params).map(([key, param]) => [key, String(param)])) }
        : null,
  };
}

/** The answer of `list`. */
export function readListing(value: unknown): Listing {
  if (!isObject(value)) return { files: [], truncated: false };
  const files = Array.isArray(value.files) ? value.files.map(readFile).filter((file): file is TodoFile => file !== null) : [];
  return { files, truncated: value.truncated === true };
}

/** The panel's settings from the settings message; the defaults for what is missing. */
export function panelSettings(settings: Record<string, unknown>): PanelSettings {
  const seconds = settings['todo.refreshSeconds'];
  return {
    hideDone: settings['todo.hideDone'] === true,
    hideArchive: settings['todo.hideArchive'] === true,
    refreshSeconds: typeof seconds === 'number' && seconds >= 0 && seconds <= 300 ? Math.floor(seconds) : 5,
  };
}

/** Whether `item` is the top-level `Archive:` project. */
export const isArchiveHead = (item: Item): boolean => item.kind === 'project' && item.depth === 0 && item.text === ARCHIVE;

const finished = (item: Item): boolean => item.state === 'done' || item.state === 'cancelled';

/** The items of the section at `line` (RB-110-16): a heading with everything below it up to the
 * next heading of its level or a higher one at its depth, a less deep line or the archive; a
 * project or task with its block (the deeper lines after it). `null` for the top level (0) or a
 * line the file no longer has. */
export function sectionOf(items: Item[], line: number): Item[] | null {
  const at = items.findIndex((item) => item.line === line);
  const head = items[at];
  if (line <= 0 || head === undefined) return null;
  const section = [head];
  for (const next of items.slice(at + 1)) {
    if (next.archived || isArchiveHead(next) || next.depth < head.depth) break;
    if (head.kind === 'heading') {
      if (next.kind === 'heading' && next.depth === head.depth && (next.level ?? 1) <= (head.level ?? 1)) break;
    } else if (next.depth <= head.depth) {
      break;
    }
    section.push(next);
  }
  return section;
}

/** What a file shows: its items outside the archive (finished ones hidden on request, only the
 * tasks carrying `tag` while one is chosen, only the section at line `section` while one is
 * chosen, only the matches of `find` with their context while one is typed - RB-110-18) and the
 * archived ones apart - none while a section is chosen. */
export function visible(
  items: Item[],
  settings: PanelSettings,
  tag: string | null,
  section = 0,
  find: Search | null = null,
): { active: Item[]; archive: Item[] } {
  const keep = (item: Item): boolean => {
    if (settings.hideDone && finished(item)) return false;
    if (tag !== null) return item.kind === 'task' && item.tags.some((own) => own.name === tag);
    return true;
  };
  const pick = (list: Item[]): Item[] => (searching(find) ? searchItems(list, find, keep) : list.filter(keep));
  const chosen = sectionOf(items, section);
  if (chosen !== null) return { active: pick(chosen), archive: [] };
  const active = pick(items.filter((item) => !item.archived && !isArchiveHead(item)));
  const archive = settings.hideArchive ? [] : pick(items.filter((item) => item.archived));
  return { active, archive };
}

/** The tags of every file, sorted, without those the plugin writes itself. */
export function tagNames(files: TodoFile[]): string[] {
  const names = new Set<string>();
  for (const file of files) {
    for (const item of file.items) {
      for (const tag of item.tags) if (tag.name && !OWN_TAGS.has(tag.name)) names.add(tag.name);
    }
  }
  return [...names].sort();
}

/** Where a new task of `file` can go: the top level (line 0) and every project and Markdown
 * heading outside the archive, by line; a heading named with its `#`s. */
export function addTargets(file: TodoFile): { line: number; label: string }[] {
  return file.items
    .filter((item) => (item.kind === 'project' || item.kind === 'heading') && !item.archived && !isArchiveHead(item))
    .map((item) => {
      if (item.kind === 'heading') return { line: item.line, label: `${'#'.repeat(item.level ?? 1)} ${item.text}` };
      return { line: item.line, label: item.project ? `${item.project}.${item.text}` : item.text };
    });
}

/** How a row looks (RB-110-15): the classes of the row and of its text, and its level as a
 * heading (below the file's own `h2`), `null` for no heading. The icons are `TodoRow.vue`'s: the
 * icon bundle finds only names written in a `.vue` file. */
export interface Look {
  row: string;
  text: string;
  ariaLevel: number | null;
}

/** A heading's text by level (RB-110-16): six sizes, each a step apart, in rem so they grow with
 * the system's text size; the weight and colour step down with them. */
const HEADING_TEXT = [
  'text-2xl font-bold text-highlighted',
  'text-xl font-bold text-highlighted',
  'text-lg font-semibold text-highlighted',
  'text-base font-semibold text-highlighted',
  'text-sm font-semibold text-default',
  'text-xs font-semibold uppercase tracking-wide text-muted',
];
/** A heading's room above it by level; the two top levels underlined, the first more strongly. */
const HEADING_ROW = ['mt-6 border-b-2 border-default pb-1.5', 'mt-5 border-b border-default pb-1', 'mt-4', 'mt-3', 'mt-2', 'mt-1.5'];

/** How `item` stands out: a Markdown heading by its level, a project as a heading of its own
 * (the top-level ones larger), a note muted and italic, a task as plain text. */
export function look(item: Item): Look {
  if (item.kind === 'heading') {
    const at = Math.min(6, Math.max(1, item.level ?? 1)) - 1;
    return { row: `${HEADING_ROW[at] ?? ''} first:mt-0`, text: HEADING_TEXT[at] ?? '', ariaLevel: Math.min(6, at + 3) };
  }
  if (item.kind === 'project') {
    const top = item.depth === 0;
    return {
      row: top ? 'mt-3 first:mt-0' : 'mt-1.5',
      text: top ? 'text-base font-bold text-highlighted' : 'text-sm font-bold text-highlighted',
      ariaLevel: Math.min(6, item.depth + 3),
    };
  }
  if (item.kind === 'note') return { row: '', text: 'text-sm text-muted italic', ariaLevel: null };
  return { row: '', text: 'text-sm', ariaLevel: null };
}

/** Whether a failed call says the file changed since it was shown. */
export function isConflict(error: unknown): boolean {
  return error instanceof Error && CONFLICTS.includes(error.message);
}

/** Minutes east of UTC of this computer, which `@done(...)` is written in. */
export function utcOffset(date: Date = new Date()): number {
  return -date.getTimezoneOffset();
}

/** The `failed` list of an `archive` answer: each file's failure as the error a call throws, its
 * code the message and its parameters attached, so the panel says it in one sentence. */
export function readFailures(value: unknown): Error[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isObject).flatMap((entry) => {
    if (typeof entry.error !== 'string') return [];
    const params = isObject(entry.params) ? Object.fromEntries(Object.entries(entry.params).map(([key, param]) => [key, String(param)])) : {};
    return [Object.assign(new Error(entry.error), { params })];
  });
}
