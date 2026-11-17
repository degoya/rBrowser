// Evaluation of the viewer's XPath subset (xpath-parse.ts) over the tree's XML nodes. Its own
// evaluator rather than `document.evaluate`: an unprefixed name matches an element of any
// namespace by its local name, so `//url/loc` finds the URLs of a sitemap whose default
// namespace is the sitemap schema's - with the browser's XPath that needs a prefix and a
// namespace resolver. A prefixed name (`x:alt`) matches the qualified name as written.

import { type Expr, type NodeTest, type PathExpr, type Step, XPathError, parseXPath } from './xpath-parse';
import type { XmlNode } from './xml-nodes';

/** The document above the top-level nodes: the context of an absolute path. */
const DOC: unique symbol = Symbol('document');
type XNode = XmlNode | typeof DOC;
type Value = XNode[] | string | number | boolean;

interface Context {
  node: XNode;
  position: number;
  size: number;
}

export type XPathAnswer =
  | { ok: true; nodes: XmlNode[]; truncated: boolean }
  | { ok: false; offset: number };

/** The most nodes a query answers. */
export const MAX_RESULTS = 10000;

class Evaluator {
  constructor(private readonly roots: readonly XmlNode[]) {}

  children(node: XNode): readonly XmlNode[] {
    return node === DOC ? this.roots : node.all();
  }

  /** `node` and every node below it (not attributes), in document order, without recursion. */
  descendantsOrSelf(node: XNode): XNode[] {
    const out: XNode[] = [];
    const stack: XNode[] = [node];
    while (stack.length > 0) {
      const next = stack.pop() as XNode;
      out.push(next);
      const kids = this.children(next);
      for (let i = kids.length - 1; i >= 0; i -= 1) stack.push(kids[i] as XmlNode);
    }
    return out;
  }

  axis(node: XNode, step: Step): XNode[] {
    switch (step.axis) {
      case 'child':
        return [...this.children(node)];
      case 'attribute':
        return node === DOC ? [] : [...node.attrs()];
      case 'self':
        return [node];
      case 'parent':
        return node === DOC ? [] : [node.parent ?? DOC];
      case 'descendant-or-self':
        return this.descendantsOrSelf(node);
    }
  }

  matches(node: XNode, test: NodeTest, attribute: boolean): boolean {
    if (test.kind === 'node') return true;
    if (node === DOC) return false;
    switch (test.kind) {
      case 'text':
        return node.kind === 'text' || node.kind === 'cdata';
      case 'comment':
        return node.kind === 'comment';
      case 'instruction':
        return node.kind === 'instruction';
      case 'name': {
        if (node.kind !== (attribute ? 'attribute' : 'element')) return false;
        const name = node.name ?? '';
        const prefix = name.includes(':') ? name.slice(0, name.indexOf(':')) : null;
        if (test.local === '*') return test.prefix === null || test.prefix === prefix;
        if (test.prefix === null) return node.localName === test.local;
        return name === `${test.prefix}:${test.local}`;
      }
    }
  }

  path(expr: PathExpr, context: XNode, limit = Number.POSITIVE_INFINITY): { nodes: XNode[]; truncated: boolean } {
    let nodes: XNode[] = [expr.absolute ? DOC : context];
    let truncated = false;
    expr.steps.forEach((step, index) => {
      const last = index === expr.steps.length - 1;
      const seen = new Set<XNode>();
      const next: XNode[] = [];
      for (const node of nodes) {
        let found = this.axis(node, step).filter((candidate) =>
          this.matches(candidate, step.test, step.axis === 'attribute'),
        );
        for (const predicate of step.predicates) {
          const size = found.length;
          found = found.filter((candidate, i) => {
            const value = this.value(predicate, { node: candidate, position: i + 1, size });
            return typeof value === 'number' ? value === i + 1 : boolean(value);
          });
        }
        for (const candidate of found) {
          if (seen.has(candidate)) continue;
          if (last && next.length >= limit) {
            truncated = true;
            break;
          }
          seen.add(candidate);
          next.push(candidate);
        }
      }
      nodes = next;
    });
    return { nodes, truncated };
  }

  value(expr: Expr, context: Context): Value {
    switch (expr.type) {
      case 'literal':
      case 'number':
        return expr.value;
      case 'path':
        return this.path(expr, context.node).nodes;
      case 'union': {
        const all = new Set<XNode>();
        for (const path of expr.paths) {
          const value = this.value(path, context);
          if (Array.isArray(value)) value.forEach((node) => all.add(node));
        }
        return [...all];
      }
      case 'call':
        return this.call(expr.name, expr.args, context);
      case 'binary': {
        const left = this.value(expr.left, context);
        if (expr.op === 'and') return boolean(left) && boolean(this.value(expr.right, context));
        if (expr.op === 'or') return boolean(left) || boolean(this.value(expr.right, context));
        const right = this.value(expr.right, context);
        if (expr.op === '+') return number(left) + number(right);
        if (expr.op === '-') return number(left) - number(right);
        return compare(expr.op, left, right);
      }
    }
  }

  call(name: string, args: Expr[], context: Context): Value {
    const arg = (index: number): Value => this.value(args[index] as Expr, context);
    const nodesOf = (index: number): XNode[] => {
      const value: Value = args[index] ? arg(index) : [context.node];
      return Array.isArray(value) ? value : [];
    };
    switch (name) {
      case 'last':
        return context.size;
      case 'position':
        return context.position;
      case 'not':
        return !boolean(arg(0));
      case 'contains':
        return string(arg(0)).includes(string(arg(1)));
      case 'starts-with':
        return string(arg(0)).startsWith(string(arg(1)));
      case 'count':
        return nodesOf(0).length;
      case 'string':
        return string(args[0] ? arg(0) : [context.node]);
      case 'normalize-space':
        return string(args[0] ? arg(0) : [context.node]).trim().replace(/\s+/g, ' ');
      case 'local-name':
      case 'name': {
        const node = nodesOf(0)[0];
        if (node === undefined || node === DOC) return '';
        return name === 'name' ? (node.name ?? '') : node.localName;
      }
      default:
        return false;
    }
  }
}

function stringOf(node: XNode): string {
  return node === DOC ? '' : node.value();
}

function string(value: Value): string {
  if (Array.isArray(value)) return value[0] === undefined ? '' : stringOf(value[0]);
  return String(value);
}

function number(value: Value): number {
  if (typeof value === 'number') return value;
  if (typeof value === 'boolean') return value ? 1 : 0;
  const text = string(value).trim();
  return text === '' ? Number.NaN : Number(text);
}

function boolean(value: Value): boolean {
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'number') return value !== 0 && !Number.isNaN(value);
  if (typeof value === 'string') return value !== '';
  return value;
}

function compareAtoms(op: string, left: string | number | boolean, right: string | number | boolean): boolean {
  if (op === '=' || op === '!=') {
    let equal: boolean;
    if (typeof left === 'boolean' || typeof right === 'boolean') equal = boolean(left) === boolean(right);
    else if (typeof left === 'number' || typeof right === 'number') equal = number(left) === number(right);
    else equal = left === right;
    return op === '=' ? equal : !equal;
  }
  const [a, b] = [number(left), number(right)];
  return op === '<' ? a < b : op === '<=' ? a <= b : op === '>' ? a > b : a >= b;
}

/** XPath 1.0 comparison: a node-set compares true when any of its nodes does. */
function compare(op: string, left: Value, right: Value): boolean {
  const atoms = (value: Value): (string | number | boolean)[] =>
    Array.isArray(value) ? value.map(stringOf) : [value];
  return atoms(left).some((a) => atoms(right).some((b) => compareAtoms(op, a, b)));
}

/** The nodes `query` selects among the document's top-level `roots`, at most `limit`. */
export function evaluateXPath(roots: readonly XmlNode[], query: string, limit = MAX_RESULTS): XPathAnswer {
  let expr: Expr;
  try {
    expr = parseXPath(query);
  } catch (error) {
    if (error instanceof XPathError) return { ok: false, offset: error.offset };
    throw error;
  }
  const evaluator = new Evaluator(roots);
  const paths = expr.type === 'union' ? expr.paths : [expr];
  const nodes: XmlNode[] = [];
  const seen = new Set<XNode>();
  let truncated = false;
  for (const path of paths) {
    if (path.type !== 'path') return { ok: false, offset: 0 };
    const answer = evaluator.path(path, DOC, limit - nodes.length);
    truncated ||= answer.truncated;
    for (const node of answer.nodes) {
      if (node === DOC || seen.has(node)) continue;
      seen.add(node);
      nodes.push(node);
    }
  }
  return { ok: true, nodes, truncated };
}
