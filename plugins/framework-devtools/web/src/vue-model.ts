// The Vue panel's model (RB-080-24), pure: the component tree the kit's `components` inspector
// answers, flattened to the rows the panel shows under the expanded nodes and a filter, and
// the selected component's state (`props`, `setup`, `data`, ...) as rows of key and formatted
// value. The kit's own `stringify` revives special values before this runs (`parse`).

import { isObject } from '../../../web-common/src/is-object';

/** A node of the kit's inspector tree (`CustomInspectorNode`). */
export interface TreeNode {
  id: string;
  label?: string;
  name?: string;
  children?: TreeNode[];
  tags?: { label: string }[];
}

/** One row of the tree as shown. */
export interface Row {
  id: string;
  label: string;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
  tags: string[];
}

/** One row of the state pane. */
export interface StateRow {
  section: string;
  key: string;
  value: string;
}

/** The longest value text shown. */
const MAX_VALUE = 200;

function labelOf(node: TreeNode): string {
  return node.label ?? node.name ?? node.id;
}

/** The rows of `nodes` with `expanded` nodes open, depth first. */
export function rows(nodes: TreeNode[], expanded: ReadonlySet<string>, depth = 0): Row[] {
  const out: Row[] = [];
  for (const node of nodes) {
    const children = node.children ?? [];
    const isOpen = expanded.has(node.id);
    out.push({
      id: node.id,
      label: labelOf(node),
      depth,
      hasChildren: children.length > 0,
      expanded: isOpen,
      tags: (node.tags ?? []).map((tag) => tag.label),
    });
    if (isOpen && children.length > 0) out.push(...rows(children, expanded, depth + 1));
  }
  return out;
}

/** Every node id of `nodes`, depth first. */
export function allIds(nodes: TreeNode[]): string[] {
  const ids: string[] = [];
  for (const node of nodes) {
    ids.push(node.id);
    if (node.children) ids.push(...allIds(node.children));
  }
  return ids;
}

/** The ids of every node of `nodes` with children, down to `depth` levels (the first open). */
export function idsToDepth(nodes: TreeNode[], depth: number): string[] {
  if (depth <= 0) return [];
  const ids: string[] = [];
  for (const node of nodes) {
    if (node.children && node.children.length > 0) {
      ids.push(node.id);
      ids.push(...idsToDepth(node.children, depth - 1));
    }
  }
  return ids;
}


function clip(text: string): string {
  return text.length > MAX_VALUE ? `${text.slice(0, MAX_VALUE - 1)}…` : text;
}

/** A value of the kit's inspector state as one line: custom values by their display text,
 *  strings quoted, collections summarised, the rest as JSON. */
export function formatValue(value: unknown): string {
  if (value === undefined) return 'undefined';
  if (value === null) return 'null';
  if (typeof value === 'string') return clip(JSON.stringify(value));
  if (typeof value === 'number' || typeof value === 'boolean' || typeof value === 'bigint') {
    return String(value);
  }
  if (typeof value === 'function') return 'function';
  if (Array.isArray(value)) return `Array(${value.length})`;
  if (isObject(value)) {
    const custom = value._custom;
    if (isObject(custom)) {
      if (typeof custom.displayText === 'string') return clip(custom.displayText);
      if (typeof custom.display === 'string') return clip(custom.display);
      if (custom.value !== undefined) return formatValue(custom.value);
      return typeof custom.type === 'string' ? custom.type : 'Object';
    }
    const keys = Object.keys(value);
    return keys.length === 0 ? '{}' : `Object {${clip(keys.join(', '))}}`;
  }
  return clip(String(value));
}

/** The rows of `state` (`{ props: [...], setup: [...], ... }`), sections in the kit's order. */
export function stateRows(state: unknown): StateRow[] {
  if (!isObject(state)) return [];
  const out: StateRow[] = [];
  for (const [section, entries] of Object.entries(state)) {
    if (!Array.isArray(entries)) continue;
    for (const entry of entries) {
      if (!isObject(entry) || typeof entry.key !== 'string') continue;
      out.push({ section, key: entry.key, value: formatValue(entry.value) });
    }
  }
  return out;
}

/** The ids of the nodes whose label matches `filter` (case-insensitive) and of their
 *  ancestors, so a filtered tree still shows the path; every id for an empty filter. */
export function matching(nodes: TreeNode[], filter: string): Set<string> {
  const needle = filter.trim().toLowerCase();
  const keep = new Set<string>();
  if (needle === '') {
    for (const id of allIds(nodes)) keep.add(id);
    return keep;
  }
  const visit = (node: TreeNode): boolean => {
    let hit = labelOf(node).toLowerCase().includes(needle);
    for (const child of node.children ?? []) if (visit(child)) hit = true;
    if (hit) keep.add(node.id);
    return hit;
  };
  for (const node of nodes) visit(node);
  return keep;
}

/** `nodes` without the subtrees `keep` leaves out. */
export function prune(nodes: TreeNode[], keep: ReadonlySet<string>): TreeNode[] {
  const out: TreeNode[] = [];
  for (const node of nodes) {
    if (!keep.has(node.id)) continue;
    const children = node.children ? prune(node.children, keep) : undefined;
    out.push(children === undefined ? { ...node } : { ...node, children });
  }
  return out;
}
