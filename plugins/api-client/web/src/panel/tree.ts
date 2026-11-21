// The answer's JSON as tree rows (RB-096-01): one row per value, a container open when its path
// is in `open`; a search shows the rows whose key or value holds it and the containers on their
// way, opened. At most `limit` rows, so a large answer stays fluid; `cut` says some were left out.

export type Kind = 'object' | 'array' | 'string' | 'number' | 'boolean' | 'null';

export interface Row {
  path: string;
  depth: number;
  label: string;
  kind: Kind;
  /** The value's text, or the container's size. */
  text: string;
  size: number;
  expandable: boolean;
  open: boolean;
  match: boolean;
}

export interface Rows {
  rows: Row[];
  cut: boolean;
}

function kindOf(value: unknown): Kind {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  if (typeof value === 'object') return 'object';
  if (typeof value === 'string') return 'string';
  if (typeof value === 'number') return 'number';
  if (typeof value === 'boolean') return 'boolean';
  return 'null';
}

function entries(value: unknown): [string, unknown][] {
  if (Array.isArray(value)) return value.map((item, index) => [String(index), item]);
  if (value !== null && typeof value === 'object') return Object.entries(value as Record<string, unknown>);
  return [];
}

/** The path of child `key` of `parent`. */
export function childPath(parent: string, key: string, array: boolean): string {
  if (array) return `${parent}[${key}]`;
  return /^[A-Za-z_$][\w$]*$/.test(key) ? `${parent}.${key}` : `${parent}['${key.replaceAll("'", "\\'")}']`;
}

function textOf(value: unknown, kind: Kind): string {
  if (kind === 'string') return JSON.stringify(value);
  if (kind === 'object' || kind === 'array') return '';
  return String(value);
}

function holds(value: unknown, needle: string): boolean {
  const kind = kindOf(value);
  if (kind === 'object' || kind === 'array') {
    return entries(value).some(([key, child]) => key.toLowerCase().includes(needle) || holds(child, needle));
  }
  return textOf(value, kind).toLowerCase().includes(needle);
}

/** The rows of `value` under `label` at `path`. */
export function rows(value: unknown, open: ReadonlySet<string>, search = '', limit = 2000, label = '$', path = '$'): Rows {
  const needle = search.trim().toLowerCase();
  const out: Row[] = [];
  let cut = false;
  const walk = (item: unknown, name: string, at: string, depth: number): void => {
    if (out.length >= limit) {
      cut = true;
      return;
    }
    const kind = kindOf(item);
    const children = entries(item);
    const expandable = children.length > 0;
    const selfMatch = needle !== '' && (name.toLowerCase().includes(needle) || textOf(item, kind).toLowerCase().includes(needle));
    const below = needle !== '' && expandable && children.some(([key, child]) => key.toLowerCase().includes(needle) || holds(child, needle));
    if (needle !== '' && !selfMatch && !below && depth > 0) return;
    const isOpen = expandable && (needle !== '' ? below : open.has(at) || depth === 0);
    out.push({ path: at, depth, label: name, kind, text: textOf(item, kind), size: children.length, expandable, open: isOpen, match: selfMatch });
    if (!isOpen) return;
    for (const [key, child] of children) walk(child, key, childPath(at, key, kind === 'array'), depth + 1);
  };
  walk(value, label, path, 0);
  return { rows: out, cut };
}
