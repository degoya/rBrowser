// The export preview's syntax highlighting (RB-090-60): the generated TypeScript as tokens -
// comments, strings, keywords, numbers, the rest plain - rendered as text in spans, never as
// HTML, so nothing in a recording can reach the panel's markup. Small on purpose: the code is
// rBrowser's own export, not arbitrary source.

export type TokenKind = 'plain' | 'comment' | 'string' | 'keyword' | 'number';

export interface Token {
  kind: TokenKind;
  text: string;
}

const KEYWORDS = new Set([
  'async',
  'await',
  'const',
  'else',
  'export',
  'false',
  'for',
  'from',
  'function',
  'if',
  'import',
  'let',
  'new',
  'null',
  'return',
  'throw',
  'true',
  'undefined',
]);

/** The end of the string starting at `start` (its quote), past the closing quote. */
function stringEnd(code: string, start: number): number {
  const quote = code.charAt(start);
  let at = start + 1;
  while (at < code.length && code.charAt(at) !== quote) {
    at += code.charAt(at) === '\\' ? 2 : 1;
  }
  return Math.min(at + 1, code.length);
}

/** The token starting at `at`, if one other than plain text does. */
function special(code: string, at: number): Token | null {
  const char = code.charAt(at);
  if (code.startsWith('//', at)) {
    const end = code.indexOf('\n', at);
    return { kind: 'comment', text: code.slice(at, end < 0 ? code.length : end) };
  }
  if (code.startsWith('/*', at)) {
    const end = code.indexOf('*/', at + 2);
    return { kind: 'comment', text: code.slice(at, end < 0 ? code.length : end + 2) };
  }
  if (char === "'" || char === '"' || char === '`') {
    return { kind: 'string', text: code.slice(at, stringEnd(code, at)) };
  }
  const before = at === 0 ? '' : code.charAt(at - 1);
  if (/[A-Za-z_$]/.test(char) && !/[\w$]/.test(before)) {
    const word = /^[A-Za-z_$][\w$]*/.exec(code.slice(at))?.[0] ?? char;
    return KEYWORDS.has(word) ? { kind: 'keyword', text: word } : { kind: 'plain', text: word };
  }
  if (/[0-9]/.test(char) && !/[\w$]/.test(before)) {
    return { kind: 'number', text: /^[0-9][0-9_.]*/.exec(code.slice(at))?.[0] ?? char };
  }
  return null;
}

/** `code` as tokens; joined they are `code` again. */
export function highlight(code: string): Token[] {
  const tokens: Token[] = [];
  const push = (token: Token) => {
    const last = tokens[tokens.length - 1];
    if (token.kind === 'plain' && last?.kind === 'plain') {
      last.text += token.text;
    } else {
      tokens.push({ ...token });
    }
  };
  let at = 0;
  while (at < code.length) {
    const token = special(code, at) ?? { kind: 'plain' as const, text: code.charAt(at) };
    push(token);
    at += token.text.length;
  }
  return tokens;
}
