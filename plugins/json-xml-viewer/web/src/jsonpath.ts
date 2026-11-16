// A JSONPath subset (RFC 9535 without filters and functions) over the tree's JSON nodes:
// `$`, `.name`, `.*`, `..name`, `..*`, `..[...]`, and brackets with quoted names, indexes
// (negative from the end), `*`, slices `start:end:step` and unions of these. Filter expressions
// (`?(...)`) are refused: they would need script evaluation, which the viewer never does.

import type { JsonNode } from './nodes';

type Selector =
  | { type: 'name'; name: string }
  | { type: 'wildcard' }
  | { type: 'index'; index: number }
  | { type: 'slice'; start: number | null; end: number | null; step: number | null };

interface Segment {
  descendant: boolean;
  selectors: Selector[];
}

export type ParsedPath = { ok: true; segments: Segment[] } | { ok: false; offset: number };

export type PathAnswer =
  | { ok: true; nodes: JsonNode[]; truncated: boolean }
  | { ok: false; offset: number };

/** The most nodes a query answers. */
export const MAX_RESULTS = 10000;

const NAME_CHAR = /[A-Za-z0-9_$\-\u0080-\uFFFF]/;
const INTEGER = /^-?(0|[1-9][0-9]*)/;

class Fail {
  constructor(readonly offset: number) {}
}

class Reader {
  i = 0;
  constructor(readonly text: string) {}

  peek(): string | undefined {
    return this.text[this.i];
  }

  skipSpace(): void {
    while (this.peek() === ' ') this.i += 1;
  }

  expect(char: string): void {
    if (this.peek() !== char) throw new Fail(this.i);
    this.i += 1;
  }

  name(): string {
    const start = this.i;
    while (this.i < this.text.length && NAME_CHAR.test(this.text[this.i] as string)) this.i += 1;
    if (this.i === start) throw new Fail(start);
    return this.text.slice(start, this.i);
  }

  integer(): number | null {
    const match = INTEGER.exec(this.text.slice(this.i));
    if (!match) return null;
    this.i += match[0].length;
    return Number(match[0]);
  }

  quoted(): string {
    const quote = this.peek() as string;
    this.i += 1;
    let out = '';
    for (;;) {
      const c = this.peek();
      if (c === undefined) throw new Fail(this.i);
      this.i += 1;
      if (c === quote) return out;
      if (c === '\\') {
        const next = this.peek();
        if (next === undefined) throw new Fail(this.i);
        this.i += 1;
        out += next === 'n' ? '\n' : next === 't' ? '\t' : next;
      } else {
        out += c;
      }
    }
  }

  selector(): Selector {
    this.skipSpace();
    const c = this.peek();
    if (c === "'" || c === '"') return { type: 'name', name: this.quoted() };
    if (c === '*') {
      this.i += 1;
      return { type: 'wildcard' };
    }
    const start = this.integer();
    this.skipSpace();
    if (this.peek() !== ':') {
      if (start === null) throw new Fail(this.i);
      return { type: 'index', index: start };
    }
    this.i += 1;
    this.skipSpace();
    const end = this.integer();
    this.skipSpace();
    let step: number | null = null;
    if (this.peek() === ':') {
      this.i += 1;
      this.skipSpace();
      step = this.integer();
    }
    if (step === 0) throw new Fail(this.i - 1);
    return { type: 'slice', start, end, step };
  }

  bracket(): Selector[] {
    this.expect('[');
    const selectors = [this.selector()];
    for (;;) {
      this.skipSpace();
      if (this.peek() === ',') {
        this.i += 1;
        selectors.push(this.selector());
        continue;
      }
      this.expect(']');
      return selectors;
    }
  }
}

/** The query's segments, or the offset of the first character it cannot read. */
export function parseJsonPath(text: string): ParsedPath {
  const reader = new Reader(text.trimEnd());
  try {
    reader.skipSpace();
    reader.expect('$');
    const segments: Segment[] = [];
    while (reader.i < reader.text.length) {
      let descendant = false;
      if (reader.text.startsWith('..', reader.i)) {
        descendant = true;
        reader.i += 2;
      } else if (reader.peek() === '.') {
        reader.i += 1;
      } else if (reader.peek() !== '[') {
        throw new Fail(reader.i);
      }
      if (reader.peek() === '[') {
        segments.push({ descendant, selectors: reader.bracket() });
      } else if (reader.peek() === '*') {
        reader.i += 1;
        segments.push({ descendant, selectors: [{ type: 'wildcard' }] });
      } else {
        segments.push({ descendant, selectors: [{ type: 'name', name: reader.name() }] });
      }
    }
    return { ok: true, segments };
  } catch (error) {
    if (error instanceof Fail) return { ok: false, offset: error.offset };
    throw error;
  }
}

function sliceIndexes(length: number, selector: Extract<Selector, { type: 'slice' }>): number[] {
  const step = selector.step ?? 1;
  const norm = (value: number) => (value < 0 ? Math.max(length + value, -1) : Math.min(value, length));
  const out: number[] = [];
  if (step > 0) {
    const start = Math.max(norm(selector.start ?? 0), 0);
    const end = norm(selector.end ?? length);
    for (let i = start; i < end; i += step) out.push(i);
  } else {
    const start = Math.min(norm(selector.start ?? length - 1), length - 1);
    const end = selector.end === null ? -1 : norm(selector.end);
    for (let i = start; i > end; i += step) out.push(i);
  }
  return out;
}

function select(node: JsonNode, selector: Selector): JsonNode[] {
  const kids = node.children() as JsonNode[];
  switch (selector.type) {
    case 'wildcard':
      return kids.slice();
    case 'name':
      return node.kind === 'object' ? kids.filter((kid) => kid.key === selector.name) : [];
    case 'index': {
      if (node.kind !== 'array') return [];
      const kid = kids[selector.index < 0 ? kids.length + selector.index : selector.index];
      return kid ? [kid] : [];
    }
    case 'slice':
      return node.kind === 'array'
        ? sliceIndexes(kids.length, selector).flatMap((i) => (kids[i] ? [kids[i]] : []))
        : [];
  }
}

/** `node` and every node below it, in document order, without recursion. */
function* descendants(node: JsonNode): Generator<JsonNode> {
  const stack: JsonNode[] = [node];
  while (stack.length > 0) {
    const next = stack.pop() as JsonNode;
    yield next;
    const kids = next.children() as JsonNode[];
    for (let i = kids.length - 1; i >= 0; i -= 1) stack.push(kids[i] as JsonNode);
  }
}

/** The nodes `query` selects below `root`, at most `limit`, each once. */
export function evaluateJsonPath(root: JsonNode, query: string, limit = MAX_RESULTS): PathAnswer {
  const parsed = parseJsonPath(query);
  if (!parsed.ok) return parsed;
  let nodes: JsonNode[] = [root];
  let truncated = false;
  for (const segment of parsed.segments) {
    const seen = new Set<JsonNode>();
    const next: JsonNode[] = [];
    outer: for (const node of nodes) {
      for (const candidate of segment.descendant ? descendants(node) : [node]) {
        for (const selector of segment.selectors) {
          for (const found of select(candidate, selector)) {
            if (seen.has(found)) continue;
            if (next.length >= limit) {
              truncated = true;
              break outer;
            }
            seen.add(found);
            next.push(found);
          }
        }
      }
    }
    nodes = next;
  }
  return { ok: true, nodes, truncated };
}
