// The message format of the catalogue, shared with the core (`docs/i18n.md`, rb-i18n's
// `message.rs`): literal text, named placeholders `{name}` and ICU plurals
// `{name, plural, =0 {...} one {...} other {...}}`, where `#` inside a branch is the argument as
// given. There is no quoting: an apostrophe is an apostrophe and a `#` outside a plural is a `#`.
// vue-i18n's own syntax differs (`|` is a plural, `@:` a link), so the shell installs
// `shellMessageCompiler` instead. A message that does not parse is shown as written; the lint and
// the loader refuse it long before it ships.

import type { MessageCompiler, MessageContext } from 'vue-i18n';

export type PluralCategory = 'zero' | 'one' | 'two' | 'few' | 'many' | 'other';

const CATEGORIES: readonly string[] = ['zero', 'one', 'two', 'few', 'many', 'other'];

type Selector = { kind: 'exact'; value: bigint } | { kind: 'category'; value: PluralCategory };

type Part =
  | { kind: 'text'; text: string }
  /** `#` inside a plural branch. */
  | { kind: 'hash' }
  | { kind: 'arg'; name: string; source: string }
  | { kind: 'plural'; name: string; source: string; branches: Branch[] };

type Branch = { selector: Selector; parts: Part[] };

/** Why a message does not parse. */
export class MessageFormatError extends Error {
  override name = 'MessageFormatError';
}

/** The largest selector `=n` the core accepts (`u64`). */
const U64_MAX = (1n << 64n) - 1n;

function isAsciiDigit(c: string | undefined): boolean {
  return c !== undefined && c >= '0' && c <= '9';
}

function isAsciiAlpha(c: string | undefined): boolean {
  return c !== undefined && ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'));
}

/** Rust's `u8::is_ascii_whitespace`: space, tab, line feed, form feed, carriage return. */
function isAsciiSpace(c: string | undefined): boolean {
  return c === ' ' || c === '\t' || c === '\n' || c === '\f' || c === '\r';
}

class Parser {
  private pos = 0;

  constructor(private readonly src: string) {}

  parse(): Part[] {
    return this.message(false);
  }

  private peek(): string | undefined {
    return this.src[this.pos];
  }

  private skipSpace(): void {
    while (isAsciiSpace(this.peek())) {
      this.pos += 1;
    }
  }

  private expect(wanted: string): void {
    if (this.peek() !== wanted) {
      throw new MessageFormatError(`expected \`${wanted}\` at offset ${this.pos}`);
    }
    this.pos += 1;
  }

  /** `[A-Za-z_][A-Za-z0-9_]*`, or an error naming `what`. */
  private identifier(what: string): string {
    const start = this.pos;
    const first = this.peek();
    if (!isAsciiAlpha(first) && first !== '_') {
      throw new MessageFormatError(`expected ${what} at offset ${start}`);
    }
    let c = this.peek();
    while (isAsciiAlpha(c) || isAsciiDigit(c) || c === '_') {
      this.pos += 1;
      c = this.peek();
    }
    return this.src.slice(start, this.pos);
  }

  /** A message up to its end, or - inside a plural branch - up to the `}` that closes it. */
  private message(inBranch: boolean): Part[] {
    const parts: Part[] = [];
    for (;;) {
      const c = this.peek();
      if (c === undefined) {
        if (inBranch) {
          throw new MessageFormatError('a plural branch is not closed');
        }
        return parts;
      }
      if (c === '}') {
        if (inBranch) {
          return parts;
        }
        throw new MessageFormatError(`unmatched \`}\` at offset ${this.pos}`);
      }
      if (c === '{') {
        parts.push(this.argument());
      } else if (c === '#' && inBranch) {
        this.pos += 1;
        parts.push({ kind: 'hash' });
      } else {
        const start = this.pos;
        let d = this.peek();
        while (d !== undefined && d !== '{' && d !== '}' && !(inBranch && d === '#')) {
          this.pos += 1;
          d = this.peek();
        }
        parts.push({ kind: 'text', text: this.src.slice(start, this.pos) });
      }
    }
  }

  /** `{name}` or `{name, plural, <branches>}`, the cursor on its `{`. */
  private argument(): Part {
    const start = this.pos;
    this.pos += 1;
    this.skipSpace();
    const name = this.identifier('an argument name');
    this.skipSpace();
    if (this.peek() === '}') {
      this.pos += 1;
      return { kind: 'arg', name, source: this.src.slice(start, this.pos) };
    }
    this.expect(',');
    this.skipSpace();
    const kind = this.identifier('`plural`');
    if (kind !== 'plural') {
      throw new MessageFormatError(`\`${kind}\` is not supported, only \`plural\``);
    }
    this.skipSpace();
    this.expect(',');
    const branches = this.branches();
    return { kind: 'plural', name, source: this.src.slice(start, this.pos), branches };
  }

  /** The branches of a plural up to and including its closing `}`. */
  private branches(): Branch[] {
    const branches: Branch[] = [];
    for (;;) {
      this.skipSpace();
      const c = this.peek();
      if (c === undefined) {
        throw new MessageFormatError('a plural is not closed');
      }
      if (c === '}') {
        this.pos += 1;
        break;
      }
      const selector = this.selector();
      if (branches.some((b) => sameSelector(b.selector, selector))) {
        throw new MessageFormatError(`a plural names ${String(selector.value)} twice`);
      }
      this.skipSpace();
      this.expect('{');
      const parts = this.message(true);
      this.expect('}');
      branches.push({ selector, parts });
    }
    if (!branches.some((b) => b.selector.kind === 'category' && b.selector.value === 'other')) {
      throw new MessageFormatError('a plural needs an `other` branch');
    }
    return branches;
  }

  private selector(): Selector {
    if (this.peek() === '=') {
      this.pos += 1;
      const start = this.pos;
      while (isAsciiDigit(this.peek())) {
        this.pos += 1;
      }
      const digits = this.src.slice(start, this.pos);
      const value = digits === '' ? -1n : BigInt(digits);
      if (value < 0n || value > U64_MAX) {
        throw new MessageFormatError(`expected a number after \`=\` at offset ${start}`);
      }
      return { kind: 'exact', value };
    }
    const name = this.identifier('a plural category');
    if (!CATEGORIES.includes(name)) {
      throw new MessageFormatError(`\`${name}\` is no plural category`);
    }
    return { kind: 'category', value: name as PluralCategory };
  }
}

function sameSelector(a: Selector, b: Selector): boolean {
  return a.kind === b.kind && a.value === b.value;
}

/** The parts of `message`; throws a `MessageFormatError` when it does not parse. */
function parse(message: string): Part[] {
  return new Parser(message).parse();
}

/** Why `message` does not parse, or `null` when it is well-formed. */
export function validateMessage(message: string): string | null {
  try {
    parse(message);
    return null;
  } catch (error) {
    if (error instanceof MessageFormatError) {
      return error.message;
    }
    throw error;
  }
}

/**
 * The plural category of `value` in English (phase 1 has no other language): `one` for the
 * integer 1 (also `-1`, `01`), `other` for everything else, `1.0` included - CLDR counts the
 * visible fraction digits.
 */
export function pluralCategory(value: string): PluralCategory {
  const trimmed = value.trim();
  const digits = trimmed.startsWith('-') ? trimmed.slice(1) : trimmed;
  return /^[0-9]+$/.test(digits) && /^0*1$/.test(digits) ? 'one' : 'other';
}

/** `value` as a number the way Rust's `str::parse::<f64>` reads it, or `null`. */
function parseNumber(value: string): number | null {
  const trimmed = value.trim();
  return /^[+-]?(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/.test(trimmed)
    ? Number(trimmed)
    : null;
}

/** The branch for `value`: an exact `=n` first, then its category, then `other`. */
function select(branches: Branch[], value: string): Part[] | undefined {
  const number = parseNumber(value);
  const exact = branches.find(
    (b) => b.selector.kind === 'exact' && number !== null && Number(b.selector.value) === number,
  );
  const by = (wanted: PluralCategory) =>
    branches.find((b) => b.selector.kind === 'category' && b.selector.value === wanted);
  return (exact ?? by(pluralCategory(value)) ?? by('other'))?.parts;
}

/** The accented look-alikes of the pseudo-locale `en-XA`. */
const ACCENTS: Readonly<Record<string, string>> = {
  a: 'á',
  c: 'ç',
  e: 'é',
  i: 'í',
  n: 'ñ',
  o: 'ó',
  u: 'ú',
  y: 'ý',
  A: 'Á',
  C: 'Ç',
  E: 'É',
  I: 'Í',
  N: 'Ñ',
  O: 'Ó',
  U: 'Ú',
  Y: 'Ý',
};

function accent(text: string): string {
  return Array.from(text, (c) => ACCENTS[c] ?? c).join('');
}

/** `text` in brackets, padded with `~` to at least 140 % of its length; empty stays empty. */
function pad(text: string): string {
  const length = Array.from(text).length;
  if (length === 0) {
    return '';
  }
  const tildes = Math.max(Math.ceil((length * 2) / 5) - 2, 0);
  return `[${text}${'~'.repeat(tildes)}]`;
}

/** `text` as the pseudo-locale shows it: accented, bracketed, padded. */
export function pseudoLocalize(text: string): string {
  return pad(accent(text));
}

/** The pseudo-locale for layout tests. */
export const PSEUDO_LOCALE = 'en-XA';

/** An argument value as text, or `undefined` when it is missing. */
export type ArgLookup = (name: string) => string | undefined;

function render(parts: Part[], arg: ArgLookup, pseudo: boolean, hash: string | null): string {
  let out = '';
  for (const part of parts) {
    switch (part.kind) {
      case 'text':
        out += pseudo ? accent(part.text) : part.text;
        break;
      case 'hash':
        out += hash ?? '#';
        break;
      case 'arg':
        out += arg(part.name) ?? part.source;
        break;
      case 'plural': {
        const value = arg(part.name);
        if (value === undefined) {
          out += part.source;
        } else {
          const branch = select(part.branches, value);
          if (branch !== undefined) {
            out += render(branch, arg, pseudo, value);
          }
        }
        break;
      }
    }
  }
  return out;
}

/** Parsed messages by source; `null` marks one that does not parse. */
const parsed = new Map<string, Part[] | null>();

function parseCached(message: string): Part[] | null {
  let parts = parsed.get(message);
  if (parts === undefined) {
    try {
      parts = parse(message);
    } catch (error) {
      if (!(error instanceof MessageFormatError)) {
        throw error;
      }
      parts = null;
    }
    parsed.set(message, parts);
  }
  return parts;
}

function formatWith(message: string, arg: ArgLookup, locale: string): string {
  const parts = parseCached(message);
  if (parts === null) {
    return message;
  }
  const pseudo = locale === PSEUDO_LOCALE;
  const out = render(parts, arg, pseudo, null);
  return pseudo ? pad(out) : out;
}

/** An argument as text (`String(value)`); `undefined` and `null` count as missing. */
function argText(value: unknown): string | undefined {
  return value === undefined || value === null ? undefined : String(value);
}

/**
 * `message` with its placeholders and plurals filled from `args`, in `locale` (`en-XA`
 * pseudo-localizes). A missing argument keeps its placeholder as written; a message that does not
 * parse is returned as it is.
 */
export function formatMessage(
  message: string,
  args: Readonly<Record<string, unknown>> = {},
  locale = 'en',
): string {
  return formatWith(
    message,
    (name) => (Object.hasOwn(args, name) ? argText(args[name]) : undefined),
    locale,
  );
}

/**
 * The vue-i18n `messageCompiler` for this format: the message function reads its arguments from
 * `ctx.values`. A message that does not parse is shown as written (like the core); a precompiled
 * AST, which the shell never produces, shows its key.
 */
export const shellMessageCompiler: MessageCompiler = (message, context) => {
  if (typeof message !== 'string') {
    return () => context.key;
  }
  const locale = context.locale;
  return (ctx: MessageContext) =>
    formatWith(
      message,
      (name) => (Object.hasOwn(ctx.values, name) ? argText(ctx.values[name]) : undefined),
      locale,
    );
};
