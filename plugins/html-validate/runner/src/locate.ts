// The element of a line (RB-094-06). The findings panel highlights a finding's element by its
// selector; html-validate names one for most of its messages, the Nu HTML Checker never does
// (its JSON has lines and columns only), and neither does html-validate for a stray end tag.
// Such a message gets the selector of the element at its place: html-validate's parser reads the
// validated source again, and the element is the last one whose start tag begins at or before
// the place - the element of a start tag the place lies in, else the one the place follows. The
// source is parsed once per answer, and only when a message needs it.

import { type HtmlElement, HtmlValidate, Parser } from 'html-validate/browser';
import type { AnswerMessage } from './shape';

/** A 1-based place in the source. */
export interface Place {
  line: number;
  column: number;
}

/** The offset of the 1-based `line` and `column` in `source`; `null` outside it. */
export function offsetOf(source: string, line: number, column: number): number | null {
  if (line < 1 || column < 1) return null;
  let start = 0;
  for (let at = 1; at < line; at += 1) {
    const newline = source.indexOf('\n', start);
    if (newline === -1) return null;
    start = newline + 1;
  }
  const offset = start + column - 1;
  return offset <= source.length ? offset : null;
}

/** Every element of the parsed source with its start offset, in document order. */
function elements(source: string): HtmlElement[] {
  const config = new HtmlValidate({ root: true, extends: [], elements: ['html5'], rules: {} });
  const root = new Parser(config.getConfigForSync('page.html')).parseHtml(source);
  const found: HtmlElement[] = [];
  const walk = (parent: HtmlElement): void => {
    for (const child of parent.childElements) {
      found.push(child);
      walk(child);
    }
  };
  walk(root);
  return found;
}

function lastAtOrBefore(all: HtmlElement[], offset: number): string | null {
  let best: HtmlElement | null = null;
  for (const element of all) {
    if (element.location.offset <= offset) best = element;
  }
  return best?.generateSelector() ?? null;
}

/** The selector of the element at `line` and `column` of `source`; `null` without one. */
export function selectorAt(source: string, line: number, column: number): string | null {
  const offset = offsetOf(source, line, column);
  if (offset === null || source === '') return null;
  return lastAtOrBefore(elements(source), offset);
}

/** `messages` with the selector of the element at their place where they name none. The place
 * looked at is `lookup[i]` when given (Nu's end of the range: inside the start tag), else the
 * message's line and column. */
export function withSelectors(
  source: string,
  messages: AnswerMessage[],
  lookup: (Place | undefined)[] = [],
): AnswerMessage[] {
  const needs = (message: AnswerMessage) => message.selector === null && message.line > 0;
  if (!messages.some(needs)) return messages;
  let all: HtmlElement[] | null = null;
  return messages.map((message, index) => {
    if (!needs(message)) return message;
    const place = lookup[index] ?? message;
    const offset = offsetOf(source, place.line, place.column);
    if (offset === null) return message;
    all ??= elements(source);
    return { ...message, selector: lastAtOrBefore(all, offset) };
  });
}
