// The page tree of the sitemap start page (RB-099-09) without Vue: the core's tree flattened into
// nodes by id, a selection over the pages by index, a search over the URLs, the rows a tree shows
// and the window of rows on screen. The core puts the pages in the tree's order, so every node's
// pages are one range (`first`, `count`): choosing a subtree fills a range, and a node's state is
// a difference of two prefix sums - 10,000 pages stay fluid.

import type { JsonValue, SitemapDiscovery, SitemapTreeNode } from '../../ipc/protocol';

/** One node of the flattened tree. */
export interface FlatNode {
  id: number;
  name: string;
  /** 1 for a site. */
  level: number;
  /** The parent's id; -1 for a site. */
  parent: number;
  first: number;
  count: number;
  children: number[];
}

export interface PageTree {
  pages: string[];
  nodes: FlatNode[];
  roots: number[];
}

/** The core's tree as nodes by id (depth first, parents before their children). */
export function flatten(discovery: Pick<SitemapDiscovery, 'pages' | 'tree'>): PageTree {
  const nodes: FlatNode[] = [];
  const add = (node: SitemapTreeNode, level: number, parent: number): number => {
    const id = nodes.length;
    const flat: FlatNode = { id, name: node.name, level, parent, first: node.first, count: node.count, children: [] };
    nodes.push(flat);
    for (const child of node.children ?? []) {
      flat.children.push(add(child, level + 1, id));
    }
    return id;
  };
  const roots = discovery.tree.map((node) => add(node, 1, -1));
  return { pages: discovery.pages, nodes, roots };
}

/** Whether `node` is one page. */
export function isPage(node: FlatNode): boolean {
  return node.children.length === 0 && node.count === 1;
}

/** The pages a search keeps: `null` for none (every page), else a flag per page. */
export type Scope = Uint8Array | null;

/** The pages whose URL holds `query` (any case); `null` for an empty query. */
export function search(pages: readonly string[], query: string): Scope {
  const wanted = query.trim().toLowerCase();
  if (wanted === '') return null;
  const scope = new Uint8Array(pages.length);
  pages.forEach((page, index) => {
    scope[index] = page.toLowerCase().includes(wanted) ? 1 : 0;
  });
  return scope;
}

/** Prefix sums: `sums[i]` is how many of the first `i` entries `at` counts. */
function prefix(length: number, at: (index: number) => boolean): Int32Array {
  const sums = new Int32Array(length + 1);
  for (let index = 0; index < length; index += 1) {
    sums[index + 1] = (sums[index] ?? 0) + (at(index) ? 1 : 0);
  }
  return sums;
}

/** The chosen pages, by index. */
export class Selection {
  private readonly bits: Uint8Array;
  count = 0;

  constructor(readonly size: number) {
    this.bits = new Uint8Array(size);
  }

  has(index: number): boolean {
    return this.bits[index] === 1;
  }

  /** Chooses (or lets go of) the pages `first .. first + count` that `scope` keeps. */
  setRange(first: number, count: number, on: boolean, scope: Scope = null): void {
    const end = Math.min(first + count, this.size);
    const value = on ? 1 : 0;
    for (let index = Math.max(first, 0); index < end; index += 1) {
      if (scope !== null && scope[index] !== 1) continue;
      if (this.bits[index] !== value) {
        this.bits[index] = value;
        this.count += on ? 1 : -1;
      }
    }
  }

  /** Every page `scope` keeps, chosen or let go of. */
  setAll(on: boolean, scope: Scope = null): void {
    this.setRange(0, this.size, on, scope);
  }

  /** The chosen pages of `pages`, in their order. */
  urls(pages: readonly string[]): string[] {
    return pages.filter((_, index) => this.bits[index] === 1);
  }
}

/** How a node's checkbox stands. */
export type CheckState = boolean | 'indeterminate';

/** The counts a tree's rows are drawn from: pages in scope and chosen pages in scope. */
export interface Counts {
  inScope: Int32Array;
  chosen: Int32Array;
}

/** The prefix sums of `selection` and `scope`, made again after every change (one pass). */
export function countsOf(selection: Selection, scope: Scope): Counts {
  const kept = (index: number): boolean => scope === null || scope[index] === 1;
  return {
    inScope: prefix(selection.size, kept),
    chosen: prefix(selection.size, (index) => kept(index) && selection.has(index)),
  };
}

function rangeOf(sums: Int32Array, node: FlatNode): number {
  return (sums[node.first + node.count] ?? 0) - (sums[node.first] ?? 0);
}

/** The pages of `node` in scope, and how many of them are chosen. */
export function nodeCounts(node: FlatNode, counts: Counts): { inScope: number; chosen: number } {
  return { inScope: rangeOf(counts.inScope, node), chosen: rangeOf(counts.chosen, node) };
}

/** The checkbox of `node`: every page in scope chosen, none, or some. */
export function checkState(node: FlatNode, counts: Counts): CheckState {
  const { inScope, chosen } = nodeCounts(node, counts);
  if (chosen === 0) return false;
  return chosen === inScope ? true : 'indeterminate';
}

/** Toggles `node`: a node whose pages in scope are all chosen lets go of them, any other
 * chooses them. */
export function toggle(node: FlatNode, selection: Selection, scope: Scope, counts: Counts): void {
  selection.setRange(node.first, node.count, checkState(node, counts) !== true, scope);
}

/** The ids of the rows shown, in order: nodes with a page in scope, the children of an open
 * node. */
export function visibleRows(tree: PageTree, isOpen: (id: number) => boolean, counts: Counts): number[] {
  const rows: number[] = [];
  const stack = [...tree.roots].reverse();
  while (stack.length > 0) {
    const id = stack.pop() as number;
    const node = tree.nodes[id];
    if (node === undefined || nodeCounts(node, counts).inScope === 0) continue;
    rows.push(id);
    if (node.children.length > 0 && isOpen(id)) {
      for (let at = node.children.length - 1; at >= 0; at -= 1) {
        stack.push(node.children[at] as number);
      }
    }
  }
  return rows;
}

/** The rows `start .. end` drawn for a list of `total` rows of `rowHeight` px scrolled to
 * `scrollTop` in a viewport of `viewport` px, with `overscan` rows around them. */
export function windowOf(
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  total: number,
  overscan = 8,
): { start: number; end: number } {
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const shown = Math.ceil(viewport / rowHeight) + overscan * 2;
  return { start, end: Math.min(total, start + shown) };
}

/** The views a run takes, from the start page's choice. */
export type ViewsChoice = 'all' | 'visible';

/** `sitemap.capture`'s arguments for the chosen pages (`views` only when not every view: the
 * default). */
export function captureArgs(
  discoveryId: string,
  urls: string[],
  env: string | null,
  views: ViewsChoice,
): { [key: string]: JsonValue } {
  const args: { [key: string]: JsonValue } = { discovery: discoveryId, urls };
  if (env !== null) args.env = env;
  if (views === 'visible') args.views = 'visible';
  return args;
}
