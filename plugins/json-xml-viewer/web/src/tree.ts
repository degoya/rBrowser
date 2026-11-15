// The visible rows of a tree: the roots and, below every open node, its children - as two
// parallel arrays (node, level) that the view renders a window of. Opening and closing a node
// splices its rows in or out; a node closed and opened again shows what was open below it.
// Opening many levels at once (the default depth, "expand all") keeps to a row budget, level by
// level from the top, so a large document never builds more rows than the budget.

import type { TreeNode } from './nodes';

/** Rows opened when a document opens (`expandDepth`). */
export const OPEN_BUDGET = 5000;
/** Rows "expand all" opens at most. */
export const EXPAND_ALL_BUDGET = 200000;

export class Tree {
  nodes: TreeNode[] = [];
  levels: number[] = [];
  private open = new Set<TreeNode>();

  /**
   * @param roots the top rows.
   * @param labelled whether the top rows are query results, shown with their path.
   */
  constructor(
    readonly roots: readonly TreeNode[],
    readonly labelled = false,
  ) {
    this.rebuild();
  }

  get length(): number {
    return this.nodes.length;
  }

  isOpen(node: TreeNode): boolean {
    return this.open.has(node);
  }

  /** The rows below `index` that belong to it: the end of its subtree (exclusive). */
  subtreeEnd(index: number): number {
    const level = this.levels[index] ?? 0;
    let end = index + 1;
    while (end < this.levels.length && (this.levels[end] as number) > level) end += 1;
    return end;
  }

  /** The row of `index`'s parent, -1 for a top row. */
  parentIndex(index: number): number {
    const level = this.levels[index] ?? 0;
    for (let i = index - 1; i >= 0; i -= 1) {
      if ((this.levels[i] as number) < level) return i;
    }
    return -1;
  }

  /** The rows of `node`'s open subtree below it, starting at `level`. */
  private rowsBelow(node: TreeNode, level: number, nodes: TreeNode[], levels: number[]): void {
    const stack: [TreeNode, number][] = [];
    const push = (parent: TreeNode, at: number) => {
      const kids = parent.children();
      for (let i = kids.length - 1; i >= 0; i -= 1) stack.push([kids[i] as TreeNode, at]);
    };
    push(node, level);
    while (stack.length > 0) {
      const [next, at] = stack.pop() as [TreeNode, number];
      nodes.push(next);
      levels.push(at);
      if (this.open.has(next)) push(next, at + 1);
    }
  }

  private rebuild(): void {
    const nodes: TreeNode[] = [];
    const levels: number[] = [];
    for (const root of this.roots) {
      nodes.push(root);
      levels.push(0);
      if (this.open.has(root)) this.rowsBelow(root, 1, nodes, levels);
    }
    this.nodes = nodes;
    this.levels = levels;
  }

  expand(index: number): void {
    const node = this.nodes[index];
    if (!node || this.open.has(node) || !node.hasChildren()) return;
    this.open.add(node);
    const nodes: TreeNode[] = [];
    const levels: number[] = [];
    this.rowsBelow(node, (this.levels[index] as number) + 1, nodes, levels);
    // concat, not splice(...rows): a spread of 100 000 rows would exceed the argument limit.
    this.nodes = this.nodes.slice(0, index + 1).concat(nodes, this.nodes.slice(index + 1));
    this.levels = this.levels.slice(0, index + 1).concat(levels, this.levels.slice(index + 1));
  }

  collapse(index: number): void {
    const node = this.nodes[index];
    if (!node || !this.open.has(node)) return;
    this.open.delete(node);
    const count = this.subtreeEnd(index) - index - 1;
    this.nodes.splice(index + 1, count);
    this.levels.splice(index + 1, count);
  }

  toggle(index: number): void {
    const node = this.nodes[index];
    if (node && this.open.has(node)) this.collapse(index);
    else this.expand(index);
  }

  /**
   * Opens every node above `depth` levels, top levels first, while the rows below the roots
   * stay within `budget`; everything deeper is closed. False when the budget stopped it.
   */
  expandToDepth(depth: number, budget = OPEN_BUDGET): boolean {
    this.open = new Set();
    let count = this.roots.length;
    let frontier: readonly TreeNode[] = this.roots;
    let complete = true;
    for (let level = 0; level < depth && frontier.length > 0 && complete; level += 1) {
      const next: TreeNode[] = [];
      for (const node of frontier) {
        const kids = node.childCount();
        if (kids === 0) continue;
        // The roots always open (a top-level array of 100 000 items is still one list to
        // scroll); the budget holds from the second level down.
        if (level > 0 && count + kids > budget) {
          complete = false;
          break;
        }
        this.open.add(node);
        count += kids;
        for (const kid of node.children()) next.push(kid);
      }
      frontier = next;
    }
    this.rebuild();
    return complete;
  }

  /** Opens everything within `budget` rows; false when the budget stopped it. */
  expandAll(budget = EXPAND_ALL_BUDGET): boolean {
    return this.expandToDepth(Number.POSITIVE_INFINITY, budget);
  }

  collapseAll(): void {
    this.open = new Set();
    this.rebuild();
  }
}
