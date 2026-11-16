// Parsing a JSON document, and where it is wrong when it does not parse. `JSON.parse` does the
// parsing (fast, native); its message names a position in some engines and versions only, so on
// a failure a small scanner of the JSON grammar (RFC 8259) finds the first offending character
// itself. The scanner runs on failures only and keeps no stack of its own beyond the brackets,
// so a deeply nested document cannot overflow it.

export type ErrorReason = 'unexpected' | 'end';

export interface JsonError {
  /** UTF-16 offset of the first character that cannot be there (the text's length at its end). */
  offset: number;
  reason: ErrorReason;
}

export type Parsed =
  | { ok: true; value: unknown }
  | { ok: false; error: JsonError & { line: number; column: number } };

const WS = new Set([' ', '\t', '\n', '\r']);
const ESCAPES = new Set(['"', '\\', '/', 'b', 'f', 'n', 'r', 't']);
const HEX = /^[0-9a-fA-F]{4}$/;
const LITERALS = ['true', 'false', 'null'];

type State = 'value' | 'valueOrClose' | 'after' | 'keyOrClose' | 'key' | 'colon';

/** The end of the string starting at `start` (its opening quote), past the closing quote. */
function scanString(text: string, start: number): number | JsonError {
  let i = start + 1;
  while (i < text.length) {
    const c = text.charCodeAt(i);
    if (c === 0x22) return i + 1;
    if (c < 0x20) return { offset: i, reason: 'unexpected' };
    if (c === 0x5c) {
      const next = text[i + 1];
      if (next === undefined) return { offset: i + 1, reason: 'end' };
      if (next === 'u') {
        if (!HEX.test(text.slice(i + 2, i + 6))) {
          return i + 6 > text.length ? { offset: text.length, reason: 'end' } : { offset: i + 1, reason: 'unexpected' };
        }
        i += 6;
        continue;
      }
      if (!ESCAPES.has(next)) return { offset: i + 1, reason: 'unexpected' };
      i += 2;
      continue;
    }
    i += 1;
  }
  return { offset: text.length, reason: 'end' };
}

function digits(text: string, start: number): number {
  let i = start;
  while (i < text.length && text.charCodeAt(i) >= 0x30 && text.charCodeAt(i) <= 0x39) i += 1;
  return i;
}

/** The end of the number starting at `start`. */
function scanNumber(text: string, start: number): number | JsonError {
  const fail = (at: number): JsonError =>
    at >= text.length ? { offset: at, reason: 'end' } : { offset: at, reason: 'unexpected' };
  let i = start;
  if (text[i] === '-') i += 1;
  if (text[i] === '0') i += 1;
  else {
    const end = digits(text, i);
    if (end === i) return fail(i);
    i = end;
  }
  if (text[i] === '.') {
    const end = digits(text, i + 1);
    if (end === i + 1) return fail(end);
    i = end;
  }
  if (text[i] === 'e' || text[i] === 'E') {
    i += 1;
    if (text[i] === '+' || text[i] === '-') i += 1;
    const end = digits(text, i);
    if (end === i) return fail(i);
    i = end;
  }
  return i;
}

/** The end of the scalar value at `i`, or the error. */
function scanScalar(text: string, i: number): number | JsonError {
  const c = text[i];
  if (c === '"') return scanString(text, i);
  if (c === '-' || (c !== undefined && c >= '0' && c <= '9')) return scanNumber(text, i);
  for (const literal of LITERALS) {
    if (c === literal[0]) {
      return text.startsWith(literal, i) ? i + literal.length : { offset: i, reason: 'unexpected' };
    }
  }
  return { offset: i, reason: 'unexpected' };
}

/** The first error of `text` as JSON, or null when it is valid. */
export function locateJsonError(text: string): JsonError | null {
  const stack: string[] = [];
  let state: State = 'value';
  let i = 0;
  for (;;) {
    while (i < text.length && WS.has(text[i] as string)) i += 1;
    const c = text[i];
    if (c === undefined) {
      return state === 'after' && stack.length === 0 ? null : { offset: i, reason: 'end' };
    }
    switch (state) {
      case 'value':
      case 'valueOrClose': {
        if (state === 'valueOrClose' && c === ']') {
          stack.pop();
          i += 1;
          state = 'after';
        } else if (c === '{') {
          stack.push('}');
          i += 1;
          state = 'keyOrClose';
        } else if (c === '[') {
          stack.push(']');
          i += 1;
          state = 'valueOrClose';
        } else {
          const end = scanScalar(text, i);
          if (typeof end !== 'number') return end;
          i = end;
          state = 'after';
        }
        break;
      }
      case 'after': {
        const close = stack[stack.length - 1];
        if (close === undefined) return { offset: i, reason: 'unexpected' };
        if (c === ',') {
          state = close === '}' ? 'key' : 'value';
          i += 1;
        } else if (c === close) {
          stack.pop();
          i += 1;
        } else {
          return { offset: i, reason: 'unexpected' };
        }
        break;
      }
      case 'keyOrClose':
      case 'key': {
        if (state === 'keyOrClose' && c === '}') {
          stack.pop();
          i += 1;
          state = 'after';
          break;
        }
        if (c !== '"') return { offset: i, reason: 'unexpected' };
        const end = scanString(text, i);
        if (typeof end !== 'number') return end;
        i = end;
        state = 'colon';
        break;
      }
      case 'colon': {
        if (c !== ':') return { offset: i, reason: 'unexpected' };
        i += 1;
        state = 'value';
        break;
      }
    }
  }
}

/** Line and column (both from 1, the column in UTF-16 units) of `offset`. */
export function lineColumn(text: string, offset: number): { line: number; column: number } {
  let line = 1;
  let start = 0;
  let at = text.indexOf('\n');
  while (at !== -1 && at < offset) {
    line += 1;
    start = at + 1;
    at = text.indexOf('\n', start);
  }
  return { line, column: offset - start + 1 };
}

/** `text` parsed, or where it is wrong. */
export function parseJson(text: string): Parsed {
  try {
    return { ok: true, value: JSON.parse(text) as unknown };
  } catch {
    // The engine's message is not used: it differs between engines and may quote the document.
    const error = locateJsonError(text) ?? { offset: 0, reason: 'unexpected' as const };
    return { ok: false, error: { ...error, ...lineColumn(text, error.offset) } };
  }
}
