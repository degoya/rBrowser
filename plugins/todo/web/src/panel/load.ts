// The todo panel's reads (RB-110-13, the owner's acceptance 2026-11-16): a panel's call and its
// answer are at most 64 KiB of JSON, so `list` answers in pages. The panel reads the files' heads
// (path, rev, counts) page by page, then the items of every file whose rev it does not show yet,
// page by page from `next` with the rev of the file's first page; a file that changes between two
// pages (`errors.changed`) is read again from its start. The look for changes stays cheap: the
// heads only, and the items of the files that changed.
import { isConflict, readFile, readListing, type Listing, type TodoFile } from './model';

/** `list` with its arguments. */
export type ListCall = (args: Record<string, unknown>) => Promise<unknown>;

/** The largest `files` argument of one call, in bytes of JSON: well below the 64 KiB message. */
export const MAX_ARGS_BYTES = 32 * 1024;
/** How often a file that changes while its pages are read is read again from its start. */
export const MAX_RESTARTS = 3;

const encoder = new TextEncoder();
const jsonBytes = (value: unknown): number => encoder.encode(JSON.stringify(value)).length;
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;

/** `paths` in groups whose JSON stays within `budget` bytes (a longer path in a group alone). */
export function chunks(paths: string[], budget = MAX_ARGS_BYTES): string[][] {
  const groups: string[][] = [];
  let group: string[] = [];
  let used = 2;
  for (const path of paths) {
    const size = jsonBytes(path) + 1;
    if (group.length > 0 && used + size > budget) {
      groups.push(group);
      group = [];
      used = 2;
    }
    group.push(path);
    used += size;
  }
  if (group.length > 0) groups.push(group);
  return groups;
}

/** The `next` of a page: where to go on, only when it lies past `from` (never a loop). */
function nextOf(page: unknown, from: number): number | null {
  const next = isObject(page) ? page.next : undefined;
  return typeof next === 'number' && Number.isSafeInteger(next) && next > from ? next : null;
}

/** The heads of the files - every todo file of the project (`null`), or `paths` - page by page. */
export async function listHeads(call: ListCall, paths: string[] | null): Promise<Listing> {
  const files: TodoFile[] = [];
  let truncated = false;
  for (const group of paths === null ? [null] : chunks(paths)) {
    let from = 0;
    for (;;) {
      const args: Record<string, unknown> = group === null ? {} : { files: group };
      if (from > 0) args.from = from;
      const page = await call(args);
      const listing = readListing(page);
      files.push(...listing.files);
      truncated ||= listing.truncated;
      const next = nextOf(page, from);
      if (next === null) break;
      from = next;
    }
  }
  return { files, truncated };
}

/** A file that does not read: its failure's code and parameters, no items. */
function failedFile(path: string, error: unknown): TodoFile {
  const params = isObject(error) && isObject(error.params) ? error.params : {};
  return {
    path,
    rev: '',
    items: [],
    counts: { open: 0, done: 0, cancelled: 0 },
    error: {
      code: error instanceof Error ? error.message : String(error),
      params: Object.fromEntries(Object.entries(params).map(([key, param]) => [key, String(param)])),
    },
  };
}

/** One file with all its items, page by page from `first` (the page a change answers) or from
 * its start. A file that changes meanwhile is read again from its start, `MAX_RESTARTS` times;
 * then the conflict is the caller's. Any other failure is the file's, shown on its card. */
export async function loadFile(call: ListCall, path: string, first?: unknown): Promise<TodoFile> {
  let page = first;
  for (let attempt = 0; ; attempt += 1) {
    try {
      page ??= await call({ file: path });
      const head = readFile(page) ?? { path, rev: '', items: [], counts: { open: 0, done: 0, cancelled: 0 }, error: null };
      const items = [...head.items];
      let from = 0;
      let next = nextOf(page, from);
      while (next !== null) {
        from = next;
        const more = await call({ file: head.path, from, rev: head.rev });
        items.push(...(readFile(more)?.items ?? []));
        next = nextOf(more, from);
      }
      return { ...head, items };
    } catch (error) {
      if (!isConflict(error)) return failedFile(path, error);
      if (attempt >= MAX_RESTARTS) throw error;
      page = undefined;
    }
  }
}

/** The files as they are now: the heads of every file (`all`) or of those `shown`, and the items
 * of each file whose rev `shown` does not have; the others are kept as shown. */
export async function readFiles(call: ListCall, shown: TodoFile[], all: boolean): Promise<Listing> {
  const listing = await listHeads(call, all ? null : shown.map((file) => file.path));
  const files: TodoFile[] = [];
  for (const head of listing.files) {
    const known = shown.find((file) => file.path === head.path && file.error === null && file.rev === head.rev);
    if (head.error) files.push(head);
    else files.push(known ?? (await loadFile(call, head.path)));
  }
  return { files, truncated: listing.truncated };
}
