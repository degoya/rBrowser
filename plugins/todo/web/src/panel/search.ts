// The todo panel's search (RB-110-18), the same rules as `todo_list`'s `query` (src/search.rs):
// the terms split at white space, each without case and accents; a term `@name` is a tag whose
// name starts with `name`, any other a word that the item's text, a tag's name or a tag's value
// contains. An item matches when every term does - and, with "only open", when it is a task left
// to do. The list keeps each match's headings, projects and parent lines above it as context.

import type { Item, Tag } from './model';

export interface Query {
  /** The words, folded. */
  words: string[];
  /** The tag terms' names without `@`, folded. */
  tags: string[];
}

export interface Search {
  query: Query;
  /** Only tasks left to do match. */
  openOnly: boolean;
}

const MARK = /\p{M}/gu;
const ASCII = /^[\x00-\x7f]*$/;

/** `char` without case and accents: decomposed, its marks dropped, lower case. */
function foldChar(char: string): string {
  return char.normalize('NFD').replace(MARK, '').toLowerCase();
}

/** `text` without case and accents, character by character (as `src/search.rs` folds it). */
export function fold(text: string): string {
  if (ASCII.test(text)) return text.toLowerCase();
  let folded = '';
  for (const char of text) folded += foldChar(char);
  return folded;
}

/** The terms of a search text; `@` alone and terms that fold to nothing are left out. */
export function parseQuery(text: string): Query {
  const query: Query = { words: [], tags: [] };
  for (const term of text.split(/\s+/)) {
    if (term.startsWith('@')) {
      const name = fold(term.slice(1).split('(')[0] ?? '');
      if (name !== '') query.tags.push(name);
    } else {
      const word = fold(term);
      if (word !== '') query.words.push(word);
    }
  }
  return query;
}

/** Whether `search` filters at all. */
export const searching = (search: Search | null): search is Search =>
  search !== null && (search.openOnly || search.query.words.length > 0 || search.query.tags.length > 0);

/** The folded texts an item is searched in, kept per item: a file of thousands of lines is
 * searched again on every key. */
const haystacks = new WeakMap<Item, { text: string; tags: { name: string; value: string }[] }>();

function haystack(item: Item): { text: string; tags: { name: string; value: string }[] } {
  let found = haystacks.get(item);
  if (!found) {
    found = { text: fold(item.text), tags: item.tags.map((tag) => ({ name: fold(tag.name), value: fold(tag.value ?? '') })) };
    haystacks.set(item, found);
  }
  return found;
}

/** Whether `item` matches every term of `search`. */
export function matches(item: Item, search: Search): boolean {
  if (search.openOnly && !(item.kind === 'task' && item.state === 'open')) return false;
  const { words, tags } = search.query;
  if (words.length === 0 && tags.length === 0) return true;
  const own = haystack(item);
  return (
    words.every((word) => own.text.includes(word) || own.tags.some((tag) => tag.name.includes(word) || tag.value.includes(word))) &&
    tags.every((name) => own.tags.some((tag) => tag.name.startsWith(name)))
  );
}

/** Whether `tag` is one a term of `query` found: a word in its name or value, a tag term at the
 * start of its name. */
export function tagHit(tag: Tag, query: Query): boolean {
  const name = fold(tag.name);
  const value = fold(tag.value ?? '');
  return query.words.some((word) => name.includes(word) || value.includes(word)) || query.tags.some((term) => name.startsWith(term));
}

/** Whether `head` holds `item`, the next line after everything it held so far: a deeper line, or
 * - for a Markdown heading - a line at its depth that is no heading of its level or a higher one
 * (`sectionOf` in model.ts). */
function holds(head: Item, item: Item): boolean {
  if (head.depth < item.depth) return true;
  if (head.kind !== 'heading' || head.depth !== item.depth) return false;
  return item.kind !== 'heading' || (item.level ?? 1) > (head.level ?? 1);
}

const isArchiveHead = (item: Item): boolean => item.kind === 'project' && item.depth === 0 && item.text === 'Archive';

/** The items of `list` that match `search` and pass `keep`, each with the lines that hold it
 * (its headings, projects and parents) as context, in file order. One pass: the lines that hold
 * the current one are a stack. */
export function searchItems(list: Item[], search: Search, keep: (item: Item) => boolean = () => true): Item[] {
  const shown = new Set<Item>();
  const stack: Item[] = [];
  for (const item of list) {
    if (isArchiveHead(item)) stack.length = 0;
    while (stack.length > 0 && !holds(stack[stack.length - 1] as Item, item)) stack.pop();
    if (keep(item) && matches(item, search)) {
      // Upwards until a line already shown: its own context is shown with it.
      for (let at = stack.length - 1; at >= 0; at -= 1) {
        const head = stack[at] as Item;
        if (shown.has(head)) break;
        shown.add(head);
      }
      shown.add(item);
    }
    stack.push(item);
  }
  return list.filter((item) => shown.has(item));
}

export interface Part {
  text: string;
  hit: boolean;
}

/** `text` in parts, those where one of `words` (folded) stands marked as hits. */
export function marks(text: string, words: string[]): Part[] {
  if (text === '') return [];
  const terms = words.filter((word) => word !== '');
  if (terms.length === 0) return [{ text, hit: false }];
  // The folded text, and for each of its code units the original's span it came from.
  let folded = '';
  const starts: number[] = [];
  const ends: number[] = [];
  let offset = 0;
  for (const char of text) {
    const own = foldChar(char);
    for (let unit = 0; unit < own.length; unit += 1) {
      starts.push(offset);
      ends.push(offset + char.length);
    }
    folded += own;
    offset += char.length;
  }
  const hit = new Array<boolean>(text.length).fill(false);
  for (const word of terms) {
    for (let at = folded.indexOf(word); at >= 0; at = folded.indexOf(word, at + 1)) {
      const from = starts[at] ?? 0;
      const to = ends[at + word.length - 1] ?? text.length;
      hit.fill(true, from, to);
    }
  }
  const parts: Part[] = [];
  for (let at = 0; at < text.length; at += 1) {
    const last = parts[parts.length - 1];
    const char = text.charAt(at);
    if (last && last.hit === hit[at]) last.text += char;
    else parts.push({ text: char, hit: hit[at] === true });
  }
  return parts;
}
