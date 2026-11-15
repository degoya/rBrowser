// Which entries the palette lists for a query, in which order (RB-060-11): every word of the
// query must match the title, a keyword or the group (fuzzy, `fuzzy.ts`); the sum of the best
// score per word ranks, a recent entry gets a lift. Without a query a pinned entry of the open
// step comes first, then the recent entries, newest first, then the rest in the providers' order.

import { fuzzyScore } from './fuzzy';
import type { EntryScope, PaletteEntry } from './types';

/** The prefixes and the scope each keeps to. */
export const PREFIXES: Readonly<Record<string, EntryScope>> = { '>': 'command', '@': 'view', '#': 'setting' };

/** A keyword match counts a little less than a title match, the group less again. */
const KEYWORD_PENALTY = 2;
const GROUP_PENALTY = 6;
/** The lift of a recent entry in a ranked list. */
const RECENT_LIFT = 4;

export interface ParsedQuery {
  scope: EntryScope | null;
  text: string;
}

/** Splits a leading prefix (`>`, `@`, `#`) off the typed text when `prefixes` is on. */
export function parseQuery(raw: string, prefixes: boolean): ParsedQuery {
  const trimmed = raw.trimStart();
  const scope = prefixes ? PREFIXES[trimmed.charAt(0)] : undefined;
  if (scope !== undefined) {
    return { scope, text: trimmed.slice(1).trim() };
  }
  return { scope: null, text: trimmed.trim() };
}

function best(scores: (number | null)[]): number | null {
  const found = scores.filter((score): score is number => score !== null);
  return found.length === 0 ? null : Math.max(...found);
}

/** The score of one query word against an entry, or `null` when it matches nothing. */
function wordScore(word: string, entry: PaletteEntry): number | null {
  const title = fuzzyScore(word, entry.title);
  const keyword = best((entry.keywords ?? []).map((k) => fuzzyScore(word, k)));
  const group = entry.group === undefined ? null : fuzzyScore(word, entry.group);
  return best([
    title,
    keyword === null ? null : keyword - KEYWORD_PENALTY,
    group === null ? null : group - GROUP_PENALTY,
  ]);
}

/** The score of `entry` for every word of `words`, or `null` when one word matches nothing. */
function entryScore(words: string[], entry: PaletteEntry): number | null {
  let total = 0;
  for (const word of words) {
    const score = wordScore(word, entry);
    if (score === null) {
      return null;
    }
    total += score;
  }
  return total;
}

export interface RankOptions {
  /** Entry keys, newest first. */
  recent?: readonly string[];
  /** The open argument step; `null` at the top level. */
  step?: string | null;
  /** The scope of a typed prefix. */
  scope?: EntryScope | null;
}

/** The entries for `text` (a query without its prefix), best first. */
export function rank(entries: readonly PaletteEntry[], text: string, options: RankOptions = {}): PaletteEntry[] {
  const recent = options.recent ?? [];
  const step = options.step ?? null;
  const scope = options.scope ?? null;
  const words = text.trim().split(/\s+/).filter((word) => word !== '');
  const visible = entries.filter((entry) => {
    if (step !== null) {
      return entry.parent === step;
    }
    if (scope !== null && entry.scope !== scope) {
      return false;
    }
    // A step's entries join the top level while a query or a prefix narrows it.
    return entry.parent === undefined || words.length > 0 || scope !== null;
  });
  if (words.length === 0) {
    const order = (entry: PaletteEntry): number => {
      // Only in its step: at the top level a bare prefix lists everything (RB-080-68 review).
      if (entry.pinned === true && step !== null) {
        return -1;
      }
      const index = recent.indexOf(entry.key);
      return index === -1 ? recent.length : index;
    };
    return visible
      .map((entry, index) => ({ entry, index }))
      .sort((a, b) => order(a.entry) - order(b.entry) || a.index - b.index)
      .map(({ entry }) => entry);
  }
  return visible
    .map((entry, index) => {
      const score = entryScore(words, entry);
      return { entry, index, score: score === null ? null : score + (recent.includes(entry.key) ? RECENT_LIFT : 0) };
    })
    .filter((row): row is { entry: PaletteEntry; index: number; score: number } => row.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map(({ entry }) => entry);
}
