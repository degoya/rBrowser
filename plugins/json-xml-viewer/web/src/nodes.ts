// The tree's nodes: one interface over a parsed JSON value (here) and an XML document's DOM
// (xml-nodes.ts). Children are made when first asked for and kept, so a large document costs
// only what is opened, searched or queried.

export type Kind =
  | 'object'
  | 'array'
  | 'string'
  | 'number'
  | 'boolean'
  | 'null'
  | 'element'
  | 'attribute'
  | 'text'
  | 'comment'
  | 'cdata'
  | 'instruction';

export interface TreeNode {
  readonly kind: Kind;
  readonly parent: TreeNode | null;
  /** 0 for a root. */
  readonly depth: number;
  /** The key, index, tag or attribute name; null for a JSON root. */
  readonly name: string | null;
  hasChildren(): boolean;
  children(): readonly TreeNode[];
  /** How many children, without making them. */
  childCount(): number;
  /** What "Copy value" copies: a scalar as text, a container as JSON or XML. */
  value(): string;
  /** A short text of a scalar for the row; empty for a container. */
  preview(): string;
  /** JSONPath (`$.items[3].id`) or XPath (`/urlset/url[2]/loc`). */
  path(): string;
  /** The text a search matches: the name and a scalar's value. */
  searchText(): string;
}

const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

/** One step of a JSONPath: `.key`, `['odd key']` or `[3]`. */
export function jsonPathStep(key: string | number): string {
  if (typeof key === 'number') return `[${key}]`;
  if (IDENTIFIER.test(key)) return `.${key}`;
  return `['${key.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}']`;
}

function kindOfValue(value: unknown): Kind {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  switch (typeof value) {
    case 'object':
      return 'object';
    case 'string':
      return 'string';
    case 'number':
      return 'number';
    default:
      return 'boolean';
  }
}

/** The longest preview of a string row, in characters; the value is copied whole. */
export const PREVIEW_CHARS = 500;

export class JsonNode implements TreeNode {
  readonly kind: Kind;
  readonly depth: number;
  private made: JsonNode[] | null = null;

  constructor(
    readonly json: unknown,
    readonly key: string | number | null = null,
    readonly parent: JsonNode | null = null,
  ) {
    this.kind = kindOfValue(json);
    this.depth = parent ? parent.depth + 1 : 0;
  }

  get name(): string | null {
    return this.key === null ? null : String(this.key);
  }

  hasChildren(): boolean {
    return this.childCount() > 0;
  }

  childCount(): number {
    if (this.kind === 'array') return (this.json as unknown[]).length;
    if (this.kind === 'object') return Object.keys(this.json as object).length;
    return 0;
  }

  children(): readonly JsonNode[] {
    if (this.made) return this.made;
    if (this.kind === 'array') {
      this.made = (this.json as unknown[]).map((item, index) => new JsonNode(item, index, this));
    } else if (this.kind === 'object') {
      this.made = Object.entries(this.json as object).map(
        ([key, item]) => new JsonNode(item as unknown, key, this),
      );
    } else {
      this.made = [];
    }
    return this.made;
  }

  value(): string {
    if (this.kind === 'string') return this.json as string;
    if (this.kind === 'object' || this.kind === 'array') return JSON.stringify(this.json, null, 2);
    return String(this.json);
  }

  preview(): string {
    switch (this.kind) {
      case 'string': {
        const text = JSON.stringify(this.json as string);
        return text.length > PREVIEW_CHARS ? `${text.slice(0, PREVIEW_CHARS)}…` : text;
      }
      case 'object':
      case 'array':
        return '';
      default:
        return String(this.json);
    }
  }

  path(): string {
    const steps: string[] = [];
    for (let node: JsonNode | null = this; node && node.key !== null; node = node.parent) {
      steps.push(jsonPathStep(node.key));
    }
    return `$${steps.reverse().join('')}`;
  }

  searchText(): string {
    const name = this.key === null || typeof this.key === 'number' ? '' : this.key;
    if (this.kind === 'object' || this.kind === 'array') return name;
    return `${name}\u0000${String(this.json)}`;
  }
}
