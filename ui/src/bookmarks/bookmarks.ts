// Project bookmarks in the shell (RB-070-06): the list the core sends with each bookmark's URL in
// the current environment, grouped for the toolbar's list and searched by the quick pick; the
// commands both run. A bookmark holds a path, not a host (RB-080-48) - but which path a page has
// under the environments is the core's alone to say (`rb_config::bookmark_paths`): the dialog
// sends what was typed and its switch (`follow`), nothing here computes a path or a URL.

import type { BookmarkSummary, JsonValue, ShellToCore, StateSnapshot } from '../ipc/protocol';

export const OPEN_COMMAND = 'bookmarks.open';
export const ADD_COMMAND = 'bookmarks.add';

/** Which file a new bookmark goes to. */
export type BookmarkScope = 'project' | 'local';

/** Where a bookmark leads: a path after the environment's URL, or an absolute URL. */
export type BookmarkTarget = { path: string } | { url: string };

/** One group of the toolbar list; `name` is null for the bookmarks without a group. */
export interface BookmarkGroup {
  name: string | null;
  items: BookmarkSummary[];
}

/** What the dialog sends. */
export interface BookmarkDraft {
  label: string;
  target: BookmarkTarget;
  /** The dialog's switch: the core stores a path when the target has one, else the URL. */
  follow: boolean;
  group: string;
  scope: BookmarkScope;
}

/** The project's bookmarks in config order. */
export function bookmarksOf(state: StateSnapshot | null): BookmarkSummary[] {
  return state?.bookmarks ?? [];
}

/** The bookmarks by group: those without one first, then each group where it first appears. */
export function groupBookmarks(list: BookmarkSummary[]): BookmarkGroup[] {
  const groups: BookmarkGroup[] = [];
  const loose: BookmarkSummary[] = [];
  for (const bookmark of list) {
    if (bookmark.group === undefined || bookmark.group === '') {
      loose.push(bookmark);
      continue;
    }
    const group = groups.find((g) => g.name === bookmark.group);
    if (group === undefined) {
      groups.push({ name: bookmark.group, items: [bookmark] });
    } else {
      group.items.push(bookmark);
    }
  }
  return loose.length > 0 ? [{ name: null, items: loose }, ...groups] : groups;
}

/** The bookmarks whose label, group, path or URL hold every word of `query` (any case). */
export function filterBookmarks(list: BookmarkSummary[], query: string): BookmarkSummary[] {
  const words = query.toLowerCase().split(/\s+/).filter((word) => word !== '');
  return list.filter((bookmark) => {
    const text = [bookmark.label, bookmark.group ?? '', bookmark.path ?? '', bookmark.url ?? '']
      .join(' ')
      .toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

const WEB_URL = /^https?:\/\/[^/?#\s]+[^\s]*$/i;
// One `/` first and no backslash: `//host` and `/\\host` are another host to a browser. The
// core's check (`is_bookmark_path`) is the authority; this only marks the field at once.
const PATH = /^\/(?!\/)[^\s\\]*$/;

/** The setting that stores a new bookmark of an environment's page as its path. */
export const FOLLOW_KEY = 'bookmarks.followEnvironment';

/** Whether new bookmarks follow the environment: on unless the setting says no. */
export function followsEnvironment(settings: Record<string, JsonValue>): boolean {
  return settings[FOLLOW_KEY] !== false;
}

/** A typed target: `/...` is a path, an http(s) URL a URL; null for anything else. */
export function targetOf(text: string): BookmarkTarget | null {
  const value = text.trim();
  if (PATH.test(value)) {
    return { path: value };
  }
  return WEB_URL.test(value) ? { url: value } : null;
}

/** `bookmarks.open` for one bookmark. */
export function openMessage(id: string): ShellToCore {
  return { type: 'runCommand', id: OPEN_COMMAND, args: { id } };
}

/** `bookmarks.add` with what the dialog holds; empty label and group, and the form the
 *  bookmark is stored in, are left to the core. */
export function addMessage(draft: BookmarkDraft): ShellToCore {
  const label = draft.label.trim();
  const group = draft.group.trim();
  return {
    type: 'runCommand',
    id: ADD_COMMAND,
    args: {
      ...(label === '' ? {} : { label }),
      ...draft.target,
      follow: draft.follow,
      ...(group === '' ? {} : { group }),
      scope: draft.scope,
    },
  };
}
