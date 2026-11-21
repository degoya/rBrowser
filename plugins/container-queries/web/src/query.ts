// The container size queries of CSS Containment 3: the prelude of an `@container` rule split into
// its name and condition, the condition parsed, evaluated against one container's size and read
// for its bounds (the thresholds the panel lists). Style queries (`style(...)`) and every other
// function are out of scope: they evaluate to unknown, and unknown never matches.
//
// The writing mode is taken as horizontal: `width` is the inline axis, `height` the block axis.

/** A comparison of the range syntax, as the feature compares to the value. */
export type Op = '<' | '<=' | '>' | '>=' | '=';
export type Axis = 'inline' | 'block';
type Feature = 'width' | 'height' | 'inline-size' | 'block-size' | 'aspect-ratio' | 'orientation';

interface Length {
  kind: 'length';
  number: number;
  unit: string;
  text: string;
}
interface Ratio {
  kind: 'ratio';
  number: number;
  text: string;
}
type Value = Length | Ratio | { kind: 'ident'; text: string };

/** A parsed condition. */
export type Node =
  | { kind: 'and' | 'or'; items: Node[] }
  | { kind: 'not'; item: Node }
  | { kind: 'cmp'; feature: Feature; op: Op; value: Value }
  | { kind: 'is'; feature: Feature }
  | { kind: 'unknown'; text: string };

/** What a container's size is measured against: its content box and the units' px. */
export interface Units {
  /** The container's font size, in px. */
  em: number;
  /** The root's font size, in px. */
  rem: number;
  /** The viewport, in px. */
  vw: number;
  vh: number;
}
export interface Sizes extends Units {
  inline: number;
  block: number;
}

/** One threshold of a condition. */
export interface Bound {
  axis: Axis;
  op: Op;
  /** The value as written (`50rem`). */
  value: string;
  /** In px for the units given; null for a unit this module does not know. */
  px: number | null;
}

const FEATURES: readonly Feature[] = ['width', 'height', 'inline-size', 'block-size', 'aspect-ratio', 'orientation'];
const FLIP: Record<Op, Op> = { '<': '>', '<=': '>=', '>': '<', '>=': '<=', '=': '=' };

type Token =
  | { type: 'open' | 'close' | 'colon' | 'slash' }
  | { type: 'op'; op: Op }
  | { type: 'ident'; text: string }
  | { type: 'number'; number: number; unit: string; text: string }
  | { type: 'func' | 'junk'; text: string };

const NUMBER = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/;
const IDENT = /^-?-?[a-zA-Z_][\w-]*/;

function tokenize(text: string): Token[] {
  const tokens: Token[] = [];
  let rest = text;
  while (rest.length > 0) {
    const space = /^\s+/.exec(rest);
    if (space) {
      rest = rest.slice(space[0].length);
      continue;
    }
    const two = rest.slice(0, 2);
    if (two === '<=' || two === '>=') {
      tokens.push({ type: 'op', op: two as Op });
      rest = rest.slice(2);
      continue;
    }
    const one = rest[0] as string;
    const punct = { '(': 'open', ')': 'close', ':': 'colon', '/': 'slash' } as const;
    if (one in punct) {
      tokens.push({ type: punct[one as keyof typeof punct] });
      rest = rest.slice(1);
      continue;
    }
    if (one === '<' || one === '>' || one === '=') {
      tokens.push({ type: 'op', op: one as Op });
      rest = rest.slice(1);
      continue;
    }
    const number = NUMBER.exec(rest);
    if (number) {
      const unit = /^[a-zA-Z%]*/.exec(rest.slice(number[0].length))?.[0] ?? '';
      const raw = number[0] + unit;
      tokens.push({ type: 'number', number: Number(number[0]), unit: unit.toLowerCase(), text: raw });
      rest = rest.slice(raw.length);
      continue;
    }
    const ident = IDENT.exec(rest);
    if (ident && rest[ident[0].length] === '(') {
      let depth = 0;
      let end = ident[0].length;
      for (; end < rest.length; end += 1) {
        if (rest[end] === '(') depth += 1;
        else if (rest[end] === ')' && --depth === 0) break;
      }
      tokens.push({ type: 'func', text: rest.slice(0, end + 1) });
      rest = rest.slice(end + 1);
      continue;
    }
    if (ident) {
      tokens.push({ type: 'ident', text: ident[0].toLowerCase() });
      rest = rest.slice(ident[0].length);
      continue;
    }
    tokens.push({ type: 'junk', text: one });
    rest = rest.slice(1);
  }
  return tokens;
}

/** The prelude `[name] condition` of an `@container` rule. */
export function parsePrelude(prelude: string): { name: string; query: string } {
  const text = prelude.trim();
  const ident = IDENT.exec(text);
  if (!ident || text[ident[0].length] === '(' || /^(not|and|or|none)$/i.test(ident[0])) {
    return { name: '', query: text };
  }
  return { name: ident[0], query: text.slice(ident[0].length).trim() };
}

const unknown = (text: string): Node => ({ kind: 'unknown', text });

function featureOf(name: string): Feature | null {
  return (FEATURES as readonly string[]).includes(name) ? (name as Feature) : null;
}

function valueOf(tokens: Token[]): Value | null {
  const [first, second, third] = tokens;
  if (tokens.length === 1 && first?.type === 'number') {
    return { kind: 'length', number: first.number, unit: first.unit, text: first.text };
  }
  if (tokens.length === 1 && first?.type === 'ident') return { kind: 'ident', text: first.text };
  if (tokens.length === 3 && first?.type === 'number' && second?.type === 'slash' && third?.type === 'number') {
    if (third.number === 0) return null;
    return { kind: 'ratio', number: first.number / third.number, text: `${first.text}/${third.text}` };
  }
  return null;
}

/** The inside of `( ... )` that is a feature: plain, boolean or range. */
function parseFeature(tokens: Token[], text: string): Node {
  const first = tokens[0];
  if (tokens.length === 1 && first?.type === 'ident') {
    const feature = featureOf(first.text);
    return feature ? { kind: 'is', feature } : unknown(text);
  }
  const colon = tokens.findIndex((token) => token.type === 'colon');
  if (colon === 1 && first?.type === 'ident') {
    const prefix = /^(min|max)-/.exec(first.text)?.[1];
    const feature = featureOf(prefix ? first.text.slice(prefix.length + 1) : first.text);
    const value = valueOf(tokens.slice(2));
    if (!feature || !value) return unknown(text);
    const op: Op = prefix === 'min' ? '>=' : prefix === 'max' ? '<=' : '=';
    if (prefix && feature === 'orientation') return unknown(text);
    return { kind: 'cmp', feature, op, value };
  }
  const ops = tokens.flatMap((token, index) => (token.type === 'op' ? [index] : []));
  const at = tokens.findIndex((token) => token.type === 'ident' && featureOf(token.text) !== null);
  const name = tokens[at];
  if (name?.type !== 'ident' || ops.length === 0 || ops.length > 2) return unknown(text);
  const feature = featureOf(name.text) as Feature;
  const opAt = (index: number): Op => {
    const token = tokens[index];
    return token?.type === 'op' ? token.op : '=';
  };
  /** The comparison the operator at `index` makes, its value before (flipped) or after it. */
  const side = (index: number, before: boolean): Node | null => {
    const value = before ? valueOf(tokens.slice(0, index)) : valueOf(tokens.slice(index + 1));
    return value ? { kind: 'cmp', feature, op: before ? FLIP[opAt(index)] : opAt(index), value } : null;
  };
  if (ops.length === 1) {
    const [op] = ops as [number];
    const node = op === at + 1 ? side(op, false) : op === at - 1 ? side(op, true) : null;
    return node ?? unknown(text);
  }
  const [left, right] = ops as [number, number];
  if (left !== at - 1 || right !== at + 1) return unknown(text);
  const low = side(left, true);
  const high = side(right, false);
  return low && high ? { kind: 'and', items: [low, high] } : unknown(text);
}

class Parser {
  private index = 0;
  constructor(
    private readonly tokens: Token[],
    private readonly text: string,
  ) {}

  condition(): Node {
    const first = this.tokens[this.index];
    if (first?.type === 'ident' && first.text === 'not') {
      this.index += 1;
      return { kind: 'not', item: this.inParens() };
    }
    const items = [this.inParens()];
    let joiner: 'and' | 'or' | null = null;
    for (let next = this.tokens[this.index]; next?.type === 'ident'; next = this.tokens[this.index]) {
      if (next.text !== 'and' && next.text !== 'or') break;
      if (joiner && joiner !== next.text) return unknown(this.text);
      joiner = next.text;
      this.index += 1;
      items.push(this.inParens());
    }
    return joiner ? { kind: joiner, items } : (items[0] as Node);
  }

  private inParens(): Node {
    const token = this.tokens[this.index];
    if (token?.type === 'func') {
      this.index += 1;
      return unknown(token.text);
    }
    if (token?.type !== 'open') {
      this.index = this.tokens.length;
      return unknown(this.text);
    }
    const next = this.tokens[this.index + 1];
    if (next?.type === 'open' || next?.type === 'func' || (next?.type === 'ident' && next.text === 'not')) {
      this.index += 1;
      const inner = this.condition();
      if (this.tokens[this.index]?.type !== 'close') return unknown(this.text);
      this.index += 1;
      return inner;
    }
    let depth = 0;
    let end = this.index;
    for (; end < this.tokens.length; end += 1) {
      const type = this.tokens[end]?.type;
      if (type === 'open') depth += 1;
      else if (type === 'close' && --depth === 0) break;
    }
    if (end >= this.tokens.length) {
      this.index = end;
      return unknown(this.text);
    }
    const inside = this.tokens.slice(this.index + 1, end);
    this.index = end + 1;
    return parseFeature(inside, this.text);
  }

  done(): boolean {
    return this.index >= this.tokens.length;
  }
}

/** The condition `text` (the prelude without its name). */
export function parseCondition(text: string): Node {
  const parser = new Parser(tokenize(text), text);
  const node = parser.condition();
  return parser.done() ? node : unknown(text);
}

/** `length` in px, or null for a unit not known here. */
export function toPx(length: Length, units: Units): number | null {
  const scale: Record<string, number> = {
    px: 1,
    em: units.em,
    rem: units.rem,
    vw: units.vw / 100,
    vh: units.vh / 100,
    vmin: Math.min(units.vw, units.vh) / 100,
    vmax: Math.max(units.vw, units.vh) / 100,
  };
  if (length.unit === '') return length.number === 0 ? 0 : null;
  const factor = scale[length.unit];
  return factor === undefined ? null : length.number * factor;
}

function compare(left: number, op: Op, right: number): boolean {
  switch (op) {
    case '<':
      return left < right;
    case '<=':
      return left <= right;
    case '>':
      return left > right;
    case '>=':
      return left >= right;
    default:
      return Math.abs(left - right) < 0.01;
  }
}

function axisOf(feature: Feature): Axis | null {
  if (feature === 'width' || feature === 'inline-size') return 'inline';
  if (feature === 'height' || feature === 'block-size') return 'block';
  return null;
}

/** Whether `node` holds for `sizes`: true, false, or null for unknown (which never matches). */
export function evaluate(node: Node, sizes: Sizes): boolean | null {
  switch (node.kind) {
    case 'and': {
      const results = node.items.map((item) => evaluate(item, sizes));
      return results.includes(false) ? false : results.includes(null) ? null : true;
    }
    case 'or': {
      const results = node.items.map((item) => evaluate(item, sizes));
      return results.includes(true) ? true : results.includes(null) ? null : false;
    }
    case 'not': {
      const result = evaluate(node.item, sizes);
      return result === null ? null : !result;
    }
    case 'is': {
      const axis = axisOf(node.feature);
      return axis ? sizes[axis] !== 0 : node.feature === 'aspect-ratio' ? sizes.block !== 0 : true;
    }
    case 'cmp': {
      const axis = axisOf(node.feature);
      if (axis && node.value.kind === 'length') {
        const px = toPx(node.value, sizes);
        return px === null ? null : compare(sizes[axis], node.op, px);
      }
      if (node.feature === 'aspect-ratio' && node.value.kind !== 'ident' && sizes.block > 0) {
        return compare(sizes.inline / sizes.block, node.op, node.value.number);
      }
      if (node.feature === 'orientation' && node.value.kind === 'ident') {
        const portrait = sizes.block >= sizes.inline;
        if (node.value.text === 'portrait') return portrait;
        if (node.value.text === 'landscape') return !portrait;
      }
      return null;
    }
    default:
      return null;
  }
}

/** The size thresholds of `node`, in the order written. */
export function boundsOf(node: Node, units: Units): Bound[] {
  switch (node.kind) {
    case 'and':
    case 'or':
      return node.items.flatMap((item) => boundsOf(item, units));
    case 'not':
      return boundsOf(node.item, units);
    case 'cmp': {
      const axis = axisOf(node.feature);
      if (!axis || node.value.kind !== 'length') return [];
      return [{ axis, op: node.op, value: node.value.text, px: toPx(node.value, units) }];
    }
    default:
      return [];
  }
}

/** The axes `node` reads: a container of `container-type: inline-size` answers the inline one only. */
export function axesOf(node: Node): Set<Axis> {
  const axes = new Set<Axis>();
  const visit = (item: Node): void => {
    if (item.kind === 'and' || item.kind === 'or') item.items.forEach(visit);
    else if (item.kind === 'not') visit(item.item);
    else if (item.kind === 'cmp' || item.kind === 'is') {
      const axis = axisOf(item.feature);
      if (axis) axes.add(axis);
      else {
        axes.add('inline');
        axes.add('block');
      }
    }
  };
  visit(node);
  return axes;
}
