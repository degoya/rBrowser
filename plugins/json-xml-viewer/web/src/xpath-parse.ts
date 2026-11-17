// The parser of the viewer's XPath 1.0 subset: location paths with `/`, `//`, `.`, `..`, `@`,
// name tests (`name`, `p:name`, `*`, `p:*`) and node tests (`text()`, `node()`, `comment()`,
// `processing-instruction()`), predicates with `and`, `or`, `=`, `!=`, `<`, `<=`, `>`, `>=`,
// `+`, `-`, strings, numbers and the functions of `FUNCTIONS`, and unions with `|`. Explicit
// axes (`child::`) and variables are not part of it. Evaluation is xpath.ts.

export type Axis = 'child' | 'attribute' | 'self' | 'parent' | 'descendant-or-self';

export type NodeTest =
  | { kind: 'name'; prefix: string | null; local: string }
  | { kind: 'text' | 'node' | 'comment' | 'instruction' };

export interface Step {
  axis: Axis;
  test: NodeTest;
  predicates: Expr[];
}

export interface PathExpr {
  type: 'path';
  absolute: boolean;
  steps: Step[];
}

export type Operator = '=' | '!=' | '<' | '<=' | '>' | '>=' | '+' | '-' | 'and' | 'or';

export type Expr =
  | PathExpr
  | { type: 'union'; paths: Expr[] }
  | { type: 'literal'; value: string }
  | { type: 'number'; value: number }
  | { type: 'binary'; op: Operator; left: Expr; right: Expr }
  | { type: 'call'; name: string; args: Expr[] };

/** The functions a predicate may call, with their number of arguments (min, max). */
export const FUNCTIONS: Record<string, readonly [number, number]> = {
  last: [0, 0],
  position: [0, 0],
  not: [1, 1],
  contains: [2, 2],
  'starts-with': [2, 2],
  count: [1, 1],
  string: [0, 1],
  'normalize-space': [0, 1],
  'local-name': [0, 1],
  name: [0, 1],
};

type NodeKind = Exclude<NodeTest['kind'], 'name'>;

const NODE_TESTS: Record<string, NodeKind> = {
  text: 'text',
  node: 'node',
  comment: 'comment',
  'processing-instruction': 'instruction',
};

const NAME = /^[A-Za-z_\u00C0-\uFFFF][\w.\-\u00B7\u00C0-\uFFFF]*/;
const NUMBER = /^(\d+(\.\d*)?|\.\d+)/;
const ANY_NODE: NodeTest = { kind: 'node' };

export class XPathError {
  constructor(readonly offset: number) {}
}

class Parser {
  i = 0;
  constructor(readonly text: string) {}

  space(): void {
    while (/\s/.test(this.text[this.i] ?? '')) this.i += 1;
  }

  at(token: string): boolean {
    this.space();
    return this.text.startsWith(token, this.i);
  }

  take(token: string): boolean {
    if (!this.at(token)) return false;
    this.i += token.length;
    return true;
  }

  expect(token: string): void {
    if (!this.take(token)) throw new XPathError(this.i);
  }

  name(): string | null {
    this.space();
    const match = NAME.exec(this.text.slice(this.i));
    return match ? match[0] : null;
  }

  union(): Expr {
    const first = this.path();
    if (!this.at('|')) return first;
    const paths = [first];
    while (this.take('|')) paths.push(this.path());
    return { type: 'union', paths };
  }

  path(): PathExpr {
    this.space();
    const steps: Step[] = [];
    let absolute = false;
    if (this.take('//')) {
      absolute = true;
      steps.push({ axis: 'descendant-or-self', test: ANY_NODE, predicates: [] });
    } else if (this.take('/')) {
      absolute = true;
      if (!this.startsStep()) return { type: 'path', absolute, steps };
    }
    steps.push(this.step());
    for (;;) {
      if (this.take('//')) {
        steps.push({ axis: 'descendant-or-self', test: ANY_NODE, predicates: [] });
      } else if (!this.take('/')) {
        return { type: 'path', absolute, steps };
      }
      steps.push(this.step());
    }
  }

  startsStep(): boolean {
    this.space();
    const c = this.text[this.i];
    return c === '.' || c === '@' || c === '*' || this.name() !== null;
  }

  step(): Step {
    if (this.take('..')) return { axis: 'parent', test: ANY_NODE, predicates: [] };
    if (this.take('.')) return { axis: 'self', test: ANY_NODE, predicates: [] };
    const axis: Axis = this.take('@') ? 'attribute' : 'child';
    const test = this.nodeTest();
    const predicates: Expr[] = [];
    while (this.take('[')) {
      predicates.push(this.or());
      this.expect(']');
    }
    return { axis, test, predicates };
  }

  nodeTest(): NodeTest {
    if (this.take('*')) return { kind: 'name', prefix: null, local: '*' };
    const name = this.name();
    if (name === null) throw new XPathError(this.i);
    this.i += name.length;
    const kind = NODE_TESTS[name];
    if (kind && this.take('(')) {
      this.expect(')');
      return { kind };
    }
    if (this.text[this.i] === ':' && this.text[this.i + 1] !== ':') {
      this.i += 1;
      if (this.text[this.i] === '*') {
        this.i += 1;
        return { kind: 'name', prefix: name, local: '*' };
      }
      const local = NAME.exec(this.text.slice(this.i))?.[0];
      if (!local) throw new XPathError(this.i);
      this.i += local.length;
      return { kind: 'name', prefix: name, local };
    }
    return { kind: 'name', prefix: null, local: name };
  }

  binary(next: () => Expr, ops: Operator[]): Expr {
    let left = next();
    for (;;) {
      const op = ops.find((candidate) => this.at(candidate) && this.isOperator(candidate));
      if (!op) return left;
      this.i += op.length;
      left = { type: 'binary', op, left, right: next() };
    }
  }

  /** `and` and `or` are operators only as whole words. */
  isOperator(op: Operator): boolean {
    if (op !== 'and' && op !== 'or') return true;
    return !/[\w.-]/.test(this.text[this.i + op.length] ?? '');
  }

  or(): Expr {
    return this.binary(() => this.and(), ['or']);
  }

  and(): Expr {
    return this.binary(() => this.compare(), ['and']);
  }

  compare(): Expr {
    return this.binary(() => this.additive(), ['!=', '<=', '>=', '=', '<', '>']);
  }

  additive(): Expr {
    return this.binary(() => this.primary(), ['+', '-']);
  }

  primary(): Expr {
    this.space();
    const c = this.text[this.i];
    if (c === "'" || c === '"') {
      const end = this.text.indexOf(c, this.i + 1);
      if (end === -1) throw new XPathError(this.text.length);
      const value = this.text.slice(this.i + 1, end);
      this.i = end + 1;
      return { type: 'literal', value };
    }
    const number = NUMBER.exec(this.text.slice(this.i));
    if (number) {
      this.i += number[0].length;
      return { type: 'number', value: Number(number[0]) };
    }
    if (c === '(') {
      this.i += 1;
      const inner = this.or();
      this.expect(')');
      return inner;
    }
    const name = this.name();
    if (name !== null && !NODE_TESTS[name] && name in FUNCTIONS) {
      const start = this.i;
      this.i += name.length;
      if (this.take('(')) return this.call(name, start);
      this.i = start;
    }
    if (!this.startsStep() && c !== '/') throw new XPathError(this.i);
    return this.path();
  }

  call(name: string, start: number): Expr {
    const args: Expr[] = [];
    if (!this.take(')')) {
      args.push(this.or());
      while (this.take(',')) args.push(this.or());
      this.expect(')');
    }
    const [min, max] = FUNCTIONS[name] as readonly [number, number];
    if (args.length < min || args.length > max) throw new XPathError(start);
    return { type: 'call', name, args };
  }
}

/** The query as an expression; throws `XPathError` with the offset it cannot read. */
export function parseXPath(text: string): Expr {
  const parser = new Parser(text);
  const expr = parser.union();
  parser.space();
  if (parser.i < text.length) throw new XPathError(parser.i);
  return expr;
}
