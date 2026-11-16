// The tokens of a JSON text for the settings window's JSON view (RB-080-42): a small pure
// tokenizer of our own, no dependency. A key is the string a colon follows; a string with its
// escapes is one token; the tokens joined are the text again, so nothing is lost and an odd
// character (the text is always what `JSON.stringify` wrote) is passed through as punctuation.

export type JsonTokenKind = 'key' | 'string' | 'number' | 'boolean' | 'null' | 'punct' | 'space';

export interface JsonToken {
  kind: JsonTokenKind;
  text: string;
}

/**
 * The classes of each kind: `json-<kind>` for tests and styling, and the colour as a Nuxt UI
 * text token, so light and dark themes both work without a colour value of our own.
 */
export const JSON_TOKEN_CLASSES: Readonly<Record<JsonTokenKind, string>> = {
  key: 'json-key text-primary font-semibold',
  string: 'json-string text-success',
  number: 'json-number text-warning',
  boolean: 'json-boolean text-info',
  null: 'json-null text-muted italic',
  punct: 'json-punct text-dimmed',
  space: '',
};

const SPACE = /^\s+/;
const NUMBER = /^-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/;
const WORDS: Record<string, JsonTokenKind> = { true: 'boolean', false: 'boolean', null: 'null' };
const WORD = /^(?:true|false|null)/;
const PUNCT = new Set(['{', '}', '[', ']', ':', ',']);

/** The end of the string starting at `start` (a `"`): the index after its closing quote. */
function stringEnd(text: string, start: number): number {
  let i = start + 1;
  while (i < text.length) {
    const char = text[i];
    if (char === '\\') {
      i += 2;
    } else if (char === '"') {
      return i + 1;
    } else {
      i += 1;
    }
  }
  return text.length;
}

/** Whether a colon follows `index`, after whitespace: the string before it is a key. */
function colonFollows(text: string, index: number): boolean {
  let i = index;
  while (i < text.length && (text[i] === ' ' || text[i] === '\t')) {
    i += 1;
  }
  return text[i] === ':';
}

/** The tokens of `text`, in order; joined they are `text` again. */
export function tokenizeJson(text: string): JsonToken[] {
  const tokens: JsonToken[] = [];
  let i = 0;
  while (i < text.length) {
    const char = text[i] ?? '';
    if (char === '"') {
      const end = stringEnd(text, i);
      tokens.push({ kind: colonFollows(text, end) ? 'key' : 'string', text: text.slice(i, end) });
      i = end;
      continue;
    }
    if (PUNCT.has(char)) {
      tokens.push({ kind: 'punct', text: char });
      i += 1;
      continue;
    }
    const rest = text.slice(i);
    const space = SPACE.exec(rest);
    if (space) {
      tokens.push({ kind: 'space', text: space[0] });
      i += space[0].length;
      continue;
    }
    const number = NUMBER.exec(rest);
    if (number) {
      tokens.push({ kind: 'number', text: number[0] });
      i += number[0].length;
      continue;
    }
    const word = WORD.exec(rest);
    if (word) {
      tokens.push({ kind: WORDS[word[0]] ?? 'punct', text: word[0] });
      i += word[0].length;
      continue;
    }
    tokens.push({ kind: 'punct', text: char });
    i += 1;
  }
  return tokens;
}

/**
 * The tokens of `text` line by line, for a view that renders lines: the text's own newlines
 * separate the lines (none is a token), a trailing newline ends the last line and adds none.
 */
export function tokenizeJsonLines(text: string): JsonToken[][] {
  const lines = text.split('\n');
  if (lines[lines.length - 1] === '') {
    lines.pop();
  }
  return lines.map(tokenizeJson);
}
