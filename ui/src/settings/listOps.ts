// The operations of an entry list in the settings (RB-080-13): move an entry, copy one with an id
// of its own, find a name a map does not hold yet. Pure, so the list component only renders.

import type { JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

/** `items` with the entry at `from` moved to `to`; unchanged when either is out of range. */
export function moveItem<T>(items: readonly T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) {
    return [...items];
  }
  const next = [...items];
  const [item] = next.splice(from, 1);
  if (item !== undefined) {
    next.splice(to, 0, item);
  }
  return next;
}

/** `base`, or `base-2`, `base-3`, ... - the first one `taken` does not hold. */
export function uniqueName(base: string, taken: ReadonlySet<string>): string {
  if (!taken.has(base)) {
    return base;
  }
  let n = 2;
  while (taken.has(`${base}-${n}`)) {
    n += 1;
  }
  return `${base}-${n}`;
}


/** A deep copy of `item`; an `id` gets the suffix `-copy` (numbered when taken), since ids are
 * unique among the entries (a breakpoint's id is referenced by the grid's cells). */
export function copyItem(item: JsonValue, items: readonly JsonValue[]): JsonValue {
  // A JSON round trip, not `structuredClone`: the entry may be a reactive proxy, which does not clone.
  const copy = JSON.parse(JSON.stringify(item)) as JsonValue;
  if (isObject(copy) && typeof copy.id === 'string') {
    const taken = new Set(items.flatMap((other) => (isObject(other) && typeof other.id === 'string' ? [other.id] : [])));
    copy.id = uniqueName(`${copy.id}-copy`, taken);
  }
  return copy;
}
