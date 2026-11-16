// What one measurement answers the component at most (RB-090-17).

/** The longest selector, path or label an answer keeps (src/scan.rs `MAX_TEXT_CHARS`). */
export const MAX_TEXT_CHARS = 300;
/** The most bytes of JSON text the items of one measurement take (src/budget.rs: the message
 *  limit of 64 KiB less the answer's reserve). */
export const MAX_ANSWER_BYTES = 48 * 1024;

const encoder = new TextEncoder();

/** `text` within {@link MAX_TEXT_CHARS} characters (code points, as the component counts). */
export function cut(text: string): string {
  return text.length <= MAX_TEXT_CHARS ? text : Array.from(text).slice(0, MAX_TEXT_CHARS).join('');
}

/** The first of `items` whose JSON text fits `budget` bytes together, the commas included. */
export function fitting<T>(items: readonly T[], budget = MAX_ANSWER_BYTES): T[] {
  const kept: T[] = [];
  let used = 0;
  for (const item of items) {
    used += encoder.encode(JSON.stringify(item)).length + 1;
    if (used > budget) break;
    kept.push(item);
  }
  return kept;
}
